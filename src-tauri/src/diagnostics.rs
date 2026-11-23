//! Local, privacy-preserving runtime diagnostics. Never persist prompts, URLs,
//! DOM contents, model responses, typed text, credentials, or private agent runs.
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tracing_subscriber::{layer::Context, Layer};

const MAX_BYTES: u64 = 2 * 1024 * 1024;
const BACKUPS: usize = 4;
const CHANNELS: &[&str] = &["runtime", "browser", "agent", "model", "errors", "crash", "ui", "automation", "voice", "security", "downloads"];
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
static SESSION: OnceLock<String> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn init(data_dir: &Path) {
    let path = data_dir.join("logs");
    if fs::create_dir_all(&path).is_err() {
        eprintln!("Browsphere diagnostics folder could not be created");
        return;
    }
    let _ = LOG_DIR.set(path.clone());
    let _ = SESSION.set(uuid::Uuid::new_v4().to_string());
    // Keep a bounded retention period even if the app restarts frequently.
    if let Ok(entries) = fs::read_dir(&path) {
        for entry in entries.flatten() {
            let p = entry.path();
            let known = p.file_name().and_then(|n| n.to_str())
                .map(|name| CHANNELS.iter().any(|c| name.starts_with(&format!("{c}."))))
                .unwrap_or(false);
            if !known || !p.is_file() { continue; }
            if let Ok(age) = entry.metadata().and_then(|m| m.modified())
                .and_then(|m| SystemTime::now().duration_since(m)
                    .map_err(std::io::Error::other))
            {
                if age > Duration::from_secs(14 * 24 * 60 * 60) {
                    let _ = fs::remove_file(p);
                }
            }
        }
    }
    std::panic::set_hook(Box::new(|info| {
        let location = info.location();
        let source = location.map(|l| l.file().rsplit(['/', '\\']).next().unwrap_or("unknown"))
            .unwrap_or("unknown");
        // Panic payload deliberately omitted: it may include secrets or page data.
        record("crash", "panic", json!({"component": source, "error_kind": "panic",
            "line": location.map(|l| l.line()).unwrap_or(0)}));
        #[cfg(debug_assertions)]
        eprintln!("Browsphere encountered an unexpected error; see the local crash diagnostics");
    }));
    record("runtime", "session_start", json!({"component": "browsphere"}));
}

pub fn folder(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

fn guarded_fields(input: Value) -> Value {
    let mut fields = Map::new();
    if let Value::Object(map) = input {
        for (key, value) in map {
            // Explicit allowlist: adding a new event cannot accidentally start
            // persisting user messages, URLs, request bodies or secrets.
            if !matches!(key.as_str(),
                "component" | "run_id" | "trace_id" | "step" | "action" |
                "status" | "risk" | "decision" | "model_route" | "latency_ms" |
                "duration_ms" | "blocked_trackers" | "error_kind" | "level" |
                "private" | "stage" | "count" | "kind" | "outcome" | "source" | "line"
            ) { continue; }
            match value {
                Value::Bool(_) | Value::Number(_) | Value::Null => { fields.insert(key, value); }
                Value::String(s) => {
                    // Only identifiers, predetermined statuses, and static labels.
                    // Path separators, query parameters, emails, and spaces are denied.
                    let safe = s.len() <= 100 && s.chars().all(|c|
                        c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'));
                    fields.insert(key, Value::String(
                        if safe { s } else { "[redacted]".into() }
                    ));
                }
                _ => {}
            }
        }
    }
    Value::Object(fields)
}

pub fn record(channel: &str, event: &str, fields: Value) {
    if !CHANNELS.contains(&channel) { return; }
    let Some(root) = LOG_DIR.get() else { return };
    if event.len() > 80 || !event.chars().all(|c|
        c.is_ascii_alphanumeric() || matches!(c, '_' | '-')) { return; }
    let entry = json!({
        "timestamp": Utc::now().to_rfc3339(),
        "session_id": SESSION.get().map(String::as_str).unwrap_or("unknown"),
        "pid": std::process::id(),
        "channel": channel,
        "event": event,
        "fields": guarded_fields(fields)
    });
    let Ok(line) = serde_json::to_string(&entry) else { return };
    let Ok(_guard) = WRITE_LOCK.lock() else { return };
    let path = root.join(format!("{channel}.jsonl"));
    if fs::metadata(&path).map(|m| m.len() + line.len() as u64 + 1 > MAX_BYTES).unwrap_or(false) {
        for idx in (1..=BACKUPS).rev() {
            let older = if idx == 1 { path.clone() } else { root.join(format!("{channel}.{}.jsonl", idx - 1)) };
            let newer = root.join(format!("{channel}.{idx}.jsonl"));
            if newer.exists() { let _ = fs::remove_file(&newer); }
            if older.exists() { let _ = fs::rename(&older, &newer); }
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Native tracing framework events, including errors in lower-level modules.
/// Log only immutable metadata; message fields may contain untrusted page text.
pub struct RuntimeMetadataLayer;
impl<S: tracing::Subscriber> Layer<S> for RuntimeMetadataLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = meta.level().as_str().to_ascii_lowercase();
        record("runtime", "tracing_event", json!({
            "level": level,
            "component": meta.target().rsplit("::").next().unwrap_or("unknown")
        }));
    }
}

/// Manual diagnostic export. Only the already-sanitized JSONL log files are
/// exported. This intentionally excludes the history/agent SQLite database.
pub fn export(data_dir: &Path) -> std::io::Result<PathBuf> {
    let root = folder(data_dir);
    fs::create_dir_all(&root)?;
    let exports = data_dir.join("diagnostic-exports");
    fs::create_dir_all(&exports)?;
    let output = exports.join(format!("Browsphere-diagnostics-{}.jsonl", Utc::now().format("%Y%m%d-%H%M%S")));
    let mut destination = fs::File::create(&output)?;
    let mut bytes = 0usize;
    for channel in CHANNELS {
        for idx in (0..=BACKUPS).rev() {
            let path = if idx == 0 { root.join(format!("{channel}.jsonl")) }
                else { root.join(format!("{channel}.{idx}.jsonl")) };
            let Ok(input) = fs::File::open(path) else { continue };
            for line in BufReader::new(input).lines().map_while(Result::ok) {
                // Re-parse so the export never includes malformed or injected lines.
                if bytes >= 20 * 1024 * 1024 { break; }
                let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
                if value.get("channel").and_then(Value::as_str) != Some(channel) { continue; }
                if value.get("fields").is_none() { continue; }
                let encoded = serde_json::to_string(&value)?;
                destination.write_all(encoded.as_bytes())?;
                destination.write_all(b"\n")?;
                bytes += encoded.len() + 1;
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_secrets_and_user_provided_text() {
        let fields = guarded_fields(json!({
            "run_id": "abc-123",
            "api_key": "top-secret",
            "goal": "read my bank data",
            "model_route": "https://api.example/v1?token=secret",
            "action": "navigate",
            "error_kind": "http_500"
        }));
        assert!(fields.get("api_key").is_none());
        assert!(fields.get("goal").is_none());
        assert_eq!(fields["model_route"], "[redacted]");
        assert_eq!(fields["action"], "navigate");
    }
}
