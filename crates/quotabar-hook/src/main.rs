use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct HookInput {
    session_id: Option<String>,
    transcript_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GateRequest {
    version: u8,
    session_id: String,
    transcript_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GateResponse {
    decision: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct BlockOutput<'a> {
    decision: &'a str,
    reason: &'a str,
}

fn socket_path() -> Option<PathBuf> {
    dirs::data_dir().map(|root| root.join("QuotaBar").join("gate.sock"))
}

fn main() {
    // Hooks are deliberately fail-open: malformed input, a stopped QuotaBar app,
    // or an unknown session can never block Codex.
    let mut raw = String::new();
    if io::stdin().read_line(&mut raw).is_err() {
        return;
    }
    let Ok(input) = serde_json::from_str::<HookInput>(&raw) else {
        return;
    };
    let Some(session_id) = input.session_id else {
        return;
    };
    let Some(path) = socket_path() else {
        return;
    };
    let Ok(mut stream) = UnixStream::connect(path) else {
        return;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(350)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(350)));
    let request = GateRequest {
        version: 1,
        session_id,
        transcript_path: input.transcript_path,
    };
    let Ok(encoded) = serde_json::to_string(&request) else {
        return;
    };
    if writeln!(stream, "{encoded}").is_err() {
        return;
    }
    let mut response = String::new();
    if BufReader::new(stream).read_line(&mut response).is_err() {
        return;
    }
    let Ok(response) = serde_json::from_str::<GateResponse>(&response) else {
        return;
    };
    if response.decision == "block" {
        let reason = response
            .reason
            .as_deref()
            .unwrap_or("QuotaBar's five-hour limit has been reached.");
        if let Ok(output) = serde_json::to_string(&BlockOutput {
            decision: "block",
            reason,
        }) {
            println!("{output}");
        }
    }
}
