use chrono::{DateTime, TimeZone, Utc};
use quota_core::{
    classify_originator, model_speed_coefficient, weighted_tokens, ClientKind, RateLimitSnapshot,
    RateLimitWindow, UsageEvent, UsageSource,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Default)]
pub struct LogBatch {
    pub sessions: Vec<(String, ClientKind)>,
    pub usage: Vec<UsageEvent>,
    pub rates: Vec<RateLimitSnapshot>,
}

#[derive(Debug)]
struct FileCursor {
    offset: u64,
    session_id: Option<String>,
    client: ClientKind,
    model: Option<String>,
    last_tokens: Option<TokenUsage>,
}

impl Default for FileCursor {
    fn default() -> Self {
        Self {
            offset: 0,
            session_id: None,
            client: ClientKind::Unknown,
            model: None,
            last_tokens: None,
        }
    }
}

pub struct SessionLogWatcher {
    root: PathBuf,
    files: HashMap<PathBuf, FileCursor>,
    pending_sessions: Vec<(String, ClientKind)>,
}

impl SessionLogWatcher {
    pub fn new(root: PathBuf) -> Self {
        let mut watcher = Self {
            root,
            files: HashMap::new(),
            pending_sessions: Vec::new(),
        };
        watcher.preload_existing();
        watcher
    }

    pub fn root_exists(&self) -> bool {
        self.root.is_dir()
    }

    fn preload_existing(&mut self) {
        if !self.root.is_dir() {
            return;
        }
        for path in jsonl_files(&self.root) {
            let mut cursor = FileCursor::default();
            hydrate_identity(&path, &mut cursor);
            hydrate_latest_counters(&path, &mut cursor);
            if let Some(session_id) = &cursor.session_id {
                self.pending_sessions
                    .push((session_id.clone(), cursor.client.clone()));
            }
            cursor.offset = std::fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            self.files.insert(path, cursor);
        }
    }

    pub fn scan(&mut self) -> LogBatch {
        let mut batch = LogBatch {
            sessions: std::mem::take(&mut self.pending_sessions),
            ..LogBatch::default()
        };
        if !self.root.is_dir() {
            return batch;
        }
        for path in jsonl_files(&self.root) {
            let cursor = self.files.entry(path.clone()).or_default();
            if let Ok(metadata) = std::fs::metadata(&path) {
                if metadata.len() < cursor.offset {
                    cursor.offset = 0;
                }
            }
            scan_file(&path, cursor, &mut batch);
        }
        batch
    }
}

fn hydrate_latest_counters(path: &Path, cursor: &mut FileCursor) {
    let Ok(file) = File::open(path) else { return };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(record) = serde_json::from_str::<LogRecord>(&line) else {
            continue;
        };
        match record.kind.as_str() {
            "turn_context" => {
                if let Some(model) = record.payload.model {
                    cursor.model = Some(model);
                }
            }
            "event_msg" if record.payload.event_type.as_deref() == Some("token_count") => {
                if let Some(tokens) = record.payload.info.and_then(|info| info.last_token_usage) {
                    cursor.last_tokens = Some(tokens);
                }
            }
            _ => {}
        }
    }
}

fn jsonl_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl")
        })
        .map(|entry| entry.into_path())
        .collect()
}

fn hydrate_identity(path: &Path, cursor: &mut FileCursor) {
    let Ok(file) = File::open(path) else { return };
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    for _ in 0..8 {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Ok(record) = serde_json::from_str::<LogRecord>(&line) {
            if record.kind == "session_meta" {
                apply_session_meta(&record.payload, cursor);
                break;
            }
        }
    }
}

