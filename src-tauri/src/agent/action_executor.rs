use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use crate::agent::policy::BrowserAction;
use crate::privacy::downloads::DownloadManager;
use crate::privacy::itp::ItpEngine;
use crate::storage::sqlite_wal::BrowserDatabase;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub detail: String,
}

fn build_click_script(request_id: Uuid, node_id: &str) -> String {
    let id_json = serde_json::to_string(node_id).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"(function() {{
  const REQUEST_ID = "{request_id}";
  const TARGET_ID = {id_json};

  function postResult(success, detail) {{
    // The trusted host receives this value from WebView2 ExecuteScript.
    return {{ success, detail }};
  }}

  function findByAetherId(root, id) {{
    if (!root) return null;
    const direct = root.querySelector(`[data-aether-id="${{CSS.escape(id)}}"]`);
    if (direct) return direct;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    let node = walker.nextNode();
    while (node) {{
      if (node.shadowRoot) {{
        const found = findByAetherId(node.shadowRoot, id);
        if (found) return found;
      }}
      node = walker.nextNode();
    }}
    return null;
  }}

  try {{
    const el = findByAetherId(document, TARGET_ID);
    if (!el) {{
      return postResult(false, `Element ${{TARGET_ID}} not found in DOM`);
      return;
    }}
    // A disabled control ignores .click(), which must not count as success.
    // Check immediately before dispatch because the DOM may have changed
    // since the model's last observation.
    if (el.matches(":disabled") || el.getAttribute("aria-disabled") === "true") {{
      return postResult(false, `Control ${{TARGET_ID}} is disabled`);
    }}
    el.scrollIntoView({{ block: "center", inline: "nearest", behavior: "instant" }});
    const rect = el.getBoundingClientRect();
    const cx = Math.round(rect.left + rect.width / 2);
    const cy = Math.round(rect.top + rect.height / 2);

    const common = {{ bubbles: true, cancelable: true, clientX: cx, clientY: cy, button: 0 }};
    el.dispatchEvent(new PointerEvent("pointerdown", common));
    el.dispatchEvent(new MouseEvent("mousedown", common));
    el.dispatchEvent(new PointerEvent("pointerup", common));
    el.dispatchEvent(new MouseEvent("mouseup", common));
    el.click();

    return postResult(true, `Clicked ${{el.tagName.toLowerCase()}} (${{TARGET_ID}})`);
  }} catch (err) {{
    return postResult(false, `Click error: ${{String(err)}}`);
  }}
}})();"#,
        request_id = request_id,
        id_json = id_json
    )
}

fn build_type_script(request_id: Uuid, node_id: &str, text: &str) -> String {
    let id_json = serde_json::to_string(node_id).unwrap_or_else(|_| "\"\"".to_string());
    let text_json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"(function() {{
  const REQUEST_ID = "{request_id}";
  const TARGET_ID = {id_json};
  const VALUE = {text_json};

  function postResult(success, detail) {{
    // The trusted host receives this value from WebView2 ExecuteScript.
    return {{ success, detail }};
  }}

  function findByAetherId(root, id) {{
    if (!root) return null;
    const direct = root.querySelector(`[data-aether-id="${{CSS.escape(id)}}"]`);
    if (direct) return direct;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    let node = walker.nextNode();
    while (node) {{
      if (node.shadowRoot) {{
        const found = findByAetherId(node.shadowRoot, id);
        if (found) return found;
      }}
      node = walker.nextNode();
    }}
    return null;
  }}

  try {{
    const el = findByAetherId(document, TARGET_ID);
    if (!el) {{
      return postResult(false, `Input ${{TARGET_ID}} not found in DOM`);
      return;
    }}
    el.scrollIntoView({{ block: "center", inline: "nearest", behavior: "instant" }});

    if (el.isContentEditable) {{
      el.textContent = VALUE;
      el.dispatchEvent(new InputEvent("input", {{ bubbles: true, data: VALUE, inputType: "insertText" }}));
      el.dispatchEvent(new Event("change", {{ bubbles: true }}));
      return postResult(true, `Typed into contenteditable (${{TARGET_ID}})`);
      return;
    }}

    const proto = el.tagName === "TEXTAREA"
      ? window.HTMLTextAreaElement.prototype
      : window.HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
    if (setter) {{
      setter.call(el, VALUE);
    }} else {{
      el.value = VALUE;
    }}
    el.dispatchEvent(new Event("input", {{ bubbles: true }}));
    el.dispatchEvent(new Event("change", {{ bubbles: true }}));
    return postResult(true, `Typed ${{VALUE.length}} chars into ${{el.tagName.toLowerCase()}} (${{TARGET_ID}})`);
  }} catch (err) {{
    return postResult(false, `Type error: ${{String(err)}}`);
  }}
}})();"#,
        request_id = request_id,
        id_json = id_json,
        text_json = text_json
    )
}

fn build_scroll_script(request_id: Uuid, delta_y: i32) -> String {
    format!(
        r#"(function() {{
  const REQUEST_ID = "{request_id}";
  function postResult(success, detail) {{
    // The trusted host receives this value from WebView2 ExecuteScript.
    return {{ success, detail }};
  }}
  try {{
    window.scrollBy({{ top: {delta_y}, left: 0, behavior: "instant" }});
    return postResult(true, `Scrolled by {delta_y}px (scrollY=${{Math.round(window.scrollY)}})`);
  }} catch (err) {{
    return postResult(false, `Scroll error: ${{String(err)}}`);
  }}
}})();"#,
        request_id = request_id,
        delta_y = delta_y
    )
}

