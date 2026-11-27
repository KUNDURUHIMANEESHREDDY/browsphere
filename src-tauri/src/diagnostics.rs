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
                "private" | "stage" | "count" | "kind" | "outcome" | "source" | "line" |
                "prompt_tokens" | "cached_prompt_tokens" | "generated_tokens" |
                "model_load_ms" | "prompt_eval_ms" | "generation_ms" | "total_ms" |
                "prompt_bytes" | "system_bytes" | "schema_constrained"
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
        let numeric = guarded_fields(json!({
            "model_route":"ollama:qwen2.5:3b-instruct",
            "prompt_tokens":1250,
            "cached_prompt_tokens":200,
            "generated_tokens":42,
            "model_load_ms":20,
            "prompt_eval_ms":345,
            "generation_ms":643,
            "total_ms":1012,
            "prompt_bytes":5422,
            "system_bytes":312,
            "schema_constrained":true,
            "prompt":"SECRET NEVER WRITE",
            "response":"SECRET NEVER WRITE"
        }));
        assert_eq!(numeric["prompt_tokens"], 1250);
        assert_eq!(numeric["schema_constrained"], true);
        assert!(numeric.get("prompt").is_none());
        assert!(numeric.get("response").is_none());
    }
}

/// Classifies known engine-generated outcomes without storing arbitrary
/// browser/page/model text that may be embedded in detailed error messages.
pub fn classify_agent_outcome(outcome: &str) -> &'static str {
    // TraceEntry::sanitized has already collapsed all untrusted text to these
    // fixed statuses. Only match them; never inspect a raw browser message.
    match outcome {
        "rejected" => "approval_rejected",
        "timed_out" => "approval_timeout",
        "unverified" => "unverified",
        "denied" => "denied",
        "failed" => "failed",
        "confirmed" => "verified",
        "ok" => "success",
        "stopped_or_unknown" => "unknown",
        _ => "other",
    }
}

/** A strictly sanitized snapshot of Lucia runs for the local diagnostics UI.
 * Never return raw log lines, page content, goals, model outputs, or arbitrary strings.
 * Private Browsing runs are not recorded by the event producer.
 */
fn public_lucia_event(value: &Value, channel: &str) -> Option<Value> {
    let name = value.get("event")?.as_str()?;
    if !match channel {
        "agent" => matches!(name, "run_started" | "action_trace" | "run_finished" | "tab_lifecycle"),
        "errors" => name == "agent_failure",
        _ => false,
    } { return None; }
    if value.get("channel")?.as_str()? != channel { return None; }
    let fields = value.get("fields")?.as_object()?;
    let run_id = fields.get("run_id")?.as_str()?;
    if uuid::Uuid::parse_str(run_id).is_err() { return None; }
    let timestamp = value.get("timestamp")?.as_str()?;
    if chrono::DateTime::parse_from_rfc3339(timestamp).is_err() { return None; }

    let fixed = |key: &str, choices: &[&str]| -> Option<&str> {
        fields.get(key).and_then(Value::as_str).filter(|s| choices.contains(s))
    };
    let step = fields.get("step").and_then(Value::as_u64).filter(|n| *n < 100000);
    let latency_ms = fields.get("latency_ms").and_then(Value::as_u64).filter(|n| *n < 86400000);
    let trace_id = fields.get("trace_id").and_then(Value::as_str)
        .filter(|s| uuid::Uuid::parse_str(s).is_ok());
    Some(json!({
        "timestamp": timestamp,
        "event": name,
        "run_id": run_id,
        "trace_id": trace_id,
        "step": step,
        "latency_ms": latency_ms,
        "action": fixed("action", &["navigate", "click", "type_text", "scroll", "download", "complete"]),
        "decision": fixed("decision", &["allow", "deny", "approval"]),
        "risk": fixed("risk", &["Low", "Medium", "High", "Critical"]),
        "model_route": fixed("model_route", &["macro", "agent_model"]),
        "status": fixed("status", &["running", "completed", "complete", "success", "replayed", "unverified", "rejected", "failed", "halted", "cancelled", "canceled", "stopped", "tab_closed", "tab_missing", "closed", "missing"]),
        "outcome": fixed("outcome", &["success", "failed", "failure", "ok", "error", "blocked", "denied", "timeout", "completed", "verified", "unverified", "loop_guard", "approval_rejected", "approval_timeout", "approval_closed", "cancelled", "replay_drift", "unknown", "other"]),
        "source": if name == "tab_lifecycle" { fixed("source", &[
            "user_close", "private_session_end", "startup_rollback",
            "unknown_close", "webview_missing"
        ]) } else { None },
        "kind": if name == "tab_lifecycle" { fixed("kind", &["intentional", "unexpected"]) } else { None },
        "error_kind": if name == "agent_failure" { Some("agent_run") } else { None },
    }))
}