fn scan_file(path: &Path, cursor: &mut FileCursor, batch: &mut LogBatch) {
    let Ok(mut file) = File::open(path) else {
        return;
    };
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() {
        return;
    }
    let mut reader = BufReader::new(file);
    loop {
        let start = cursor.offset;
        let mut line = String::new();
        let Ok(bytes) = reader.read_line(&mut line) else {
            break;
        };
        if bytes == 0 {
            break;
        }
        if !line.ends_with('\n') {
            let _ = reader.seek(SeekFrom::Start(start));
            break;
        }
        cursor.offset += bytes as u64;
        let Ok(record) = serde_json::from_str::<LogRecord>(&line) else {
            continue;
        };
        let observed_at = parse_timestamp(record.timestamp.as_deref()).unwrap_or_else(Utc::now);
        match record.kind.as_str() {
            "session_meta" => {
                apply_session_meta(&record.payload, cursor);
                if let Some(id) = &cursor.session_id {
                    batch.sessions.push((id.clone(), cursor.client.clone()));
                }
            }
            "turn_context" => {
                if let Some(model) = record.payload.model {
                    cursor.model = Some(model);
                }
            }
            "event_msg" if record.payload.event_type.as_deref() == Some("token_count") => {
                if let Some(rate_limits) = record
                    .payload
                    .rate_limits
                    .and_then(|raw| raw.into_snapshot(observed_at))
                {
                    batch.rates.push(rate_limits);
                }
                if let Some(tokens) = record.payload.info.and_then(|info| info.last_token_usage) {
                    let delta = tokens.delta_from(cursor.last_tokens);
                    cursor.last_tokens = Some(tokens);
                    let weighted = weighted_tokens(
                        delta.input_tokens,
                        delta.cached_input_tokens,
                        delta.output_tokens,
                        model_speed_coefficient(cursor.model.as_deref()),
                    );
                    if weighted > 0.0 {
                        batch.usage.push(UsageEvent {
                            source: usage_source(&cursor.client),
                            session_id: cursor
                                .session_id
                                .clone()
                                .unwrap_or_else(|| path.to_string_lossy().to_string()),
                            model: cursor.model.clone(),
                            weighted_tokens: weighted,
                            observed_at,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn apply_session_meta(payload: &Payload, cursor: &mut FileCursor) {
    cursor.session_id = payload.session_id.clone().or_else(|| payload.id.clone());
    cursor.client = classify_originator(payload.originator.as_deref());
}

fn usage_source(client: &ClientKind) -> UsageSource {
    match client {
        ClientKind::Desktop => UsageSource::Desktop,
        ClientKind::Cli => UsageSource::Cli,
        ClientKind::Ide => UsageSource::Ide,
        _ => UsageSource::LocalUnknown,
    }
}

pub fn classify_transcript(path: &str, sessions_root: &Path) -> ClientKind {
    let candidate = PathBuf::from(path);
    let Ok(root) = sessions_root.canonicalize() else {
        return ClientKind::Unknown;
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return ClientKind::Unknown;
    };
    if !candidate.starts_with(root)
        || candidate.extension().and_then(|value| value.to_str()) != Some("jsonl")
    {
        return ClientKind::Unknown;
    }
    let mut cursor = FileCursor::default();
    hydrate_identity(&candidate, &mut cursor);
    cursor.client
}

fn parse_timestamp(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[derive(Debug, Deserialize)]
struct LogRecord {
    #[serde(rename = "type")]
    kind: String,
    timestamp: Option<String>,
    #[serde(default)]
    payload: Payload,
}

#[derive(Debug, Default, Deserialize)]
struct Payload {
    #[serde(rename = "type")]
    event_type: Option<String>,
    id: Option<String>,
    session_id: Option<String>,
    originator: Option<String>,
    model: Option<String>,
    info: Option<TokenInfo>,
    rate_limits: Option<RawRateLimits>,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    last_token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
struct TokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl TokenUsage {
    fn delta_from(self, previous: Option<Self>) -> Self {
        let Some(previous) = previous else {
            return self;
        };
        if self.input_tokens < previous.input_tokens
            || self.cached_input_tokens < previous.cached_input_tokens
            || self.output_tokens < previous.output_tokens
        {
            return self;
        }
        Self {
            input_tokens: self.input_tokens - previous.input_tokens,
            cached_input_tokens: self.cached_input_tokens - previous.cached_input_tokens,
            output_tokens: self.output_tokens - previous.output_tokens,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawRateLimits {
    #[serde(default = "default_quota_id")]
    limit_id: String,
    primary: Option<RawWindow>,
    secondary: Option<RawWindow>,
}

fn default_quota_id() -> String {
    "codex".to_string()
}

impl RawRateLimits {
    fn into_snapshot(self, observed_at: DateTime<Utc>) -> Option<RateLimitSnapshot> {
        Some(RateLimitSnapshot {
            quota_id: self.limit_id,
            observed_at,
            primary: self.primary.and_then(RawWindow::into_window),
            secondary: self.secondary.and_then(RawWindow::into_window),
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawWindow {
    used_percent: f64,
    #[serde(alias = "windowDurationMins")]
    window_minutes: i64,
    #[serde(alias = "resetsAt")]
    resets_at: i64,
}

impl RawWindow {
    fn into_window(self) -> Option<RateLimitWindow> {
        Some(RateLimitWindow {
            used_percent: self.used_percent,
            window_minutes: self.window_minutes,
            resets_at: Utc.timestamp_opt(self.resets_at, 0).single()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_sessions() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "quotabar-log-test-{}-{suffix}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn classifies_only_known_desktop_originator() {
        let root = temporary_sessions();
        let desktop = root.join("desktop.jsonl");
        std::fs::write(
            &desktop,
            r#"{"timestamp":"2026-08-21T12:00:00Z","type":"session_meta","payload":{"id":"desktop-1","originator":"codex_work_desktop"}}
"#,
        )
        .unwrap();
        assert_eq!(
            classify_transcript(desktop.to_str().unwrap(), &root),
            ClientKind::Desktop
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_transcripts_outside_sessions_root() {
        let root = temporary_sessions();
        let outside = std::env::temp_dir().join("quotabar-outside.jsonl");
        std::fs::write(
            &outside,
            r#"{"type":"session_meta","payload":{"id":"x","originator":"Codex Desktop"}}
"#,
        )
        .unwrap();
        assert_eq!(
            classify_transcript(outside.to_str().unwrap(), &root),
            ClientKind::Unknown
        );
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn token_record_does_not_require_conversation_fields() {
        let record: LogRecord = serde_json::from_str(
            r#"{"timestamp":"2026-08-21T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":10}},"ignored_prompt":"not retained"}}"#,
        )
        .unwrap();
        let tokens = record.payload.info.unwrap().last_token_usage.unwrap();
        assert_eq!(
            weighted_tokens(
                tokens.input_tokens,
                tokens.cached_input_tokens,
                tokens.output_tokens,
                1.0
            ),
            162.0
        );
    }

    #[test]
    fn cumulative_token_counters_are_converted_to_deltas() {
        let previous = TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 20,
            output_tokens: 10,
        };
        let current = TokenUsage {
            input_tokens: 140,
            cached_input_tokens: 30,
            output_tokens: 14,
        };
        let delta = current.delta_from(Some(previous));
        assert_eq!(delta.input_tokens, 40);
        assert_eq!(delta.cached_input_tokens, 10);
        assert_eq!(delta.output_tokens, 4);
        assert_eq!(
            weighted_tokens(
                delta.input_tokens,
                delta.cached_input_tokens,
                delta.output_tokens,
                1.0
            ),
            65.0
        );
    }
}
