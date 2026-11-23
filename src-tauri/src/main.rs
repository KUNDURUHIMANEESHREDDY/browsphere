// Keep the Windows release app GUI-only; retain the console in debug builds for diagnostics.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod aws;
mod aws_discovery;
mod automation;
mod diagnostics;
mod privacy;
mod router;
mod site_permissions;
mod storage;
mod voice;
mod voice_system_prompt;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::webview::DownloadEvent;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use tracing_subscriber::prelude::*;

use aws::{AwsConfig, CloudSnapshot, CognitoSession, MemoryMeta};
use aws_discovery::AwsConnectResult;
use agent::action_executor::ActionResult;
use agent::dom_extractor::{request_live_dom_snapshot, DomSnapshot};
use agent::loop_runner::{run_react_loop, ApprovalRequest};
use agent::reactive_perception::{ReactivePerception, PerceptionStatus};
use agent::policy::TraceEntry;
use automation::engine::{AutomationEngine, AutomationJob, AutomationStep};
use privacy::downloads::DownloadManager;
use privacy::itp::{DownloadRecord, ItpEngine, ItpStats};
use router::model_router::{ChatAnswer, ChatTurn, ModelRouter, RouterConfig};
use site_permissions::{SitePermissions, SitePolicy};
use storage::credentials::CredentialStore;
use storage::sqlite_wal::{AgentRunSummary, BrowserDatabase, HistoryEntry, ItpHostCount, SavedCredentialMeta, WorkflowMacro, MemoryEntry};

// OAuth callback URLs and similar secrets must not end up in persistent history.
fn contains_sensitive_history_parameter(address: &str) -> bool {
    let Ok(parsed) = url::Url::parse(address) else { return false; };
    let is_secret = |name: &str| matches!(name.to_ascii_lowercase().as_str(),
        "access_token" | "id_token" | "refresh_token" | "code" | "token" |
        "api_key" | "apikey" | "password" | "session" | "auth" | "client_secret");
    parsed.query_pairs().any(|(key, _)| is_secret(&key)) ||
        parsed.fragment().map(|fragment|
            url::form_urlencoded::parse(fragment.as_bytes())
                .any(|(key, _)| is_secret(&key)))
            .unwrap_or(false)
}

#[cfg(test)]
mod browser_history_tests {
    use super::contains_sensitive_history_parameter;