/// Reads only bounded, known agent/error JSONL channels, including rotated files.
/// Returns a timeline ordered oldest first. Corrupt/unknown lines are skipped.
pub fn read_lucia_timeline(data_dir: &Path) -> Vec<Value> {
    use std::collections::VecDeque;
    let root = folder(data_dir);
    let mut events: VecDeque<Value> = VecDeque::new();
    for channel in ["agent", "errors"] {
        for idx in (0..=BACKUPS).rev() {
            let path = if idx == 0 { root.join(format!("{channel}.jsonl")) }
                else { root.join(format!("{channel}.{idx}.jsonl")) };
            if fs::metadata(&path).map(|m| m.len() > MAX_BYTES * 2).unwrap_or(true) {
                continue;
            }
            let Ok(file) = fs::File::open(path) else { continue };
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if line.len() > 4096 { continue; }
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if let Some(event) = public_lucia_event(&value, channel) {
                        if events.len() >= 4000 { events.pop_front(); }
                        events.push_back(event);
                    }
                }
            }
        }
    }
    let mut result: Vec<Value> = events.into_iter().collect();
    result.sort_by(|a, b| {
        a.get("timestamp").and_then(Value::as_str).cmp(
            &b.get("timestamp").and_then(Value::as_str))
    });
    result
}

#[cfg(test)]
mod dashboard_tests {
    use super::*;
    #[test]
    fn timeline_rejects_injected_fields_and_invalid_run_ids() {
        let run_id = uuid::Uuid::new_v4().to_string();
        let input = json!({"timestamp":"2026-11-23T05:00:00Z", "channel":"agent",
            "event":"action_trace", "fields":{"run_id":run_id, "step":2,
                "action":"click","outcome":"privatePassword123","goal":"secret",
                "trace_id":"not-an-id", "latency_ms":123}});
        let event = public_lucia_event(&input, "agent").unwrap();
        assert_eq!(event["action"], "click");
        assert!(event["outcome"].is_null());
        assert!(event["trace_id"].is_null());
        assert!(!event.to_string().contains("secret"));
        let close = public_lucia_event(&json!({
            "timestamp":"2026-11-23T05:00:00Z", "channel":"agent", "event":"tab_lifecycle",
            "fields":{"run_id":run_id, "status":"closed", "source":"user_close",
                "kind":"intentional", "url":"https://private.example/path",
                "secret":"do-not-log"}
        }), "agent").unwrap();
        assert_eq!(close["source"], "user_close");
        assert_eq!(close["kind"], "intentional");
        assert_eq!(close["status"], "closed");
        assert!(!close.to_string().contains("do-not-log"));
        assert!(!close.to_string().contains("private.example"));
        let missing = public_lucia_event(&json!({
            "timestamp":"2026-11-23T05:00:00Z", "channel":"agent", "event":"run_finished",
            "fields":{"run_id":run_id, "status":"tab_missing"}
        }), "agent").unwrap();
        assert_eq!(missing["status"], "tab_missing");
        assert!(!event.to_string().contains("privatePassword"));
        assert!(public_lucia_event(&json!({"timestamp":"2026-11-23T05:00:00Z",
            "channel":"agent","event":"run_started","fields":{"run_id":"bad-id"}}),
            "agent").is_none());
    }
}

#[cfg(test)]
mod dashboard_reader_tests {
    use super::*;
    #[test]
    fn classifies_only_sanitized_outcomes() {
        assert_eq!(classify_agent_outcome("confirmed"), "verified");
        assert_eq!(classify_agent_outcome("timed_out"), "approval_timeout");
        assert_eq!(classify_agent_outcome("unverified"), "unverified");
        assert_eq!(classify_agent_outcome("POSTCONDITION CONFIRMED: secret"), "other");
        assert_eq!(classify_agent_outcome("secret-token-value"), "other");
    }

    #[test]
    fn reads_retained_rotated_logs_without_leaking_unknown_fields() {
        let root = std::env::temp_dir().join(format!("browsphere-diag-test-{}", uuid::Uuid::new_v4()));
        let logs = root.join("logs");
        fs::create_dir_all(&logs).unwrap();
        let run = uuid::Uuid::new_v4().to_string();
        let record = |event: &str| json!({"timestamp":"2026-11-23T05:00:00Z",
            "channel":"agent", "event":event, "fields":{
                "run_id":run, "status":"failed", "api_key":"please-do-not-share"}});
        fs::write(logs.join("agent.1.jsonl"), format!("{}\n", record("run_started"))).unwrap();
        fs::write(logs.join("agent.jsonl"),
            format!("not json\n{}\n", record("run_finished"))).unwrap();
        let result = read_lucia_timeline(&root);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["event"], "run_started");
        assert_eq!(result[1]["status"], "failed");
        assert!(!serde_json::to_string(&result).unwrap().contains("please-do-not-share"));
        fs::remove_dir_all(root).unwrap();
    }
}
