use chrono::{TimeZone, Utc};
use quota_core::{RateLimitSnapshot, RateLimitWindow};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub fn locate_codex() -> Option<PathBuf> {
    [
        PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex"),
        PathBuf::from("/Applications/Codex.app/Contents/Resources/codex"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .or_else(|| {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join("codex"))
                .find(|path| path.is_file())
        })
    })
}

pub async fn stream_rate_limits(
    codex: &Path,
    sender: mpsc::Sender<Vec<RateLimitSnapshot>>,
) -> Result<(), String> {
    let mut child = Command::new(codex)
        .args(["app-server", "--stdio"])
        .env("OTEL_SDK_DISABLED", "true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Unable to start Codex app-server: {error}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or("Codex app-server stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Codex app-server stdout unavailable")?;
    let messages = [
        json!({"method":"initialize","id":0,"params":{"clientInfo":{"name":"quotabar","title":"QuotaBar","version":env!("CARGO_PKG_VERSION")}}}),
        json!({"method":"initialized","params":{}}),
        json!({"method":"account/rateLimits/read","id":2}),
        json!({"method":"account/usage/read","id":3}),
    ];
    for message in messages {
        stdin
            .write_all(format!("{}\n", message).as_bytes())
            .await
            .map_err(|error| error.to_string())?;
    }
    stdin.flush().await.map_err(|error| error.to_string())?;

    let mut lines = BufReader::new(stdout).lines();
    let mut refresh = interval(Duration::from_secs(60));
    refresh.tick().await;
    let mut request_id = 10_i64;
    loop {
        tokio::select! {
          line = lines.next_line() => {
            let Some(line) = line.map_err(|error| error.to_string())? else {
                return Err("Codex app-server closed its output stream".to_string());
            };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message.get("id").and_then(Value::as_i64) == Some(2)
                && message.get("error").is_some()
            {
                if let Some(error) = message.get("error") {
                    return Err(format!("Codex rate-limit request failed: {error}"));
                }
            }
            let payload = if message.get("method").and_then(Value::as_str)
                == Some("account/rateLimits/updated")
            {
                message.get("params")
            } else {
                message.get("result")
            };
            if let Some(payload) = payload {
                if let Ok(snapshots) = parse_rate_response(payload.clone()) {
                    if sender.send(snapshots).await.is_err() {
                        return Ok(());
                    }
                }
            }
          }
          _ = refresh.tick() => {
            request_id += 1;
            let request = json!({"method":"account/rateLimits/read","id":request_id});
            stdin.write_all(format!("{request}\n").as_bytes()).await
                .map_err(|error| error.to_string())?;
            stdin.flush().await.map_err(|error| error.to_string())?;
          }
        }
    }
}

fn parse_rate_response(result: Value) -> Result<Vec<RateLimitSnapshot>, String> {
    let observed_at = Utc::now();
    let mut snapshots = Vec::new();
    if let Some(map) = result.get("rateLimitsByLimitId").and_then(Value::as_object) {
        for (id, value) in map {
            if let Some(snapshot) = parse_bucket(value, id, observed_at) {
                snapshots.push(snapshot);
            }
        }
    }
    if snapshots.is_empty() {
        if let Some(value) = result.get("rateLimits") {
            if let Some(snapshot) = parse_bucket(value, "codex", observed_at) {
                snapshots.push(snapshot);
            }
        }
    }
    if snapshots.is_empty()
        && (result.get("primary").is_some() || result.get("secondary").is_some())
    {
        if let Some(snapshot) = parse_bucket(&result, "codex", observed_at) {
            snapshots.push(snapshot);
        }
    }
    if snapshots.is_empty() {
        Err("The app-server returned no recognizable Codex quota buckets".to_string())
    } else {
        Ok(snapshots)
    }
}

fn parse_bucket(
    value: &Value,
    fallback_id: &str,
    observed_at: chrono::DateTime<Utc>,
) -> Option<RateLimitSnapshot> {
    let quota_id = value
        .get("limitId")
        .and_then(Value::as_str)
        .unwrap_or(fallback_id)
        .to_string();
    Some(RateLimitSnapshot {
        quota_id,
        observed_at,
        primary: value.get("primary").and_then(parse_window),
        secondary: value.get("secondary").and_then(parse_window),
    })
}

fn parse_window(value: &Value) -> Option<RateLimitWindow> {
    if value.is_null() {
        return None;
    }
    let used_percent = value
        .get("usedPercent")
        .or_else(|| value.get("used_percent"))?
        .as_f64()?;
    let window_minutes = value
        .get("windowDurationMins")
        .or_else(|| value.get("window_minutes"))?
        .as_i64()?;
    let resets_at = value
        .get("resetsAt")
        .or_else(|| value.get("resets_at"))?
        .as_i64()?;
    Some(RateLimitWindow {
        used_percent,
        window_minutes,
        resets_at: Utc.timestamp_opt(resets_at, 0).single()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_bucket_response() {
        let result = json!({
          "rateLimitsByLimitId": {
            "codex": {
              "limitId": "codex",
              "primary": {"usedPercent": 12.0, "windowDurationMins": 10080, "resetsAt": 1780000000},
              "secondary": null
            }
          }
        });
        let parsed = parse_rate_response(result).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].weekly().unwrap().used_percent, 12.0);
    }
}