    #[test]
    fn excludes_oauth_and_secret_urls_from_persistent_history() {
        assert!(contains_sensitive_history_parameter("https://login.example/callback?code=abc"));
        assert!(contains_sensitive_history_parameter("https://example.org/#access_token=secret"));
        assert!(contains_sensitive_history_parameter("https://example.org/?API_KEY=secret"));
        assert!(!contains_sensitive_history_parameter("https://example.org/page?sort=recent"));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub hidden: bool,
}

impl Default for TabBounds {
    fn default() -> Self {
        Self {
            x: 256.0,
            y: 52.0,
            width: 1104.0,
            height: 828.0,
            hidden: false,
        }
    }
}

#[derive(Clone)]
pub struct TabManagerState {
    pub active_tab_id: Arc<Mutex<String>>,
    pub active_url: Arc<Mutex<String>>,
    pub tab_labels: Arc<Mutex<HashMap<String, String>>>,
    pub bounds: Arc<Mutex<TabBounds>>,
    pub dark_mode: Arc<AtomicBool>,
}

impl TabManagerState {
    pub fn new() -> Self {
        Self {
            active_tab_id: Arc::new(Mutex::new("tab-1".to_string())),
            active_url: Arc::new(Mutex::new("browsphere://start".to_string())),
            tab_labels: Arc::new(Mutex::new(HashMap::new())),
            bounds: Arc::new(Mutex::new(TabBounds::default())),
            dark_mode: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn sanitize_label(tab_id: &str) -> String {
        let clean: String = tab_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        if clean.is_empty() {
            "tab_default".to_string()
        } else {
            format!("wv_{clean}")
        }
    }

    pub async fn active_webview_label(&self) -> String {
        let tab_id = self.active_tab_id.lock().await.clone();
        let map = self.tab_labels.lock().await;
        map.get(&tab_id)
            .cloned()
            .unwrap_or_else(|| Self::sanitize_label(&tab_id))
    }
}


pub struct SpeechRuntimeState {
    pub tts_child: Arc<Mutex<Option<std::process::Child>>>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestReport {
    pub iterations: u32,
    pub concurrency: u32,
    pub allowed_count: u32,
    pub approval_gated_count: u32,
    pub denied_count: u32,
    pub macro_roundtrips: u32,
    pub elapsed_ms: u128,
}

#[tauri::command]
async fn run_policy_load_test(
    state: tauri::State<'_, Arc<AppState>>,
    iterations: Option<u32>,
    concurrency: Option<u32>,
) -> Result<LoadTestReport, String> {
    let total_iters = iterations.unwrap_or(200).clamp(10, 5000);
    let workers = concurrency.unwrap_or(8).clamp(1, 64);
    let started = std::time::Instant::now();

    let allowed = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let gated = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let denied = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let per_worker = (total_iters + workers - 1) / workers;
    let mut handles = Vec::with_capacity(workers as usize);

    for w_idx in 0..workers {
        let a_c = allowed.clone();
        let g_c = gated.clone();
        let d_c = denied.clone();
        handles.push(tokio::spawn(async move {
            let allowed_doms = vec!["example.org".to_string(), "github.com".to_string()];
            for i in 0..per_worker {
                let sample_action = match (w_idx + i) % 4 {
                    0 => crate::agent::policy::BrowserAction::Scroll { delta_y: 300 },
                    1 => crate::agent::policy::BrowserAction::Click {
                        node_id: format!("btn-{}", i),
                        element_label: "LoadTestButton".to_string(),
                        is_submit: false,
                    },
                    2 => crate::agent::policy::BrowserAction::TypeText {
                        node_id: "pw-1".to_string(),
                        text: "secret".to_string(),
                        is_sensitive: true,
                    },
                    _ => crate::agent::policy::BrowserAction::Navigate {
                        url: "https://evil-unlisted.example/login".to_string(),
                    },
                };

                let (_risk, decision) = crate::agent::policy::evaluate_policy(
                    "https://example.org",
                    &sample_action,
                    &allowed_doms,
                );
                match decision {
                    crate::agent::policy::PolicyDecision::Allow => {
                        a_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    crate::agent::policy::PolicyDecision::RequireApproval { .. } => {
                        g_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    crate::agent::policy::PolicyDecision::Deny { .. } => {
                        d_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    // Exercise SQLite WAL macro lookup & hit count under load
    let macro_roundtrips: u32 = {
        let mut db = state.db.lock().await;
        let test_goal = "__load_test_macro_check__";
        let test_origin = "https://loadtest.local";
        let steps = vec![crate::agent::policy::BrowserAction::Scroll { delta_y: 120 }];
        let mut hits = 0_u32;
        if let Ok(saved) = db.upsert_workflow_macro(test_origin, test_goal, &steps) {
            for _ in 0..10 {
                if db.find_workflow_macro(test_origin, test_goal).ok().flatten().is_some()
                    && db.increment_macro_hit(&saved.macro_id).is_ok()
                {
                    hits += 1;
                }
            }
            let _ = db.delete_workflow_macro(&saved.macro_id);
        }
        hits
    };

    Ok(LoadTestReport {
        iterations: total_iters,
        concurrency: workers,
        allowed_count: allowed.load(std::sync::atomic::Ordering::Relaxed),
        approval_gated_count: gated.load(std::sync::atomic::Ordering::Relaxed),
        denied_count: denied.load(std::sync::atomic::Ordering::Relaxed),
        macro_roundtrips,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[tauri::command]
async fn check_for_updates() -> Result<String, String> {
    // Never advertise or fetch unverified updates without a real publisher key.
    Ok("Secure automatic updates are disabled until release signing is configured.".into())
}

#[tauri::command]
async fn speak_text(
    state: tauri::State<'_, Arc<AppState>>,
    text: String,
    rate: Option<i32>,
) -> Result<bool, String> {
    let clean = text.trim();
    if clean.is_empty() {
        return Ok(false);
    }

    // Stop any currently playing utterance first
    if let Some(mut prev) = state.speech.tts_child.lock().await.take() {
        let _ = prev.kill();
        let _ = prev.wait();
    }

    let clamped_rate = rate.unwrap_or(0).clamp(-10, 10);
    let b64_text = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(clean.as_bytes())
    };

    let child_res = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let ps_script = format!(
                "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate = {clamped_rate}; $txt = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{b64_text}')); $s.Speak($txt)"
            );
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_script])
                .spawn()
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (clamped_rate, b64_text);
            std::process::Command::new("true").spawn()
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let child = child_res.map_err(|e| format!("TTS launch failed: {e}"))?;
    *state.speech.tts_child.lock().await = Some(child);
    Ok(true)
}

#[tauri::command]
async fn stop_speaking(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Some(mut prev) = state.speech.tts_child.lock().await.take() {
        let _ = prev.kill();
        let _ = prev.wait();
    }
    Ok(())
}

#[tauri::command]
async fn listen_speech_once(timeout_secs: Option<u64>) -> Result<Option<String>, String> {
    let secs = timeout_secs.unwrap_or(8).clamp(2, 30);
    let out = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let ps_script = format!(
                "Add-Type -AssemblyName System.Speech; $rec = New-Object System.Speech.Recognition.SpeechRecognizer; $res = $rec.Recognize([TimeSpan]::FromSeconds({secs})); if ($null -ne $res -and -not [string]::IsNullOrWhiteSpace($res.Text)) {{ [Console]::Out.Write($res.Text) }}"
            );
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_script])
                .output()
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = secs;
            std::process::Command::new("echo").args([""]).output()
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    let output = out.map_err(|e| format!("Speech recognition failed: {e}"))?;
    let transcript = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if transcript.is_empty() {
        Ok(None)
    } else {
        Ok(Some(transcript))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlPreview {
    pub url: String,
    pub domain: String,
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub excerpt: String,
}

pub struct ActiveAgentTarget {
    tab_id: String,
    run_id: Uuid,
    tab_closed: CancellationToken,
}

pub struct AppState {
    pub db: Arc<Mutex<BrowserDatabase>>,
    pub creds: Arc<CredentialStore>,
    pub router: Arc<Mutex<ModelRouter>>,
    pub aws_config: Arc<Mutex<AwsConfig>>,
    pub cognito_session: Arc<Mutex<Option<CognitoSession>>>,
    pub itp: Arc<ItpEngine>,
    pub downloads: Arc<DownloadManager>,
    pub speech: Arc<SpeechRuntimeState>,
    pub automation: Arc<AutomationEngine>,
    pub tabs: TabManagerState,
    pub perception: ReactivePerception,
    pub preview_cache: Arc<Mutex<HashMap<String, UrlPreview>>>,
    pub pending_approvals: Arc<Mutex<HashMap<Uuid, oneshot::Sender<bool>>>>,
    pub pending_snapshots: Arc<Mutex<HashMap<Uuid, oneshot::Sender<DomSnapshot>>>>,
    pub pending_action_results: Arc<Mutex<HashMap<Uuid, oneshot::Sender<ActionResult>>>>,
    pub active_cancel: Arc<Mutex<Option<CancellationToken>>>,
    pub active_agent_target: Arc<Mutex<Option<ActiveAgentTarget>>>,
    pub pause_tx: watch::Sender<bool>,
}

fn resolve_data_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let mut p = PathBuf::from(local_app_data);
        p.push("BrowsphereAI");
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    let mut p = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    p.push("data");
    let _ = std::fs::create_dir_all(&p);
    p
}

#[tauri::command]
fn window_close(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("ai-chat") {
        let _ = chat.close();
    }
    if let Some(w) = app.get_window("main") {
        w.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn window_minimize(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("ai-chat") {
        let _ = chat.hide();
    }
    if let Some(w) = app.get_window("main") {
        w.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn compute_ai_chat_rect(app: &tauri::AppHandle) -> (f64, f64, f64, f64) {
    let chat_w = 396.0;
    if let Some(main_w) = app.get_window("main") {
        let sf = main_w.scale_factor().unwrap_or(1.0).max(0.5);
        let (mx, my) = main_w
            .outer_position()
            .map(|p| (p.x as f64 / sf, p.y as f64 / sf))
            .unwrap_or((120.0, 80.0));
        let (mw, mh) = main_w
            .outer_size()
            .map(|s| (s.width as f64 / sf, s.height as f64 / sf))
            .unwrap_or((1360.0, 880.0));
        let chat_h = (mh - 96.0).clamp(380.0, 540.0);
        let chat_x = (mx + mw - chat_w - 18.0).max(mx + 16.0);
        let chat_y = my + 62.0;
        return (chat_x, chat_y, chat_w, chat_h);
    }
    (900.0, 120.0, chat_w, 520.0)
}

#[tauri::command]
async fn toggle_ai_chat_window(
    app: tauri::AppHandle,
    force_open: Option<bool>,
    title: Option<String>,
    url: Option<String>,
    dark: Option<bool>,
    initial_prompt: Option<String>,
    is_private: Option<bool>,
    force_task: Option<bool>,
) -> Result<bool, String> {
    let ctx_title = title.unwrap_or_else(|| "Active Page".to_string());
    let ctx_url = url.unwrap_or_else(|| "browsphere://start".to_string());
    let is_dark = dark.unwrap_or(false);
    let private_mode = is_private.unwrap_or(false);

    if let Some(chat) = app.get_webview_window("ai-chat") {
        let currently_visible = chat.is_visible().unwrap_or(false);
        let should_open = force_open.unwrap_or(!currently_visible);
        if should_open {
            if !currently_visible {
                let (cx, cy, cw, ch) = compute_ai_chat_rect(&app);
                let _ = chat.set_size(LogicalSize::new(cw, ch));
                let _ = chat.set_position(LogicalPosition::new(cx, cy));
            }
            let _ = chat.set_always_on_top(true);
            let _ = chat.show();
            let _ = chat.set_focus();
            let _ = chat.emit(
                "ai-chat:context",
                serde_json::json!({ "title": ctx_title, "url": ctx_url, "dark": is_dark, "is_private": private_mode }),
            );
            if let Some(prompt) = initial_prompt {
                if !prompt.trim().is_empty() {
                    let _ = chat.emit("ai-chat:prompt", serde_json::json!({ "prompt": prompt, "force_task": force_task.unwrap_or(false) }));
                }
            }
            return Ok(true);
        } else {
            let _ = chat.hide();
            return Ok(false);
        }
    }

    if force_open == Some(false) {
        return Ok(false);
    }

    let (cx, cy, cw, ch) = compute_ai_chat_rect(&app);
    let init_payload = serde_json::json!({
        "title": ctx_title,
        "url": ctx_url,
        "dark": is_dark,
        "is_private": private_mode
    });
    let init_script = format!("window.__AETHER_CHAT_INIT__ = {};", init_payload);

    let chat = WebviewWindowBuilder::new(&app, "ai-chat", WebviewUrl::App("ai-chat.html".into()))
        .title("Lucia")
        .inner_size(cw, ch)
        .min_inner_size(320.0, 340.0)
        .position(cx, cy)
        .decorations(false)
        .resizable(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(true)
        .visible(true)
        .initialization_script(&init_script)
        .build()
        .map_err(|e| format!("Failed to create Lucia window: {e}"))?;

    let _ = chat.set_focus();

    let chat_clone = chat.clone();
    let t_c = ctx_title.clone();
    let u_c = ctx_url.clone();
    let p_c = initial_prompt.clone();
    let force_task_c = force_task.unwrap_or(false);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(260)).await;
        let _ = chat_clone.emit(
            "ai-chat:context",
            serde_json::json!({ "title": t_c, "url": u_c, "dark": is_dark, "is_private": private_mode }),
        );
        if let Some(prompt) = p_c {
            if !prompt.trim().is_empty() {
                let _ = chat_clone.emit("ai-chat:prompt", serde_json::json!({ "prompt": prompt, "force_task": force_task_c }));
            }
        }
    });

    Ok(true)
}

#[tauri::command]
async fn dock_ai_chat_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("ai-chat") {
        let (cx, cy, cw, ch) = compute_ai_chat_rect(&app);
        let _ = chat.set_size(LogicalSize::new(cw, ch));
        let _ = chat.set_position(LogicalPosition::new(cx, cy));
        let _ = chat.set_always_on_top(true);
        let _ = chat.set_focus();
    }
    Ok(())
}

#[tauri::command]
async fn sync_ai_chat_context(
    app: tauri::AppHandle,
    title: String,
    url: String,
    dark: bool,
    is_private: Option<bool>,
) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("ai-chat") {
        let _ = chat.emit(
            "ai-chat:context",
            serde_json::json!({ "title": title, "url": url, "dark": dark, "is_private": is_private.unwrap_or(false) }),
        );
    }
    Ok(())
}

#[tauri::command]
async fn ai_chat_start_dragging(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("ai-chat") {
        chat.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn ai_chat_close(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(chat) = app.get_webview_window("ai-chat") {
        let _ = chat.hide();
    }
    Ok(())
}

#[tauri::command]
async fn ai_chat_open_url(app: tauri::AppHandle, url: String, new_tab: Option<bool>) -> Result<(), String> {
    if let Some(main_w) = app.get_window("main") {
        let _ = main_w.emit(
            "ai-chat:open-url",
            serde_json::json!({
                "url": url,
                "new_tab": new_tab.unwrap_or(false)
            }),
        );
        let _ = main_w.set_focus();
    }
    Ok(())
}

fn decode_basic_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let close = lower[open_end..].find("</title>")? + open_end;
    let raw = html.get(open_end..close)?.trim();
    let cleaned = decode_basic_entities(raw)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() { None } else { Some(cleaned) }
}

fn extract_meta_attr(tag: &str, attr_name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let needle = format!("{attr_name}=");
    let pos = lower.find(&needle)?;
    let rest = tag.get(pos + needle.len()..)?.trim_start();
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let after = &rest[quote.len_utf8()..];
        let end = after.find(quote)?;
        return Some(decode_basic_entities(after[..end].trim()));
    }
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '>')
        .unwrap_or(rest.len());
    Some(decode_basic_entities(rest[..end].trim()))
}

fn extract_meta_content(html: &str, keys: &[&str]) -> Option<String> {
    let lower = html.to_lowercase();
    let mut cursor = 0;
    while let Some(rel) = lower[cursor..].find("<meta") {
        let start = cursor + rel;
        let end = match lower[start..].find('>') {
            Some(e) => start + e + 1,
            None => break,
        };
        if let Some(tag) = html.get(start..end) {
            let prop = extract_meta_attr(tag, "property").unwrap_or_default().to_lowercase();
            let name = extract_meta_attr(tag, "name").unwrap_or_default().to_lowercase();
            if keys.iter().any(|k| prop == *k || name == *k) {
                if let Some(content) = extract_meta_attr(tag, "content") {
                    let trimmed = content.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
        cursor = end;
    }
    None
}

fn extract_plain_excerpt(html: &str, max_chars: usize) -> String {
    let lower = html.to_lowercase();
    let body_start = lower.find("<body").unwrap_or(0);
    let slice = &html[body_start..];
    let mut out = String::with_capacity(max_chars.min(512));
    let mut in_tag = false;
    let mut in_skip_block = false;
    let mut tag_buf = String::new();

    for ch in slice.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let t = tag_buf.trim().to_lowercase();
                if t.starts_with("script") || t.starts_with("style") || t.starts_with("noscript") || t.starts_with("svg") {
                    in_skip_block = true;
                } else if t.starts_with("/script") || t.starts_with("/style") || t.starts_with("/noscript") || t.starts_with("/svg") {
                    in_skip_block = false;
                }
                if !in_skip_block && !out.ends_with(' ') {
                    out.push(' ');
                }
            } else if tag_buf.len() < 32 {
                tag_buf.push(ch);
            }
            continue;
        }
        if !in_skip_block {
            if ch.is_whitespace() {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            } else {
                out.push(ch);
            }
            if out.len() >= max_chars * 2 {
                break;
            }
        }
    }

    let decoded = decode_basic_entities(&out);
    let cleaned = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned.chars().take(max_chars).collect()
}

#[tauri::command]
async fn fetch_url_preview(
    state: tauri::State<'_, Arc<AppState>>,
    url: String,
) -> Result<UrlPreview, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Empty URL".to_string());
    }
    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let parsed = url::Url::parse(&normalized).map_err(|e| format!("Invalid URL: {e}"))?;
    if !state.itp.allow_navigation(&parsed) {
        return Err("Blocked by ITP / scheme policy".to_string());
    }

    let cache_key = parsed.to_string();
    {
        let cache = state.preview_cache.lock().await;
        if let Some(hit) = cache.get(&cache_key) {
            return Ok(hit.clone());
        }
    }

    let domain = parsed.host_str().unwrap_or("web").to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::limited(4))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Browsphere/0.1 Safari/537.36")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(cache_key.clone())
        .send()
        .await
        .map_err(|e| format!("Preview fetch failed: {e}"))?;

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let capped = if bytes.len() > 96 * 1024 {
        &bytes[..96 * 1024]
    } else {
        &bytes[..]
    };
    let html = String::from_utf8_lossy(capped);

    let title = extract_meta_content(&html, &["og:title", "twitter:title"])
        .or_else(|| extract_html_title(&html))
        .unwrap_or_else(|| domain.clone());

    let description = extract_meta_content(&html, &["og:description", "twitter:description", "description"])
        .unwrap_or_default();

    let raw_image = extract_meta_content(&html, &["og:image", "twitter:image", "twitter:image:src"]);
    let image_url = raw_image.and_then(|img| {
        if img.starts_with("http://") || img.starts_with("https://") {
            Some(img)
        } else {
            parsed.join(&img).ok().map(|u| u.to_string())
        }
    });

    let excerpt = if !description.is_empty() {
        description.chars().take(240).collect()
    } else {
        extract_plain_excerpt(&html, 240)
    };

    let preview = UrlPreview {
        url: cache_key.clone(),
        domain,
        title,
        description,
        image_url,
        excerpt,
    };

    {
        let mut cache = state.preview_cache.lock().await;
        if cache.len() > 80 {
            cache.clear();
        }
        cache.insert(cache_key, preview.clone());
    }

    Ok(preview)
}


#[tauri::command]
fn window_toggle_maximize(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_window("main") {
        if w.is_maximized().unwrap_or(false) {
            w.unmaximize().map_err(|e| e.to_string())?;
        } else {
            w.maximize().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn window_start_dragging(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_window("main") {
        w.start_dragging().map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn hide_other_tab_webviews(app: &tauri::AppHandle, tabs: &TabManagerState, keep_label: &str) {
    let labels: Vec<String> = tabs.tab_labels.lock().await.values().cloned().collect();
    for lbl in labels {
        if lbl != keep_label {
            if let Some(wv) = app.get_webview(&lbl) {
                let _ = wv.hide();
            }
        }
    }
}

fn apply_webview_geometry(wv: &tauri::Webview, bounds: &TabBounds) {
    if bounds.hidden || bounds.width < 40.0 || bounds.height < 40.0 {
        let _ = wv.hide();
    } else {
        let _ = wv.set_position(LogicalPosition::new(bounds.x.max(0.0), bounds.y.max(0.0)));
        let _ = wv.set_size(LogicalSize::new(bounds.width.max(40.0), bounds.height.max(40.0)));
        let _ = wv.show();
    }
}


fn build_theme_sync_script(dark: bool) -> String {
    let mode = if dark { "dark" } else { "light" };
    format!(
        r#"(function() {{
  const MODE = "{mode}";
  const STYLE_ID = "__aether_dark_mode_style";

  function parseRgb(str) {{
    const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/.exec(str || "");
    if (!m) return null;
    return {{ r: +m[1], g: +m[2], b: +m[3], a: m[4] === undefined ? 1 : +m[4] }};
  }}

  function isLightBackground() {{
    const bodyBg = document.body ? parseRgb(getComputedStyle(document.body).backgroundColor) : null;
    const htmlBg = document.documentElement ? parseRgb(getComputedStyle(document.documentElement).backgroundColor) : null;
    const bg = (bodyBg && bodyBg.a > 0.1) ? bodyBg : (htmlBg && htmlBg.a > 0.1 ? htmlBg : {{ r: 255, g: 255, b: 255, a: 1 }});
    const lum = (0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b);
    return lum > 140;
  }}

  function applyTheme() {{
    if (document.documentElement) {{
      document.documentElement.style.colorScheme = MODE;
    }}
    let meta = document.querySelector('meta[name="color-scheme"]');
    if (!meta && document.head) {{
      meta = document.createElement("meta");
      meta.name = "color-scheme";
      document.head.appendChild(meta);
    }}
    if (meta) meta.content = MODE;

    const existing = document.getElementById(STYLE_ID);
    if (MODE === "light") {{
      if (existing) existing.remove();
      return;
    }}

    // Wait a tick for native prefers-color-scheme: dark CSS to settle; if still bright, apply smart dark inversion
    setTimeout(() => {{
      if (document.documentElement.style.colorScheme !== "dark") return;
      if (!isLightBackground()) {{
        const ex = document.getElementById(STYLE_ID);
        if (ex) ex.remove();
        return;
      }}
      if (!document.getElementById(STYLE_ID)) {{
        const st = document.createElement("style");
        st.id = STYLE_ID;
        st.textContent = `
          html {{ filter: invert(0.92) hue-rotate(180deg) !important; background: #111 !important; }}
          img, video, picture, canvas, iframe, [style*="background-image"] {{
            filter: invert(1) hue-rotate(180deg) !important;
          }}
        `;
        (document.head || document.documentElement).appendChild(st);
      }}
    }}, 120);
  }}

  if (document.readyState === "loading") {{
    document.addEventListener("DOMContentLoaded", applyTheme, {{ once: true }});
  }} else {{
    applyTheme();
  }}
}})();"#,
        mode = mode
    )
}

#[tauri::command]
async fn set_app_theme(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    dark: bool,
) -> Result<(), String> {
    state.tabs.dark_mode.store(dark, Ordering::Relaxed);

    if let Some(main_window) = app.get_window("main") {
        let target_theme = if dark {
            tauri::Theme::Dark
        } else {
            tauri::Theme::Light
        };
        let _ = main_window.set_theme(Some(target_theme));
    }

    let script = build_theme_sync_script(dark);
    let labels: Vec<String> = state.tabs.tab_labels.lock().await.values().cloned().collect();
    for lbl in labels {
        if let Some(wv) = app.get_webview(&lbl) {
            let _ = wv.eval(&script);
        }
    }

    Ok(())
}

#[tauri::command]
fn get_site_permissions(
    webview: tauri::Webview,
    state: tauri::State<'_, SitePermissions>,
    origin: String,
) -> Result<SitePolicy, String> {
    if webview.label() != "main" { return Err("Only the browser chrome can manage site permissions".into()); }
    state.list(&origin)
}

#[tauri::command]
fn set_site_permission(
    webview: tauri::Webview,
    state: tauri::State<'_, SitePermissions>,
    origin: String,
    permission: String,
    allowed: bool,
) -> Result<SitePolicy, String> {
    if webview.label() != "main" { return Err("Only the browser chrome can manage site permissions".into()); }
    let result = state.update(&origin, &permission, allowed);
    if result.is_ok() {
        diagnostics::record("security", "permission_changed",
            serde_json::json!({"kind": if allowed { "allowed" } else { "denied" }}));
    }
    result
}

#[tauri::command]
async fn open_or_navigate_tab(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    tab_id: String,
    url: String,
    bounds: Option<TabBounds>,
    is_private: Option<bool>,
) -> Result<(), String> {
    open_or_navigate_tab_inner(app, state, tab_id, url, bounds, is_private, true).await
}

async fn open_or_navigate_tab_inner(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    tab_id: String,
    url: String,
    bounds: Option<TabBounds>,
    is_private: Option<bool>,
    activate: bool,
) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {e}"))?;
    if !state.itp.allow_navigation(&parsed) {
        let host = parsed.host_str().unwrap_or("unknown").to_string();
        let _ = app.emit(
            "itp:blocked",
            serde_json::json!({
                "origin": parsed.to_string(),
                "blocked_host": host,
                "resource_type": "navigation",
                "total_blocked": state.itp.stats().total_blocked
            }),
        );
        return Err(format!("Blocked by ITP / scheme policy: {url}"));
    }

    let target = parsed.to_string();
    if activate {
        *state.tabs.active_tab_id.lock().await = tab_id.clone();
        *state.tabs.active_url.lock().await = target.clone();
    }

    let mut effective_bounds = if let Some(b) = bounds {
        if activate { *state.tabs.bounds.lock().await = b.clone(); }
        b
    } else {
        state.tabs.bounds.lock().await.clone()
    };
    // Background task views must never steal foreground geometry or tab focus.
    if !activate { effective_bounds.hidden = true; }

    let itp_verdict = state.itp.check_url(&target);
    if target != "about:blank" && !is_private.unwrap_or(false) && !contains_sensitive_history_parameter(&target) {
        let mut db = state.db.lock().await;
        let _ = db.record_visit(&target, None, itp_verdict.blocked_trackers);
    }

    let label = TabManagerState::sanitize_label(&tab_id);
    {
        let mut map = state.tabs.tab_labels.lock().await;
        map.insert(tab_id.clone(), label.clone());
    }

    if activate {
        hide_other_tab_webviews(&app, &state.tabs, &label).await;
    }

    if let Some(existing) = app.get_webview(&label) {
        state.perception.invalidate(&label);
        existing.navigate(parsed)
            .map_err(|e| format!("Could not navigate website: {e}"))?;
        apply_webview_geometry(&existing, &effective_bounds);
        return Ok(());
    }

    let main_window = app
        .get_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    let itp_for_nav = state.itp.clone();
    let app_for_nav = app.clone();
    let app_for_popups = app.clone();
    let app_for_permissions = app.clone();
    let tab_id_for_permissions = tab_id.clone();
    let private_for_permissions = is_private.unwrap_or(false);
    let private_for_popups = is_private.unwrap_or(false);
    let private_for_page_load = is_private.unwrap_or(false);
    let tab_id_for_popups = tab_id.clone();
    let app_for_page_load = app.clone();
    let perception_for_page_load = state.perception.clone();
    let label_for_page_load = label.clone();
    let tab_id_for_page_load = tab_id.clone();
    let itp_for_dl = state.itp.clone();
    let dl_mgr_for_dl = state.downloads.clone();
    let db_for_dl = state.db.clone();
    let private_download = is_private.unwrap_or(false);
    let app_for_dl = app.clone();

    let guard_script = state.itp.build_network_guard_script();
    let is_dark = state.tabs.dark_mode.load(Ordering::Relaxed);
    let theme_script = build_theme_sync_script(is_dark);

    let builder = WebviewBuilder::new(&label, WebviewUrl::External(parsed))
        .incognito(is_private.unwrap_or(false))
        .on_permission_request(move |webview, permission| {
            let location = webview.url().ok();
            let response = location.as_ref().map(|location| {
                app_for_permissions
                    .state::<SitePermissions>()
                    .response(location, permission, private_for_permissions)
            }).unwrap_or(tauri::webview::PermissionResponse::Deny);
            if response != tauri::webview::PermissionResponse::Allow {
                if !private_for_permissions {
                    diagnostics::record("security", "permission_denied",
                        serde_json::json!({"kind": "webview"}));
                }
                let _ = app_for_permissions.emit("browser:permission-denied", serde_json::json!({
                    "tabId": tab_id_for_permissions,
                    "permission": permission.to_string(),
                    "url": location.map(|url| url.to_string()).unwrap_or_default()
                }));
            }
            response
        })
        .on_new_window(move |popup_url, _features| {
            if matches!(popup_url.scheme(), "https" | "http")
                && popup_url.host_str().is_some()
                && popup_url.username().is_empty()
                && popup_url.password().is_none()
            {
                // The external popup is always denied. Only browser-chrome
                // user interaction may reopen it as an ordinary tab.
                if !private_for_popups {
                    diagnostics::record("security", "popup_blocked",
                        serde_json::json!({"kind": "external_window"}));
                }
                let _ = app_for_popups.emit("browser:popup-blocked", serde_json::json!({
                    "tabId": tab_id_for_popups,
                    "url": popup_url.to_string()
                }));
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .initialization_script(&guard_script)
        .initialization_script(&theme_script)
        .initialization_script(agent::reactive_perception::OBSERVER_SCRIPT)
        .on_page_load(move |_webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                if !private_for_page_load {
                    diagnostics::record("browser", "page_loaded",
                        serde_json::json!({"status": "complete"}));
                }
                perception_for_page_load.invalidate(&label_for_page_load);
                let _ = app_for_page_load.emit(
                    "browser:page-loaded",
                    serde_json::json!({
                        "tabId": tab_id_for_page_load,
                        "url": payload.url().to_string()
                    }),
                );
            }
        })
        .on_navigation(move |nav_url| {
            let allowed = itp_for_nav.allow_navigation(nav_url);
            if !allowed {
                let host = nav_url.host_str().unwrap_or("unknown").to_string();
                let _ = app_for_nav.emit(
                    "itp:blocked",
                    serde_json::json!({
                        "origin": nav_url.to_string(),
                        "blocked_host": host,
                        "resource_type": "navigation",
                        "total_blocked": itp_for_nav.stats().total_blocked
                    }),
                );
            }
            allowed
        })
        .on_download(move |_webview, event| {
            match event {
                DownloadEvent::Requested { url, destination } => {
                    let raw_name = destination
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("download.bin");
                    let clean_name = itp_for_dl.sanitize_filename(raw_name);
                    let safe_dest = dl_mgr_for_dl.resolve_unique_path(&clean_name);
                    *destination = safe_dest.clone();

                    let dl_id = Uuid::new_v4();
                    let rec = DownloadRecord {
                        id: dl_id,
                        url: url.to_string(),
                        filename: safe_dest
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&clean_name)
                            .to_string(),
                        saved_path: safe_dest.to_string_lossy().to_string(),
                        bytes_downloaded: 0,
                        total_bytes: None,
                        status: "downloading".to_string(),
                        error: None,
                    };
                    let _ = app_for_dl.emit("download:progress", &rec);
                    if !private_download {
                        let db_clone = db_for_dl.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut guard = db_clone.lock().await;
                            let _ = guard.upsert_download(&rec);
                        });
                    }
                }
                DownloadEvent::Finished { url, path, success } => {
                    if !private_download {
                        diagnostics::record("downloads", "native_download_finished",
                            serde_json::json!({"status": if success { "completed" } else { "failed" }}));
                    }
                    let saved = path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let fname = path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("download")
                        .to_string();
                    let size = path
                        .as_ref()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len())
                        .unwrap_or(0);
                    let rec = DownloadRecord {
                        id: Uuid::new_v4(),
                        url: url.to_string(),
                        filename: fname,
                        saved_path: saved,
                        bytes_downloaded: size,
                        total_bytes: Some(size),
                        status: if success { "completed" } else { "failed" }.to_string(),
                        error: if success {
                            None
                        } else {
                            Some("WebView2 native download failed".to_string())
                        },
                    };
                    let evt = if success {
                        "download:completed"
                    } else {
                        "download:failed"
                    };
                    let _ = app_for_dl.emit(evt, &rec);
                    if !private_download {
                        let db_clone = db_for_dl.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut guard = db_clone.lock().await;
                            let _ = guard.upsert_download(&rec);
                        });
                    }
                }
                _ => {}
            }
            true
        });

    let child_wv = main_window
        .add_child(
            builder,
            LogicalPosition::new(effective_bounds.x.max(0.0), effective_bounds.y.max(0.0)),
            LogicalSize::new(
                effective_bounds.width.max(40.0),
                effective_bounds.height.max(40.0),
            ),
        )
        .map_err(|e| format!("Failed to attach child WebView2 view: {e}"))?;

    agent::reactive_perception::attach_native_hint_receiver(&child_wv, label.clone(), state.perception.clone())?;
    state.perception.invalidate(&label);
    if effective_bounds.hidden {
        let _ = child_wv.hide();
    }

    Ok(())
}

#[tauri::command]
async fn switch_tab_webview(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    tab_id: String,
    is_external: bool,
    bounds: Option<TabBounds>,
) -> Result<(), String> {
    *state.tabs.active_tab_id.lock().await = tab_id.clone();

    let effective_bounds = if let Some(b) = bounds {
        *state.tabs.bounds.lock().await = b.clone();
        b
    } else {
        state.tabs.bounds.lock().await.clone()
    };

    let target_label = TabManagerState::sanitize_label(&tab_id);
    hide_other_tab_webviews(&app, &state.tabs, &target_label).await;

    let child = app.get_webview(&target_label);
    // The chrome can show New Tab while a previous website is cached underneath.
    let current_url = if is_external {
        child.as_ref().and_then(|view| view.url().ok()).map(|url| url.to_string())
    } else {
        None
    };
    *state.tabs.active_url.lock().await = current_url.unwrap_or_else(|| "browsphere://start".into());
    if let Some(tab_wv) = child {
        if is_external && !effective_bounds.hidden {
            apply_webview_geometry(&tab_wv, &effective_bounds);
        } else {
            let _ = tab_wv.hide();
        }
    }

    Ok(())
}

#[tauri::command]
async fn reload_tab_webview(
    app: tauri::AppHandle,
    tab_id: String,
) -> Result<(), String> {
    let label = TabManagerState::sanitize_label(&tab_id);
    let webview = app.get_webview(&label)
        .ok_or_else(|| "No loaded website to reload".to_string())?;
    webview.eval("window.location.reload()")
        .map_err(|e| format!("Website reload failed: {e}"))
}

#[tauri::command]
async fn close_tab_webview(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    tab_id: String,
) -> Result<(), String> {
    let closing_run = {
        let target = state.active_agent_target.lock().await;
        target.as_ref().filter(|target| target.tab_id == tab_id)
            .map(|target| (target.run_id, target.tab_closed.clone()))
    };
    if let Some((run_id, closed_signal)) = closing_run {
        // Give the worker an explicit terminal reason, even if it is paused.
        closed_signal.cancel();
        if let Some(token) = state.active_cancel.lock().await.as_ref() {
            token.cancel();
        }
        let _ = app.emit("agent:run_error", serde_json::json!({
            "run_id": run_id,
            "message": "Task stopped because its assigned browser tab was closed."
        }));
    }
    let label = {
        let mut map = state.tabs.tab_labels.lock().await;
        map.remove(&tab_id)
            .unwrap_or_else(|| TabManagerState::sanitize_label(&tab_id))
    };
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.close();
    }
    // Do not retain a closed tab's page text or private session observations.
    state.perception.forget(&label);
    Ok(())
}

#[tauri::command]
async fn sync_tab_bounds(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    bounds: TabBounds,
) -> Result<(), String> {
    *state.tabs.bounds.lock().await = bounds.clone();
    let label = state.tabs.active_webview_label().await;
    if let Some(tab_wv) = app.get_webview(&label) {
        apply_webview_geometry(&tab_wv, &bounds);
    }
    Ok(())
}

#[tauri::command]
async fn report_itp_blocked(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    origin: String,
    blocked_host: String,
    resource_type: String,
) -> Result<u64, String> {
    let total = state.itp.record_block(1);
    {
        let mut db = state.db.lock().await;
        let _ = db.record_itp_block(&origin, &blocked_host, &resource_type);
    }
    let _ = app.emit(
        "itp:blocked",
        serde_json::json!({
            "origin": origin,
            "blocked_host": blocked_host,
            "resource_type": resource_type,
            "total_blocked": total
        }),
    );
    Ok(total)
}


#[tauri::command]
async fn get_itp_breakdown(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<ItpHostCount>, String> {
    let db = state.db.lock().await;
    db.get_itp_breakdown(limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_downloads(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.clear_downloads().map_err(|e| e.to_string())
}

#[tauri::command]
async fn has_litellm_api_key(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    let router = state.router.lock().await;
    router.get_provider_api_key(&state.creds)
        .map(|key| key.is_some_and(|s| !s.is_empty()))
        .map_err(|e| e.to_string())
}

fn login_account_key(site: &str, username: &str) -> String {
    format!("login:{}:{}", site.trim().to_lowercase(), username.trim())
}

#[tauri::command]
async fn list_saved_credentials(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SavedCredentialMeta>, String> {
    let db = state.db.lock().await;
    db.list_credentials_meta().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_credential(
    state: tauri::State<'_, Arc<AppState>>,
    site: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let clean_site = site.trim().to_lowercase();
    let clean_user = username.trim().to_string();
    if clean_site.is_empty() || clean_user.is_empty() || password.is_empty() {
        return Err("Site, username, and password are required.".to_string());
    }
    let key = login_account_key(&clean_site, &clean_user);
    state
        .creds
        .set_secret(&key, &password)
        .map_err(|e| e.to_string())?;
    let mut db = state.db.lock().await;
    db.upsert_credential_meta(&clean_site, &clean_user)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_credential_secret(
    state: tauri::State<'_, Arc<AppState>>,
    site: String,
    username: String,
) -> Result<Option<String>, String> {
    let key = login_account_key(&site, &username);
    state.creds.get_secret(&key).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_credential(
    state: tauri::State<'_, Arc<AppState>>,
    site: String,
    username: String,
) -> Result<(), String> {
    let key = login_account_key(&site, &username);
    let _ = state.creds.delete_secret(&key);
    let mut db = state.db.lock().await;
    db.delete_credential_meta(&site, &username)
        .map_err(|e| e.to_string())
}

fn host_matches_site(current_host: &str, saved_site: &str) -> bool {
    let ch = current_host.trim().to_lowercase();
    let ss = saved_site
        .trim()
        .to_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();
    if ch.is_empty() || ss.is_empty() {
        return false;
    }
    ch == ss || ch.ends_with(&format!(".{ss}")) || ss.ends_with(&format!(".{ch}"))
}

#[tauri::command]
async fn autofill_active_tab(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    site: Option<String>,
    username: Option<String>,
) -> Result<String, String> {
    let active_url = state.tabs.active_url.lock().await.clone();
    let current_host = url::Url::parse(&active_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_default();

    let candidates = {
        let db = state.db.lock().await;
        db.list_credentials_meta().map_err(|e| e.to_string())?
    };

    let matched = if let (Some(ref s), Some(ref u)) = (site, username) {
        candidates.into_iter().find(|c| {
            c.site.eq_ignore_ascii_case(s.trim()) && c.username == u.trim()
        })
    } else if !current_host.is_empty() {
        candidates
            .into_iter()
            .find(|c| host_matches_site(&current_host, &c.site))
    } else {
        None
    };

    let cred = matched.ok_or_else(|| {
        if current_host.is_empty() {
            "No external page host active for autofill.".to_string()
        } else {
            format!("No saved credential matches '{current_host}' in Windows Credential Manager.")
        }
    })?;

    let secret = state
        .creds
        .get_secret(&login_account_key(&cred.site, &cred.username))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Credential secret not found in Windows Credential Manager.".to_string())?;

    let label = state.tabs.active_webview_label().await;
    let tab_wv = app
        .get_webview(&label)
        .ok_or_else(|| "Active child WebView2 not found.".to_string())?;

    let user_json = serde_json::to_string(&cred.username).map_err(|e| e.to_string())?;
    let pass_json = serde_json::to_string(&secret).map_err(|e| e.to_string())?;

    let script = format!(
        r#"(function() {{
  const usernameVal = {user_json};
  const passwordVal = {pass_json};

  function setNativeValue(el, val) {{
    if (!el) return false;
    el.focus();
    const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const desc = Object.getOwnPropertyDescriptor(proto, "value");
    if (desc && typeof desc.set === "function") {{
      desc.set.call(el, val);
    }} else {{
      el.value = val;
    }}
    el.dispatchEvent(new Event("input", {{ bubbles: true, cancelable: true }}));
    el.dispatchEvent(new Event("change", {{ bubbles: true, cancelable: true }}));
    return true;
  }}

  const pwInputs = Array.from(document.querySelectorAll('input[type="password"]'))
    .filter(el => !el.disabled && !el.readOnly);
  if (!pwInputs.length) return;

  let filledCount = 0;
  for (const pwEl of pwInputs) {{
    if (setNativeValue(pwEl, passwordVal)) filledCount++;
    const container = pwEl.closest("form") || pwEl.parentElement || document;
    const userEl = container.querySelector(
      'input[type="email"], input[type="text"], input[name*="user" i], input[name*="email" i], input[name*="login" i], input[autocomplete="username"], input[autocomplete="email"]'
    );
    if (userEl && !userEl.disabled && !userEl.readOnly) {{
      if (setNativeValue(userEl, usernameVal)) filledCount++;
    }}
  }}
}})();"#,
        user_json = user_json,
        pass_json = pass_json
    );

    tab_wv.eval(&script).map_err(|e| e.to_string())?;
    Ok(format!("Auto-filled credentials for {} ({})", cred.site, cred.username))
}

#[tauri::command]
async fn get_itp_stats(state: tauri::State<'_, Arc<AppState>>) -> Result<ItpStats, String> {
    Ok(state.itp.stats())
}

#[tauri::command]
async fn start_streaming_download(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    url: String,
    filename: Option<String>,
) -> Result<DownloadRecord, String> {
    state
        .downloads
        .start_streaming_download(
            app,
            state.db.clone(),
            state.itp.clone(),
            url,
            filename,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_download(
    state: tauri::State<'_, Arc<AppState>>,
    download_id: Uuid,
) -> Result<bool, String> {
    Ok(state.downloads.cancel_download(download_id).await)
}

#[tauri::command]
async fn get_downloads(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<DownloadRecord>, String> {
    let db = state.db.lock().await;
    db.get_recent_downloads(limit.unwrap_or(30))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn register_download(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    url: String,
    filename: String,
) -> Result<DownloadRecord, String> {
    state
        .downloads
        .start_streaming_download(
            app,
            state.db.clone(),
            state.itp.clone(),
            url,
            Some(filename),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn submit_dom_snapshot(
    state: tauri::State<'_, Arc<AppState>>,
    request_id: Uuid,
    snapshot: DomSnapshot,
) -> Result<(), String> {
    let sender = {
        let mut map = state.pending_snapshots.lock().await;
        map.remove(&request_id)
    };
    if let Some(tx) = sender {
        let _ = tx.send(snapshot);
    }
    Ok(())
}

#[tauri::command]
async fn submit_action_result(
    state: tauri::State<'_, Arc<AppState>>,
    request_id: Uuid,
    result: ActionResult,
) -> Result<(), String> {
    let sender = {
        let mut map = state.pending_action_results.lock().await;
        map.remove(&request_id)
    };
    if let Some(tx) = sender {
        let _ = tx.send(result);
    }
    Ok(())
}

#[tauri::command]
async fn capture_tab_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    tab_id: String,
) -> Result<Option<DomSnapshot>, String> {
    let label = state.tabs.tab_labels.lock().await.get(&tab_id).cloned()
        .ok_or_else(|| "Browser tab no longer exists".to_string())?;
    let generation = state.perception.generation(&label);
    let snapshot = request_live_dom_snapshot(&app, &label, &state.pending_snapshots, 2000).await;
    if let Some(ref observed) = snapshot {
        state.perception.accept(&label, generation, observed.clone());
    }
    Ok(snapshot)
}

#[tauri::command]
async fn capture_live_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<DomSnapshot>, String> {
    let label = state.tabs.active_webview_label().await;
    let generation = state.perception.generation(&label);
    let snapshot = request_live_dom_snapshot(&app, &label, &state.pending_snapshots, 2000).await;
    if let Some(ref observed) = snapshot { state.perception.accept(&label, generation, observed.clone()); }
    Ok(snapshot)
}


/// Synthetic, local-only WebView2 benchmark. Numbers start BEFORE the native
/// ExecuteScript dispatch, therefore bound (not exactly equal) mutation-to-observation
/// latency. No page-provided timestamps are trusted or used.
#[derive(Debug, Clone, Serialize)]
struct BrprLiveSample {
    mutation_dispatch_to_native_hint_ms: f64,
    mutation_dispatch_to_fresh_observation_ms: f64,
}
#[derive(Debug, Clone, Serialize)]
struct BrprLiveReport {
    attempted: usize,
    samples: Vec<BrprLiveSample>,
    failed: usize,
    detection_p50_ms: Option<f64>,
    detection_p95_ms: Option<f64>,
    detection_p99_ms: Option<f64>,
    observation_p50_ms: Option<f64>,
    observation_p95_ms: Option<f64>,
    observation_p99_ms: Option<f64>,
    metric_note: String,
}
fn bench_percentile(mut values: Vec<f64>, percentile: usize) -> Option<f64> {
    if values.is_empty() { return None; }
    values.sort_by(|a,b| a.total_cmp(b));
    Some(values[((values.len()*percentile+99)/100).saturating_sub(1).min(values.len()-1)])
}

#[tauri::command]
async fn run_brpr_live_benchmark(
    webview: tauri::Webview,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    iterations: Option<usize>,
) -> Result<BrprLiveReport, String> {
    if webview.label() != "main" { return Err("Trusted main view required".into()); }
    let label = state.tabs.active_webview_label().await;
    let child = app.get_webview(&label).ok_or("No native child browser WebView2")?;
    let current = child.url().map_err(|e| e.to_string())?;
    // Never mutate a user's real websites for a synthetic performance test.
    if current.scheme() != "http" || current.host_str() != Some("127.0.0.1")
        || current.port() != Some(18764) || current.path() != "/" {
        return Err("Benchmark requires the isolated localhost:18764 fixture".into());
    }
    let iterations = iterations.unwrap_or(20).clamp(1, 100);
    let mut report = BrprLiveReport {
        attempted: iterations, samples: Vec::new(), failed: 0,
        detection_p50_ms: None, detection_p95_ms: None, detection_p99_ms: None,
        observation_p50_ms: None, observation_p95_ms: None, observation_p99_ms: None,
        metric_note: "Elapsed Rust monotonic time starts before native script dispatch. This is an upper bound including browser scheduling, WebMessage delivery, debounce and DOM refresh, NOT a pure mutation timestamp or LLM response.".into()
    };
    // Readiness check: warm native page cache and avoid counting first load.
    let warm_generation = state.perception.generation(&label);
    if let Some(warm) = request_live_dom_snapshot(&app, &label, &state.pending_snapshots, 2000).await {
        state.perception.accept(&label, warm_generation, warm);
    }
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    for i in 0..iterations {
        let marker = format!("BRPR-LIVE-{}", i);
        let script = format!(r#"(function() {{
            const text = {};
            let el = document.getElementById("brpr-benchmark");
            if (!el) {{
                el = document.createElement("p");
                el.id = "brpr-benchmark";
                document.body.appendChild(el);
            }}
            el.textContent = text;
            return true;
        }})()"#, serde_json::to_string(&marker).unwrap());
        let before = state.perception.status(&label).signal_count;
        let start = std::time::Instant::now();
        let mut detected = None;
        if agent::native_eval::eval_json(&app, &label, script, 2000).await.is_err() {
            report.failed += 1;
            continue;
        }
        let mut observed = None;
        while start.elapsed() < std::time::Duration::from_secs(4) {
            let status = state.perception.status(&label);
            if detected.is_none() && status.signal_count > before {
                detected = Some(start.elapsed().as_secs_f64() * 1000.0);
            }
            if let Some(snapshot) = state.perception.cached_preview(&label, std::time::Duration::from_secs(2)) {
                if snapshot.text_excerpt.contains(&marker) && detected.is_some() {
                    observed = Some(start.elapsed().as_secs_f64() * 1000.0);
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        if let (Some(detection), Some(observation)) = (detected, observed) {
            report.samples.push(BrprLiveSample {
                mutation_dispatch_to_native_hint_ms: detection,
                mutation_dispatch_to_fresh_observation_ms: observation,
            });
        } else {
            report.failed += 1;
        }
        tokio::time::sleep(std::time::Duration::from_millis(35)).await;
    }
    let detections: Vec<f64> = report.samples.iter().map(|s| s.mutation_dispatch_to_native_hint_ms).collect();
    let observations: Vec<f64> = report.samples.iter().map(|s| s.mutation_dispatch_to_fresh_observation_ms).collect();
    report.detection_p50_ms = bench_percentile(detections.clone(), 50);
    report.detection_p95_ms = bench_percentile(detections.clone(), 95);
    report.detection_p99_ms = bench_percentile(detections, 99);
    report.observation_p50_ms = bench_percentile(observations.clone(), 50);
    report.observation_p95_ms = bench_percentile(observations.clone(), 95);
    report.observation_p99_ms = bench_percentile(observations, 99);
    Ok(report)
}

#[tauri::command]
async fn get_brpr_status(webview: tauri::Webview, state: tauri::State<'_, Arc<AppState>>) -> Result<PerceptionStatus, String> {
    if webview.label() != "main" && webview.label() != "ai-chat" { return Err("Trusted app view required".into()); }
    let label = state.tabs.active_webview_label().await;
    Ok(state.perception.status(&label))
}

/// Read-only, very recent observation. Never use this cached preview to
/// authorize browser actions: the agent's executor requires a fresh capture.
#[tauri::command]
async fn get_brpr_cached_observation(
    webview: tauri::Webview,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<DomSnapshot>, String> {
    if webview.label() != "main" && webview.label() != "ai-chat" {
        return Err("Trusted app view required".into());
    }
    let label = state.tabs.active_webview_label().await;
    Ok(state.perception.cached_preview(&label, std::time::Duration::from_millis(250)))
}

#[tauri::command]
async fn record_navigation(
    state: tauri::State<'_, Arc<AppState>>,
    url: String,
    title: Option<String>,
    is_private: Option<bool>,
) -> Result<(), String> {
    if is_private.unwrap_or(false) || contains_sensitive_history_parameter(&url) {
        return Ok(());
    }
    let itp_verdict = state.itp.check_url(&url);
    let mut db = state.db.lock().await;
    db.record_visit(&url, title.as_deref(), itp_verdict.blocked_trackers)
        .map_err(|e| e.to_string())?;
    diagnostics::record("browser", "navigation_recorded",
        serde_json::json!({"blocked_trackers": itp_verdict.blocked_trackers}));
    Ok(())
}

#[tauri::command]
async fn get_aws_config(state: tauri::State<'_, Arc<AppState>>) -> Result<AwsConfig, String> {
    Ok(state.aws_config.lock().await.clone())
}

#[tauri::command]
async fn update_aws_config(state: tauri::State<'_, Arc<AppState>>, config: AwsConfig) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    {
        let mut db = state.db.lock().await;
        db.save_aws_config(&config).map_err(|e| e.to_string())?;
    }
    state.router.lock().await.update_aws_config(config.clone());
    *state.aws_config.lock().await = config;
    *state.cognito_session.lock().await = None; // Changed app client/profile invalidates previous sign-in.
    Ok(())
}

#[tauri::command]
async fn aws_connect_account(
    state: tauri::State<'_, Arc<AppState>>,
    profile: String,
) -> Result<AwsConnectResult, String> {
    // The AWS CLI handles temporary credentials in its own secure credential chain.
    // After sign-in only perform read-only resource discovery, then persist nonsecret IDs.
    let current = state.aws_config.lock().await.clone();
    let result = aws_discovery::connect(current, &profile).await.map_err(|e| e.to_string())?;
    {
        let mut db = state.db.lock().await;
        db.save_aws_config(&result.config).map_err(|e| e.to_string())?;
    }
    state.router.lock().await.update_aws_config(result.config.clone());
    *state.aws_config.lock().await = result.config.clone();
    *state.cognito_session.lock().await = None;
    Ok(result)
}

#[tauri::command]
async fn aws_test_connection(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let cfg = state.aws_config.lock().await.clone();
    aws::test_identity(&cfg).await.map_err(|e| e.to_string())
}

async fn cloud_snapshot(state: &Arc<AppState>) -> Result<CloudSnapshot, String> {
    let router = state.router.lock().await.get_config();
    let memories = state.db.lock().await.list_memories(None, None, 200)
        .map_err(|e| e.to_string())?
        .into_iter().map(|m| MemoryMeta {
            kind: m.kind, scope: m.scope, updated_at: m.updated_at, evidence_count: m.evidence_count
        }).collect();
    Ok(CloudSnapshot::new(router, memories))
}

async fn apply_cloud_snapshot(state: &Arc<AppState>, snapshot: CloudSnapshot) -> Result<String, String> {
    snapshot.validate().map_err(|e| e.to_string())?;
    for endpoint in [&snapshot.router.ollama_url, &snapshot.router.litellm_url] {
        let parsed = url::Url::parse(endpoint).map_err(|_| "Invalid endpoint in backup")?;
        let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && local) {
            return Err("Backup contained an insecure endpoint".into());
        }
    }
    let mut router = snapshot.router;
    router.litellm_api_key = None;
    state.db.lock().await.save_router_config(&router).map_err(|e| e.to_string())?;
    state.router.lock().await.update_config(router);
    Ok("Cloud AI preferences restored locally. Saved API keys and agent memories were not modified.".into())
}

#[tauri::command]
async fn aws_backup_s3(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let cfg = state.aws_config.lock().await.clone();
    let snapshot = cloud_snapshot(&state).await?;
    aws::s3_backup(&cfg, &snapshot).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn aws_restore_s3(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let cfg = state.aws_config.lock().await.clone();
    let snapshot = aws::s3_restore(&cfg).await.map_err(|e| e.to_string())?;
    apply_cloud_snapshot(&state, snapshot).await
}

#[tauri::command]
async fn aws_sync_dynamodb(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let cfg = state.aws_config.lock().await.clone();
    let snapshot = cloud_snapshot(&state).await?;
    aws::dynamodb_push(&cfg, &snapshot).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn aws_pull_dynamodb(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let cfg = state.aws_config.lock().await.clone();
    let snapshot = aws::dynamodb_pull(&cfg).await.map_err(|e| e.to_string())?;
    apply_cloud_snapshot(&state, snapshot).await
}

#[tauri::command]
async fn aws_cognito_sign_in(state: tauri::State<'_, Arc<AppState>>, username: String, password: String)
    -> Result<CognitoSession, String> {
    let cfg = state.aws_config.lock().await.clone();
    let session = aws::cognito_sign_in(&cfg, &username, &password).await.map_err(|e| e.to_string())?;
    *state.cognito_session.lock().await = Some(session.clone());
    Ok(session)
}

#[tauri::command]
async fn aws_cognito_sign_out(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    *state.cognito_session.lock().await = None;
    Ok(())
}

#[tauri::command]
async fn aws_cognito_session(state: tauri::State<'_, Arc<AppState>>) -> Result<Option<CognitoSession>, String> {
    let mut guard = state.cognito_session.lock().await;
    if let Some(session) = guard.as_ref() {
        let active = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
            .map(|end| end > chrono::Utc::now()).unwrap_or(false);
        if active { return Ok(Some(session.clone())); }
    }
    *guard = None;
    Ok(None)
}

#[tauri::command]
async fn aws_invoke_lambda(state: tauri::State<'_, Arc<AppState>>, payload: String) -> Result<String, String> {
    let cfg = state.aws_config.lock().await.clone();
    aws::invoke_lambda(&cfg, &payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_router_config(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<RouterConfig, String> {
    let router = state.router.lock().await;
    Ok(router.get_config())
}

#[tauri::command]
async fn update_router_config(
    state: tauri::State<'_, Arc<AppState>>,
    config: RouterConfig,
) -> Result<(), String> {
    for endpoint in [&config.ollama_url, &config.litellm_url] {
        let parsed = url::Url::parse(endpoint).map_err(|e| format!("Invalid model endpoint: {e}"))?;
        let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && local) {
            return Err("Model endpoints must use HTTPS unless running on localhost".into());
        }
    }
    if config.ollama_model.trim().is_empty() || config.litellm_model.trim().is_empty() {
        return Err("Both model fields must have a model ID".into());
    }
    if let Some(ref api_key) = config.litellm_api_key {
        if !api_key.is_empty() {
            state
                .creds
                .set_secret(&ModelRouter::api_key_account(&config.litellm_url).map_err(|e| e.to_string())?, api_key)
                .map_err(|e| e.to_string())?;
        }
    }
    {
        let mut db = state.db.lock().await;
        db.save_router_config(&config).map_err(|e| e.to_string())?;
    }
    let mut router = state.router.lock().await;
    let mut sanitized = config;
    sanitized.litellm_api_key = None;
    router.update_config(sanitized);
    Ok(())
}

#[tauri::command]
async fn route_lucia_intent(
    state: tauri::State<'_, Arc<AppState>>,
    message: String,
) -> Result<agent::intent_router::LuciaIntent, String> {
    // A clear, explicit user navigation command is not ambiguous intent.
    // Do not turn "open youtube" into chat when the configured intent model
    // is unavailable, returns malformed JSON, or classifies it incorrectly.
    if agent::policy::requested_navigation_url(&message).is_some() {
        return Ok(agent::intent_router::LuciaIntent {
            mode: agent::intent_router::LuciaMode::BrowserTask,
            reason: "Explicit website navigation requested by the user".into(),
        });
    }
    let router = state.router.lock().await;
    router.route_lucia_intent(&message, &state.creds)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn ask_main_tab_ai(
    state: tauri::State<'_, Arc<AppState>>,
    messages: Vec<ChatTurn>,
    voice_mode: Option<bool>,
) -> Result<ChatAnswer, String> {
    let started = std::time::Instant::now();
    let router = state.router.lock().await;
    let result = router.chat_completion(&messages, &state.creds, voice_mode.unwrap_or(false))
        .await.map_err(|e| e.to_string());
    diagnostics::record("model", "chat_completion", serde_json::json!({
        "status": if result.is_ok() { "ok" } else { "failed" },
        "duration_ms": started.elapsed().as_millis() as u64,
        "count": messages.len(),
        "kind": if voice_mode.unwrap_or(false) { "voice" } else { "text" }
    }));
    if result.is_err() {
        diagnostics::record("errors", "model_failure", serde_json::json!({
            "error_kind": "chat_completion"
        }));
    }
    result
}

#[derive(Serialize)]
struct CloudModelsResult {
    endpoint: String,
    models: Vec<String>,
    selected_model: String,
}

#[tauri::command]
async fn fetch_cloud_models(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<CloudModelsResult, String> {
    // The credentials and endpoint must have been saved by the settings page first.
    let (endpoint, models) = {
        let router = state.router.lock().await;
        let endpoint = router.get_config().litellm_url;
        let models = router.fetch_provider_models(&state.creds).await.map_err(|e| e.to_string())?;
        (endpoint, models)
    };
    let mut config = state.router.lock().await.get_config();
    if config.litellm_url != endpoint {
        return Err("Provider changed while fetching models. Try Fetch Models again.".into());
    }
    config.cloud_model_catalogs.insert(endpoint.clone(), models.clone());
    if !models.contains(&config.litellm_model) {
        config.litellm_model = models[0].clone();
    }
    // Persist only model IDs and public endpoint data. Secrets stay in Credential Manager.
    state.db.lock().await.save_router_config(&config).map_err(|e| e.to_string())?;
    state.router.lock().await.update_config(config.clone());
    Ok(CloudModelsResult {
        endpoint,
        models,
        selected_model: config.litellm_model,
    })
}

#[tauri::command]
async fn test_model_connection(
    state: tauri::State<'_, Arc<AppState>>,
    backend: Option<String>,
) -> Result<String, String> {
    let router = state.router.lock().await;
    router.test_connection(&state.creds, backend.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_model_api_key(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let router = state.router.lock().await;
    let config = router.get_config();
    let account = ModelRouter::api_key_account(&config.litellm_url).map_err(|e| e.to_string())?;
    state.creds.delete_secret(&account).map_err(|e| e.to_string())?;
    if config.litellm_url == "http://127.0.0.1:4000/v1/chat/completions" {
        state.creds.delete_secret("litellm_api_key").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn list_local_models(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    let router = state.router.lock().await;
    router.list_local_models().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_history(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<HistoryEntry>, String> {
    let db = state.db.lock().await;
    db.get_history(limit.unwrap_or(50)).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_history(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_memories(state: tauri::State<'_, Arc<AppState>>, scope: Option<String>, query: Option<String>) -> Result<Vec<MemoryEntry>, String> {
    let db = state.db.lock().await;
    db.list_memories(scope.as_deref(), query.as_deref(), 200).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_memory(state: tauri::State<'_, Arc<AppState>>, id: String) -> Result<bool, String> {
    let mut db = state.db.lock().await;
    db.delete_memory(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_memories(state: tauri::State<'_, Arc<AppState>>) -> Result<usize, String> {
    let mut db = state.db.lock().await;
    db.clear_memories().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recent_runs(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<AgentRunSummary>, String> {
    let db = state.db.lock().await;
    db.get_recent_runs(limit.unwrap_or(25))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_traces_filtered(
    state: tauri::State<'_, Arc<AppState>>,
    run_id: Option<String>,
    risk: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<TraceEntry>, String> {
    let db = state.db.lock().await;
    db.get_traces_filtered(run_id.as_deref(), risk.as_deref(), limit.unwrap_or(80))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn resolve_approval(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    approval_id: Uuid,
    approved: bool,
) -> Result<(), String> {
    let sender = {
        let mut map = state.pending_approvals.lock().await;
        map.remove(&approval_id)
    }.ok_or_else(|| "This approval is no longer pending".to_string())?;
    sender.send(approved)
        .map_err(|_| "This approval has expired or its task was cancelled".to_string())?;
    let _ = app.emit("agent:approval_closed", serde_json::json!({
        "approval_id": approval_id,
        "reason": if approved { "approved" } else { "rejected" }
    }));
    Ok(())
}

/// Remove approvals whose receivers were dropped when a task was cancelled,
/// completed, or timed out. A closed sender must never remain actionable.
async fn prune_closed_approvals(
    app: &tauri::AppHandle,
    pending: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<bool>>>>,
) {
    let expired = {
        let mut map = pending.lock().await;
        let expired: Vec<Uuid> = map.iter()
            .filter(|(_, tx)| tx.is_closed())
            .map(|(id, _)| *id).collect();
        for id in &expired { map.remove(id); }
        expired
    };
    for approval_id in expired {
        let _ = app.emit("agent:approval_closed", serde_json::json!({
            "approval_id": approval_id, "reason": "task_finished_or_expired"
        }));
    }
}

#[tauri::command]
async fn set_agent_paused(
    state: tauri::State<'_, Arc<AppState>>,
    paused: bool,
) -> Result<(), String> {
    // Persist the pause flag even when the first agent has not subscribed yet.
    state.pause_tx.send_replace(paused);
    Ok(())
}

#[tauri::command]
async fn cancel_agent_run(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Some(token) = state.active_cancel.lock().await.take() {
        token.cancel();
    }
    Ok(())
}


#[tauri::command]
async fn list_workflow_macros(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<WorkflowMacro>, String> {
    let db = state.db.lock().await;
    db.list_workflow_macros(limit.unwrap_or(30))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_workflow_macro(
    state: tauri::State<'_, Arc<AppState>>,
    macro_id: String,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.delete_workflow_macro(&macro_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn replay_workflow_macro(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    macro_id: String,
    snapshot: DomSnapshot,
    allowed_domains: Vec<String>,
) -> Result<Uuid, String> {
    let wf = {
        let db = state.db.lock().await;
        db.get_workflow_macro_by_id(&macro_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Workflow macro not found".to_string())?
    };

    let mut snap = snapshot;
    if snap.origin.is_empty() || snap.origin.starts_with("browsphere://") {
        snap.origin = format!("https://{}", wf.origin_host);
    }

    start_agent_run(app, state, wf.goal_display, snap, allowed_domains, Some(20), Some(false), Some(false), Some(false), None).await
}

#[tauri::command]
async fn get_recent_traces(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<TraceEntry>, String> {
    let db = state.db.lock().await;
    db.get_recent_traces(limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}


#[tauri::command]
async fn list_automations(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<AutomationJob>, String> {
    let db = state.db.lock().await;
    db.list_automation_jobs(limit.unwrap_or(40))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_automation_job(
    state: tauri::State<'_, Arc<AppState>>,
    id: Option<String>,
    name: String,
    trigger: String,
    interval_secs: Option<u64>,
    enabled: bool,
    steps: Vec<AutomationStep>,
) -> Result<AutomationJob, String> {
    let mut db = state.db.lock().await;
    db.upsert_automation_job(id, &name, &trigger, interval_secs, enabled, &steps)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_automation_job(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.set_automation_enabled(&id, enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_automation_job(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.delete_automation_job(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_automation_now(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<String, String> {
    let job = {
        let db = state.db.lock().await;
        db.get_automation_job(&id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Automation workflow not found".to_string())?
    };

    let (approval_tx, mut approval_rx) = mpsc::channel::<ApprovalRequest>(16);
    let (trace_tx, mut trace_rx) = mpsc::channel::<TraceEntry>(64);

    let pending_map = state.pending_approvals.clone();
    let app_a = app.clone();
    tokio::spawn(async move {
        while let Some(req) = approval_rx.recv().await {
            let aid = req.approval_id;
            {
                let mut m = pending_map.lock().await;
                m.insert(aid, req.responder);
            }
            let _ = app_a.emit(
                "agent:approval_required",
                serde_json::json!({
                    "approval_id": aid,
                    "run_id": req.run_id,
                    "trace_id": req.trace_id,
                    "action": req.action,
                    "risk": req.risk,
                    "reason": req.reason
                }),
            );
        }
    });

    let db_t = state.db.clone();
    let app_t = app.clone();
    tokio::spawn(async move {
        while let Some(entry) = trace_rx.recv().await {
            {
                let mut g = db_t.lock().await;
                let _ = g.insert_trace(&entry);
            }
            let _ = app_t.emit("agent:trace", &entry);
        }
    });

    let label = state.tabs.active_webview_label().await;
    let engine = state.automation.clone();
    let router = state.router.clone();
    let creds = state.creds.clone();
    let itp = state.itp.clone();
    let downloads = state.downloads.clone();
    let db = state.db.clone();
    let active_url = state.tabs.active_url.clone();
    let snapshots = state.pending_snapshots.clone();
    let results = state.pending_action_results.clone();

    tokio::spawn(async move {
        let _ = engine
            .run_automation_job(
                app,
                job,
                router,
                creds,
                itp,
                downloads,
                db,
                label,
                active_url,
                snapshots,
                results,
                approval_tx,
                trace_tx,
            )
            .await;
    });

    Ok("started".to_string())
}

#[tauri::command]
async fn stop_automation_job(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<bool, String> {
    Ok(state.automation.stop_job(&id).await)
}

#[tauri::command]
async fn start_agent_run(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    goal: String,
    snapshot: DomSnapshot,
    allowed_domains: Vec<String>,
    max_steps: Option<u32>,
    allow_learning: Option<bool>,
    is_private: Option<bool>,
    voice_mode: Option<bool>,
    task_tab_id: Option<String>,
) -> Result<Uuid, String> {
    let private_mode = is_private.unwrap_or(false);
    let memory_enabled = allow_learning.unwrap_or(false) && !private_mode;

    let dedicated = task_tab_id.is_some();
    let tab_id = match task_tab_id {
        Some(id) if id.starts_with("tab-lucia-")
            && id.len() <= 100
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => id,
        Some(_) => return Err("Invalid Lucia task tab identifier".into()),
        None => state.tabs.active_tab_id.lock().await.clone(),
    };
    let agent_label = TabManagerState::sanitize_label(&tab_id);
    let child = app.get_webview(&agent_label);
    let on_start_page = !dedicated && state.tabs.active_url.lock().await.starts_with("browsphere://");
    let snapshot = if dedicated {
        if child.is_some() {
            return Err("Lucia task tab identifier already exists".into());
        }
        let start_url = if snapshot.origin.starts_with("http://")
            || snapshot.origin.starts_with("https://") {
            snapshot.origin.clone()
        } else {
            "about:blank".into()
        };
        let mut bounds = state.tabs.bounds.lock().await.clone();
        bounds.hidden = true;
        open_or_navigate_tab_inner(
            app.clone(), app.state::<Arc<AppState>>(), tab_id.clone(),
            start_url.clone(), Some(bounds), Some(private_mode), false,
        ).await?;
        if start_url == "about:blank" {
            DomSnapshot {
                origin: "about:blank".into(), title: String::new(),
                text_excerpt: String::new(), interactive_nodes: Vec::new(),
            }
        } else {
            snapshot
        }
    } else if on_start_page || child.is_none() {
        // Start with an empty document. The first real destination, including a
        // direct user command, goes through the ordinary action and evidence gates.
        if let Some(previous_page) = child {
            previous_page.close().map_err(|e| format!("Could not reset the task tab: {e}"))?;
            state.perception.forget(&agent_label);
        }
        let mut bounds = state.tabs.bounds.lock().await.clone();
        bounds.hidden = true;
        open_or_navigate_tab(
            app.clone(),
            app.state::<Arc<AppState>>(),
            tab_id.clone(),
            "about:blank".into(),
            Some(bounds),
            Some(private_mode),
        ).await?;
        DomSnapshot {
            origin: "about:blank".into(),
            title: String::new(),
            text_excerpt: String::new(),
            interactive_nodes: Vec::new(),
        }
    } else {
        snapshot
    };
    if app.get_webview(&agent_label).is_none() {
        return Err("Lucia could not attach to the active browser WebView2 tab.".into());
    }

    if let Some(prev) = state.active_cancel.lock().await.take() {
        prev.cancel();
    }

    let run_id = Uuid::new_v4();
    if !private_mode {
        diagnostics::record("agent", "run_started", serde_json::json!({
            "run_id": run_id.to_string(), "count": max_steps.unwrap_or(20)
        }));
    }
    let cancel_token = CancellationToken::new();
    let tab_closed = CancellationToken::new();
    *state.active_cancel.lock().await = Some(cancel_token.clone());
    *state.active_agent_target.lock().await = Some(ActiveAgentTarget {
        tab_id, run_id, tab_closed: tab_closed.clone(),
    });

    let (approval_tx, mut approval_rx) = mpsc::channel::<ApprovalRequest>(16);
    let (trace_tx, mut trace_rx) = mpsc::channel::<TraceEntry>(64);

    if !private_mode {
        let mut db = state.db.lock().await;
        db.create_agent_run(&run_id.to_string(), &goal)
            .map_err(|e| e.to_string())?;
    }

    let pending_map = state.pending_approvals.clone();
    let app_for_approval = app.clone();
    tokio::spawn(async move {
        while let Some(req) = approval_rx.recv().await {
            let approval_id = req.approval_id;
            {
                let mut map = pending_map.lock().await;
                map.insert(approval_id, req.responder);
            }
            let _ = app_for_approval.emit(
                "agent:approval_required",
                serde_json::json!({
                    "approval_id": approval_id,
                    "run_id": req.run_id,
                    "trace_id": req.trace_id,
                    "action": req.action,
                    "risk": req.risk,
                    "reason": req.reason
                }),
            );
        }
    });

    let db_for_traces = state.db.clone();
    let app_for_traces = app.clone();
    tokio::spawn(async move {
        while let Some(entry) = trace_rx.recv().await {
            // Never keep agent traces (including user goals) from Private Browsing.
            if !private_mode {
                let action = match &entry.action {
                    agent::policy::BrowserAction::Navigate { .. } => "navigate",
                    agent::policy::BrowserAction::Click { .. } => "click",
                    agent::policy::BrowserAction::TypeText { .. } => "type_text",
                    agent::policy::BrowserAction::PressKey { .. } => "press_key",
                    agent::policy::BrowserAction::Scroll { .. } => "scroll",
                    agent::policy::BrowserAction::Download { .. } => "download",
                    agent::policy::BrowserAction::Complete { .. } => "complete",
                };
                let decision = match &entry.decision {
                    agent::policy::PolicyDecision::Allow => "allow",
                    agent::policy::PolicyDecision::Deny { .. } => "deny",
                    agent::policy::PolicyDecision::RequireApproval { .. } => "approval",
                };
                diagnostics::record("agent", "action_trace", serde_json::json!({
                    "run_id": entry.run_id.to_string(),
                    "trace_id": entry.trace_id.to_string(),
                    "step": entry.step_index,
                    "action": action,
                    "risk": format!("{:?}", entry.risk),
                    "decision": decision,
                    "model_route": if entry.model_route.contains("macro:") { "macro" } else { "agent_model" },
                    "latency_ms": entry.latency_ms,
                    "outcome": diagnostics::classify_agent_outcome(&entry.outcome)
                }));
                let mut db = db_for_traces.lock().await;
                let _ = db.insert_trace(&entry);
            }
            let _ = app_for_traces.emit("agent:trace", &entry);
        }
    });

    let router_arc = state.router.clone();
    let creds_arc = state.creds.clone();
    let itp_arc = state.itp.clone();
    let downloads_arc = state.downloads.clone();
    let db_arc = state.db.clone();
    let active_webview_label = agent_label;
    // The task's navigation state is independent of whichever tab is visible.
    let active_url_arc = Arc::new(Mutex::new(snapshot.origin.clone()));
    let snapshots_arc = state.pending_snapshots.clone();
    let action_results_arc = state.pending_action_results.clone();
    let app_for_loop = app.clone();
    let mut pause_rx_combined = state.pause_tx.subscribe();

    let db_for_status = state.db.clone();
    let app_for_status = app.clone();
    let approvals_for_status = state.pending_approvals.clone();
    let target_for_status = state.active_agent_target.clone();
    tokio::spawn(async move {
        let result = run_react_loop(
            app_for_loop,
            run_id,
            goal,
            snapshot,
            allowed_domains,
            max_steps.unwrap_or(20),
            cancel_token,
            &mut pause_rx_combined,
            router_arc,
            creds_arc,
            itp_arc,
            downloads_arc,
            db_arc,
            active_webview_label,
            active_url_arc,
            snapshots_arc,
            action_results_arc,
            approval_tx,
            trace_tx,
            memory_enabled,
            voice_mode.unwrap_or(false),
        )
        .await;

        let result = close_aware_agent_result(result, tab_closed.is_cancelled());
        let final_status = match result {
            Ok(status) => status,
            Err(err) => {
                let _ = app_for_status.emit("agent:run_error", serde_json::json!({
                    "run_id": run_id,
                    "message": err.to_string().chars().take(400).collect::<String>()
                }));
                "failed".to_string()
            }
        };
        if !private_mode {
            diagnostics::record("agent", "run_finished", serde_json::json!({
                "run_id": run_id.to_string(), "status": final_status
            }));
            if final_status == "failed" || final_status == "halted" {
                diagnostics::record("errors", "agent_failure", serde_json::json!({
                    "run_id": run_id.to_string(), "error_kind": "agent_run"
                }));
            }
        }
        // run_react_loop has returned, dropping any unresolved approval
        // receivers. Close the corresponding pending UI controls immediately.
        prune_closed_approvals(&app_for_status, &approvals_for_status).await;
        {
            let mut target = target_for_status.lock().await;
            if target.as_ref().is_some_and(|target| target.run_id == run_id) {
                *target = None;
            }
        }
        if !private_mode {
            let mut db = db_for_status.lock().await;
            let _ = db.update_agent_run_status(&run_id.to_string(), &final_status);
        }
        let _ = app_for_status.emit(
            "agent:run_status",
            serde_json::json!({
                "run_id": run_id,
                "status": final_status
            }),
        );
    });

    Ok(run_id)
}

// A closed assigned task tab must never be reported as a successful or generic
// cancelled run. Preserve other statuses when the tab remains open.
fn close_aware_agent_result(
    result: Result<String, anyhow::Error>,
    tab_closed: bool,
) -> Result<String, anyhow::Error> {
    if tab_closed { Ok("tab_closed".to_owned()) } else { result }
}

#[cfg(test)]
mod tab_closed_tests {
    use super::close_aware_agent_result;

    #[test]
    fn closing_a_task_tab_overrides_a_cancelled_or_failed_run() {
        let cancelled = close_aware_agent_result(Ok("cancelled".into()), true).unwrap();
        assert_eq!(cancelled, "tab_closed");
        let failed = close_aware_agent_result(Err(anyhow::anyhow!("lost WebView")), true).unwrap();
        assert_eq!(failed, "tab_closed");
    }

    #[test]
    fn switching_tabs_preserves_the_actual_run_status() {
        let completed = close_aware_agent_result(Ok("completed".into()), false).unwrap();
        assert_eq!(completed, "completed");
        assert!(close_aware_agent_result(Err(anyhow::anyhow!("failed")), false).is_err());
    }
}

fn write_startup_log(_data_dir: &std::path::Path, msg: &str) {
    // Avoid stderr in release GUI processes, and never persist arbitrary text.
    #[cfg(debug_assertions)]
    eprintln!("[Browsphere Startup] {msg}");
    if msg.starts_with("Data dir:") {
        diagnostics::record("runtime", "startup_ready", serde_json::json!({
            "component": "browsphere"
        }));
    } else {
        diagnostics::record("errors", "startup_warning", serde_json::json!({
            "error_kind": if msg.contains("SQLite") { "sqlite_open" } else { "startup" }
        }));
    }
}

#[tauri::command]
fn open_diagnostics_folder() -> Result<String, String> {
    let path = diagnostics::folder(&resolve_data_dir());
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    std::process::Command::new("explorer.exe").arg(&path).spawn()
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn record_ui_diagnostic(kind: String) {
    if matches!(kind.as_str(), "js_error" | "unhandled_rejection" | "bridge_error") {
        diagnostics::record("ui", "frontend_failure", serde_json::json!({"error_kind": kind}));
    }
}

#[tauri::command]
fn export_diagnostics() -> Result<String, String> {
    let output = diagnostics::export(&resolve_data_dir()).map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().to_string())
}

#[tauri::command]
fn read_lucia_diagnostics() -> Vec<serde_json::Value> {
    diagnostics::read_lucia_timeline(&resolve_data_dir())
}

fn main() {
    let data_dir = resolve_data_dir();
    diagnostics::init(&data_dir);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(diagnostics::RuntimeMetadataLayer);
    // Console output is only useful in debug builds; in a GUI release it can
    // fail when no stderr handle exists. Metadata still goes to JSONL logs.
    #[cfg(debug_assertions)]
    subscriber.with(tracing_subscriber::fmt::layer()).init();
    #[cfg(not(debug_assertions))]
    subscriber.init();
    let site_permissions = SitePermissions::new(&data_dir);
    write_startup_log(&data_dir, &format!("Data dir: {}", data_dir.to_string_lossy()));

    let db_path = data_dir.join("Browsphere.sqlite");
    let db = match BrowserDatabase::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            write_startup_log(&data_dir, &format!("Primary SQLite WAL open failed ({e}), falling back to in-memory DB"));
            BrowserDatabase::open(std::path::Path::new(":memory:"))
                .expect("failed to open fallback in-memory SQLite DB")
        }
    };

    let creds = CredentialStore::new("BrowsphereAI");
    let initial_router_cfg = db.load_router_config().ok().flatten().unwrap_or_default();
    let initial_aws_cfg = db.load_aws_config().ok().flatten().unwrap_or_default();
    let mut router = ModelRouter::new(initial_router_cfg);
    router.update_aws_config(initial_aws_cfg.clone());
    let itp = ItpEngine::new();
    if let Ok(persisted_blocks) = db.get_total_itp_blocked() {
        if persisted_blocks > 0 {
            itp.record_block(persisted_blocks);
        }
    }
    let downloads = DownloadManager::new(&data_dir);
    let automation = Arc::new(AutomationEngine::new(&data_dir));
    let speech = Arc::new(SpeechRuntimeState { tts_child: Arc::new(Mutex::new(None)) });
    let (pause_tx, _) = watch::channel(false);
    let (perception, perception_rx) = ReactivePerception::new();

    let app_state = Arc::new(AppState {
        db: Arc::new(Mutex::new(db)),
        creds: Arc::new(creds),
        router: Arc::new(Mutex::new(router)),
        aws_config: Arc::new(Mutex::new(initial_aws_cfg)),
        cognito_session: Arc::new(Mutex::new(None)),
        itp: Arc::new(itp),
        downloads: Arc::new(downloads),
        speech,
        automation: automation.clone(),
        tabs: TabManagerState::new(),
        perception,
        preview_cache: Arc::new(Mutex::new(HashMap::new())),
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_snapshots: Arc::new(Mutex::new(HashMap::new())),
        pending_action_results: Arc::new(Mutex::new(HashMap::new())),
        active_cancel: Arc::new(Mutex::new(None)),
        active_agent_target: Arc::new(Mutex::new(None)),
        pause_tx,
    });

    let state_for_sched = app_state.clone();
    let auto_for_sched = automation;

            match tauri::Builder::default()
        .setup(move |app| {
            tauri::async_runtime::spawn(agent::reactive_perception::run_refresh_worker(
                app.handle().clone(), state_for_sched.perception.clone(), perception_rx
            ));
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            } else {
                diagnostics::record("errors", "main_window_missing",
                    serde_json::json!({"error_kind": "main_window"}));
            }
            AutomationEngine::spawn_scheduler(
                auto_for_sched,
                app.handle().clone(),
                state_for_sched.db.clone(),
                state_for_sched.router.clone(),
                state_for_sched.creds.clone(),
                state_for_sched.itp.clone(),
                state_for_sched.downloads.clone(),
                state_for_sched.tabs.clone(),
                state_for_sched.pending_approvals.clone(),
                state_for_sched.pending_snapshots.clone(),
                state_for_sched.pending_action_results.clone(),
                state_for_sched.pause_tx.clone(),
            );
            Ok(())
        })
        .manage(app_state)
        .manage(site_permissions)
        .invoke_handler(tauri::generate_handler![
            open_diagnostics_folder,
            export_diagnostics,
            read_lucia_diagnostics,
            record_ui_diagnostic,
            window_close,
            window_minimize,
            window_toggle_maximize,
            window_start_dragging,
            toggle_ai_chat_window,
            dock_ai_chat_window,
            sync_ai_chat_context,
            ai_chat_start_dragging,
            ai_chat_close,
            ai_chat_open_url,
            fetch_url_preview,
            set_app_theme,
            open_or_navigate_tab,
            get_brpr_status,
            get_brpr_cached_observation,
            run_brpr_live_benchmark,
            get_site_permissions,
            set_site_permission,
            switch_tab_webview,
            reload_tab_webview,
            close_tab_webview,
            capture_tab_snapshot,
            sync_tab_bounds,
            report_itp_blocked,
            get_itp_stats,
            get_itp_breakdown,
            clear_downloads,
            has_litellm_api_key,
            list_saved_credentials,
            save_credential,
            get_credential_secret,
            delete_credential,
            autofill_active_tab,
            speak_text,
            stop_speaking,
            voice::get_voice_config,
            voice::update_voice_config,
            voice::has_voice_api_key,
            voice::set_voice_api_key,
            voice::clear_voice_api_key,
            voice::synthesize_voice,
            listen_speech_once,
            run_policy_load_test,
            check_for_updates,
            start_streaming_download,
            cancel_download,
            get_downloads,
            register_download,
            submit_dom_snapshot,
            submit_action_result,
            capture_live_snapshot,
            record_navigation,
            get_router_config,
            get_aws_config,
            update_aws_config,
            aws_connect_account,
            aws_test_connection,
            aws_backup_s3,
            aws_restore_s3,
            aws_sync_dynamodb,
            aws_pull_dynamodb,
            aws_cognito_sign_in,
            aws_cognito_sign_out,
            aws_cognito_session,
            aws_invoke_lambda,
            update_router_config,
            ask_main_tab_ai,
            route_lucia_intent,
            test_model_connection,
            fetch_cloud_models,
            remove_model_api_key,
            list_local_models,
            get_history,
            clear_history,
            list_memories,
            delete_memory,
            clear_memories,
            get_recent_runs,
            get_traces_filtered,
            resolve_approval,
            set_agent_paused,
            cancel_agent_run,
            list_workflow_macros,
            delete_workflow_macro,
            replay_workflow_macro,
            get_recent_traces,
            list_automations,
            save_automation_job,
            toggle_automation_job,
            delete_automation_job,
            run_automation_now,
            stop_automation_job,
            start_agent_run
        ])
        .run(tauri::generate_context!()) {
            Ok(()) => {}
            Err(e) => {
                write_startup_log(&data_dir, &format!("Tauri runtime exit error: {e}"));
            }
        }
}