async fn eval_in_active_tab(
    app: &tauri::AppHandle,
    webview_label: &str,
    _pending_results: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<ActionResult>>>>,
    script: String,
    _request_id: Uuid,
    timeout_ms: u64,
) -> ActionResult {
    // Untrusted websites are not allowed to invoke Tauri commands.
    // Capture this action's return value via the native WebView2 callback.
    match crate::agent::native_eval::eval_json(app, webview_label, script, timeout_ms).await {
        Ok(value) => serde_json::from_value::<ActionResult>(value).unwrap_or(ActionResult {
            success: false,
            detail: "WebView2 returned no valid action acknowledgement".into(),
        }),
        Err(problem) => ActionResult {
            success: false,
            detail: format!("WebView2 action result unavailable: {problem}"),
        },
    }
}

/// Executes an approved `BrowserAction` directly against the `active-tab` child WebView2
/// without stealing OS window focus.
pub async fn execute_browser_action(
    app: &tauri::AppHandle,
    webview_label: &str,
    active_url: &Arc<Mutex<String>>,
    pending_results: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<ActionResult>>>>,
    itp: &Arc<ItpEngine>,
    downloads: &Arc<DownloadManager>,
    db: &Arc<Mutex<BrowserDatabase>>,
    action: &BrowserAction,
) -> ActionResult {
    match action {
        BrowserAction::Navigate { url } => {
            let parsed = match url::Url::parse(url) {
                Ok(u) => u,
                Err(e) => {
                    return ActionResult {
                        success: false,
                        detail: format!("Invalid URL: {e}"),
                    };
                }
            };
            let target = parsed.to_string();
            if !itp.allow_navigation(&parsed) {
                return ActionResult {
                    success: false,
                    detail: format!("Navigation blocked by privacy or URL scheme policy: {target}"),
                };
            }

            if let Some(tab_wv) = app.get_webview(webview_label) {
                if let Err(e) = tab_wv.navigate(parsed) {
                    return ActionResult {
                        success: false,
                        detail: format!("Navigation failed: {e}"),
                    };
                }
                *active_url.lock().await = target.clone();
                let itp_verdict = itp.check_url(&target);
                {
                    let mut guard = db.lock().await;
                    let _ = guard.record_visit(&target, None, itp_verdict.blocked_trackers);
                }
                let _ = app.emit("agent:tab_navigated", &target);
                ActionResult {
                    success: true,
                    detail: format!("Navigated child WebView2 to {target}"),
                }
            } else {
                ActionResult {
                    success: false,
                    detail: "Child WebView2 tab not open yet".to_string(),
                }
            }
        }
        BrowserAction::Click { node_id, .. } => {
            let req_id = Uuid::new_v4();
            let script = build_click_script(req_id, node_id);
            eval_in_active_tab(app, webview_label, pending_results, script, req_id, 1800).await
        }
        BrowserAction::TypeText { node_id, text, .. } => {
            let req_id = Uuid::new_v4();
            let script = build_type_script(req_id, node_id, text);
            eval_in_active_tab(app, webview_label, pending_results, script, req_id, 1800).await
        }
        BrowserAction::Scroll { delta_y } => {
            let req_id = Uuid::new_v4();
            let script = build_scroll_script(req_id, *delta_y);
            eval_in_active_tab(app, webview_label, pending_results, script, req_id, 1500).await
        }
        BrowserAction::Download {
            url,
            suggested_filename,
        } => {
            match downloads
                .start_streaming_download(
                    app.clone(),
                    db.clone(),
                    itp.clone(),
                    url.clone(),
                    suggested_filename.clone(),
                )
                .await
            {
                Ok(record) => {
                    let _ = app.emit("agent:download_queued", &record);
                    ActionResult {
                        success: true,
                        detail: format!(
                            "Streaming download '{}' -> {}",
                            record.filename, record.saved_path
                        ),
                    }
                }
                Err(err) => ActionResult {
                    success: false,
                    detail: format!("Download failed to start: {err}"),
                },
            }
        }
        BrowserAction::Complete { summary } => {
            // Completion is announced only by loop_runner after the independent
            // evidence verifier has accepted the final source-backed report.
            ActionResult {
                success: true,
                detail: format!("COMPLETED: {summary}"),
            }
        }
    }
}

#[cfg(test)]
mod failure_recovery_execution_tests {
    use super::*;

    #[test]
    fn click_script_checks_disabled_controls_before_claiming_success() {
        let script = build_click_script(Uuid::new_v4(), "n_2");
        let guard = script.find("el.matches(\":disabled\")").expect("Disabled-control guard missing");
        let click = script.find("el.click();").expect("Click implementation missing");
        let success = script.find("return postResult(true,").expect("Successful click result missing");
        assert!(guard < click && click < success,
            "Disabled controls must be rejected before executing and claiming a click succeeded");
        assert!(script.contains("aria-disabled"));
    }
}
