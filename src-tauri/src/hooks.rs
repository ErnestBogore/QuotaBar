use chrono::Utc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MARKER: &str = "QUOTABAR_HOOK=1";

#[derive(Debug, Clone)]
pub struct HookManager {
    codex_home: PathBuf,
}

impl HookManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    pub fn hooks_path(&self) -> PathBuf {
        self.codex_home.join("hooks.json")
    }

    pub fn is_installed(&self) -> bool {
        read_json(&self.hooks_path())
            .map(|document| contains_marker(&document))
            .unwrap_or(false)
    }

    pub fn install(&self) -> Result<(), String> {
        let hook_binary = locate_hook_binary()?;
        self.install_with_binary(&hook_binary)
    }

    fn install_with_binary(&self, hook_binary: &Path) -> Result<(), String> {
        std::fs::create_dir_all(&self.codex_home).map_err(|error| error.to_string())?;
        let path = self.hooks_path();
        if path.exists() {
            let backup = self.codex_home.join(format!(
                "hooks.json.quotabar-backup-{}",
                Utc::now().format("%Y%m%d%H%M%S")
            ));
            std::fs::copy(&path, backup).map_err(|error| error.to_string())?;
        }
        let mut document = read_json(&path).unwrap_or_else(|| json!({"hooks": {}}));
        remove_marker_entries(&mut document);
        let root = document
            .as_object_mut()
            .ok_or("hooks.json must contain a JSON object")?;
        let hooks = root.entry("hooks").or_insert_with(|| json!({}));
        let hooks = hooks
            .as_object_mut()
            .ok_or("hooks.json 'hooks' value must be an object")?;
        let entries = hooks.entry("UserPromptSubmit").or_insert_with(|| json!([]));
        let entries = entries
            .as_array_mut()
            .ok_or("UserPromptSubmit hooks must be an array")?;
        let escaped = shell_quote(hook_binary);
        entries.push(json!({
          "hooks": [{
            "type": "command",
            "command": format!("{MARKER} {escaped}"),
            "timeout": 2,
            "statusMessage": "Checking QuotaBar's five-hour gate"
          }]
        }));
        write_json(&path, &document)
    }

    pub fn remove(&self) -> Result<(), String> {
        let path = self.hooks_path();
        if !path.exists() {
            return Ok(());
        }
        let mut document = read_json(&path).ok_or("Unable to parse hooks.json")?;
        remove_marker_entries(&mut document);
        write_json(&path, &document)
    }
}

fn locate_hook_binary() -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut candidates = Vec::new();
    if let Some(parent) = current.parent() {
        candidates.push(parent.join("quotabar-hook"));
        candidates.push(parent.join("../Resources/bin/quotabar-hook"));
        candidates.push(parent.join("../../../quotabar-hook"));
    }
    if let Some(workspace) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
        candidates.push(workspace.join("target/debug/quotabar-hook"));
        candidates.push(workspace.join("target/release/quotabar-hook"));
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "quotabar-hook is not bundled. Build it with `cargo build -p quotabar-hook` and repair the gate.".to_string())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn read_json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.quotabar-tmp");
    std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn contains_marker(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains(MARKER),
        Value::Array(items) => items.iter().any(contains_marker),
        Value::Object(map) => map.values().any(contains_marker),
        _ => false,
    }
}

fn remove_marker_entries(document: &mut Value) {
    let Some(entries) = document
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("UserPromptSubmit"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    entries.retain(|entry| !contains_marker(entry));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_codex_home() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("quotabar-hook-test-{suffix}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn removes_only_quotabar_entry() {
        let mut value = json!({"hooks":{"UserPromptSubmit":[
          {"hooks":[{"type":"command","command":"other"}]},
          {"hooks":[{"type":"command","command":"QUOTABAR_HOOK=1 '/app/quotabar-hook'"}]}
        ]}});
        remove_marker_entries(&mut value);
        let entries = value["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["hooks"][0]["command"], "other");
    }

    #[test]
    fn install_repair_and_uninstall_preserve_existing_hooks() {
        let root = temporary_codex_home();
        let manager = HookManager::new(root.clone());
        std::fs::write(
            manager.hooks_path(),
            r#"{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"existing-hook"}]}]}}"#,
        )
        .unwrap();

        manager
            .install_with_binary(Path::new(
                "/Applications/QuotaBar.app/Contents/MacOS/quotabar-hook",
            ))
            .unwrap();
        manager
            .install_with_binary(Path::new(
                "/Applications/QuotaBar.app/Contents/MacOS/quotabar-hook",
            ))
            .unwrap();
        let installed = read_json(&manager.hooks_path()).unwrap();
        let entries = installed["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.to_string().contains("existing-hook")));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| contains_marker(entry))
                .count(),
            1
        );

        let backups = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("quotabar-backup")
            })
            .count();
        assert!(backups >= 1);

        manager.remove().unwrap();
        let removed = read_json(&manager.hooks_path()).unwrap();
        let entries = removed["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].to_string().contains("existing-hook"));
        let _ = std::fs::remove_dir_all(root);
    }
}
