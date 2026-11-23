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
    if (!el.isConnected || el.matches(":disabled")
        || el.getAttribute("aria-disabled") === "true") {{
      return postResult(false, `Control ${{TARGET_ID}} is detached or disabled`);
    }}
    // Never dispatch an empty site-search form. Small local models may click
    // Search before filling its observed query input; avoid a blank navigation
    // while preserving ordinary search-openers (buttons without a form).
    const form = el.form || el.closest('form');
    const type = (el.getAttribute('type') || '').toLowerCase();
    const submit = (el.tagName === 'BUTTON' && (type === 'submit' || (type === '' && !!form)))
      || (el.tagName === 'INPUT' && (type === 'submit' || type === 'image'));
    if (submit && form) {{
      const searchFields = Array.from(form.querySelectorAll(
        'input[type="search"], input[role="searchbox"], input[name="search"], input[name="search_query"], input[aria-label*="Search"], input[placeholder*="Search"]'
      )).filter(field => !field.disabled && field.type !== 'hidden');
      if (searchFields.length && searchFields.every(field => !String(field.value || '').trim())) {{
        return postResult(false, 'Search form has no query; enter the requested search text first');
      }}
    }}
    const style = getComputedStyle(el);
    const bounds = el.getBoundingClientRect();
    if (style.display === "none" || style.visibility === "hidden"
        || bounds.width < 2 || bounds.height < 2) {{
      return postResult(false, `Control ${{TARGET_ID}} is not currently visible`);
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
    if (!el.isConnected || el.matches(":disabled")
        || el.getAttribute("aria-disabled") === "true"
        || el.hasAttribute("readonly") || el.readOnly) {{
      return postResult(false, `Input ${{TARGET_ID}} is detached, disabled or read-only`);
    }}
    if (!el.isContentEditable && el.tagName !== "INPUT" && el.tagName !== "TEXTAREA") {{
      return postResult(false, `Control ${{TARGET_ID}} is not an editable text field`);
    }}
    el.scrollIntoView({{ block: "center", inline: "nearest", behavior: "instant" }});
    el.focus();

    if (el.isContentEditable) {{
      el.textContent = VALUE;
      el.dispatchEvent(new InputEvent("input", {{ bubbles: true, data: VALUE, inputType: "insertText" }}));
      el.dispatchEvent(new Event("change", {{ bubbles: true }}));
      if (el.textContent !== VALUE) {{
        return postResult(false, `Editable control ${{TARGET_ID}} rejected the requested value`);
      }}
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
    if (el.value !== VALUE) {{
      return postResult(false, `Text field ${{TARGET_ID}} rejected the requested value`);
    }}
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

fn build_key_script(node_id: &str, key: &str) -> String {
    let id_json = serde_json::to_string(node_id).unwrap_or_else(|_| "\"\"".into());
    let key_json = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into());
    format!(r#"(function() {{
      const target = {id_json};
      const key = {key_json};
      function find(root) {{
        if (!root) return null;
        const here = root.querySelector('[data-aether-id="' + CSS.escape(target) + '"]');
        if (here) return here;
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
        let el;
        while ((el = walker.nextNode())) {{
          if (el.shadowRoot) {{
            const result = find(el.shadowRoot);
            if (result) return result;
          }}
        }}
        return null;
      }}
      const fail = detail => ({{ success: false, detail }});
      try {{
        if (key !== "Enter" && key !== "Escape") return fail("Unsupported key");
        const el = find(document);
        if (!el || !el.isConnected || el.matches(":disabled") ||
            el.getAttribute("aria-disabled") === "true") return fail("Key target unavailable");
        const box = el.getBoundingClientRect();
        const style = getComputedStyle(el);
        if (box.width < 2 || box.height < 2 || style.display === "none" ||
            style.visibility === "hidden") return fail("Key target is hidden");
        if (key === "Enter") {{
          const tag = el.tagName.toLowerCase();
          const role = (el.getAttribute("role") || "").toLowerCase();
          const hint = [el.getAttribute("aria-label"), el.getAttribute("placeholder"),
            el.getAttribute("name")].join(" ").toLowerCase();
          const kind = (el.getAttribute("type") || "").toLowerCase();
          if (tag !== "input" && tag !== "textarea" && !el.isContentEditable)
            return fail("Enter requires a text input");
          if (kind === "password" || kind === "hidden" || el.readOnly)
            return fail("Restricted input");
          if (role !== "searchbox" && role !== "combobox" && kind !== "search" &&
              !hint.includes("search")) return fail("Not a search field");
          // A synthetic Enter must never submit a blank search query.
          if (!String(el.value || el.textContent || "").trim())
            return fail("Search field is empty; enter the requested query first");
          const form = el.closest("form");
          if (form) {{
            if (form.querySelector('input[type="password"],input[autocomplete^="cc-"],input[name*="secret"]'))
              return fail("Sensitive search form");
            const otherInputs = Array.from(form.querySelectorAll("input, textarea, select"))
              .filter(field => field !== el && field.type !== "hidden" && field.type !== "submit");
            if (otherInputs.length) return fail("Search form has additional fields");
            const action = new URL(form.getAttribute("action") || location.href, location.href);
            if (action.origin !== location.origin) return fail("Cross-origin search form blocked");
            if (/\b(payment|checkout|purchase|delete account|transfer|contract)\b/i.test(form.innerText || ""))
              return fail("Consequential search form blocked");
          }}
          el.focus();
          const proceed = el.dispatchEvent(new KeyboardEvent("keydown",
            {{ key: "Enter", code: "Enter", bubbles: true, cancelable: true }}));
          el.dispatchEvent(new KeyboardEvent("keyup",
            {{ key: "Enter", code: "Enter", bubbles: true }}));
          if (proceed && form && typeof form.requestSubmit === "function")
            form.requestSubmit();
          return {{ success: true, detail: "Enter dispatched to observed search control" }};
        }}
        const dialog = el.tagName === "DIALOG" && el.open ? el
          : el.closest("dialog[open]");
        if (dialog && dialog.open) {{
          // A synthetic Escape does not trigger WebView2's native dialog
          // default action. Close only this freshly observed DOM dialog.
          if (!dialog.dispatchEvent(new Event("cancel", {{ cancelable: true }})))
            return fail("Dialog prevented Escape cancellation");
          dialog.close();
          if (dialog.open) return fail("Dialog is still open after Escape");
          return {{ success: true, detail: "Observed native dialog closed with Escape" }};
        }}
        el.focus();
        el.dispatchEvent(new KeyboardEvent("keydown",
          {{ key: "Escape", code: "Escape", bubbles: true, cancelable: true }}));
        el.dispatchEvent(new KeyboardEvent("keyup",
          {{ key: "Escape", code: "Escape", bubbles: true }}));
        return {{ success: true, detail: "Escape dispatched to observed control" }};
      }} catch (error) {{
        return fail("Key action failed: " + String(error).slice(0, 120));
      }}
    }})();"#, id_json=id_json, key_json=key_json)
}

fn build_select_script(node_id: &str, option_label: &str) -> String {
    let id_json = serde_json::to_string(node_id).unwrap_or_else(|_| "\"\"".into());
    let label_json = serde_json::to_string(option_label).unwrap_or_else(|_| "\"\"".into());
    format!(r#"(function() {{
      const targetId = {id_json}, requestedLabel = {label_json}.trim().toLowerCase();
      const fail = detail => ({{success:false,detail}});
      function find(root) {{
        if (!root) return null;
        const direct = root.querySelector('[data-aether-id="' + CSS.escape(targetId) + '"]');
        if (direct) return direct;
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
        let el;
        while ((el = walker.nextNode())) {{
          if (el.shadowRoot) {{
            const nested = find(el.shadowRoot);
            if (nested) return nested;
          }}
        }}
        return null;
      }}
      try {{
        const el = find(document);
        if (!el || el.tagName !== "SELECT" || !el.isConnected ||
            el.disabled || el.matches(":disabled") || el.getAttribute("aria-disabled") === "true")
          return fail("Select target unavailable or disabled");
        const rect = el.getBoundingClientRect(), style = getComputedStyle(el);
        if (rect.width < 2 || rect.height < 2 || style.display === "none" ||
            style.visibility === "hidden") return fail("Select is hidden");
        const options = Array.from(el.options).filter(o =>
          !o.disabled && !o.parentElement?.disabled &&
          (o.textContent || "").replace(/\s+/g, " ").trim().toLowerCase() === requestedLabel);
        if (options.length !== 1) return fail("Option missing or ambiguous");
        const option = options[0];
        if (el.multiple) return fail("Multi-select must be handled separately");
        const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")?.set;
        if (setter) setter.call(el, option.value);
        else el.value = option.value;
        el.dispatchEvent(new Event("input", {{bubbles:true}}));
        el.dispatchEvent(new Event("change", {{bubbles:true}}));
        const chosen = el.selectedOptions[0];
        if (chosen !== option) return fail("Select did not accept requested option");
        return {{success:true,detail:"Selected an observed dropdown option"}};
      }} catch (err) {{
        return fail("Select action failed: " + String(err).slice(0, 120));
      }}
    }})();"#, id_json=id_json, label_json=label_json)
}

fn build_checked_script(node_id: &str, expected: bool) -> String {
    let id_json = serde_json::to_string(node_id).unwrap_or_else(|_| "\"\"".into());
    format!(r#"(function() {{
      const targetId = {id_json}, expected = {expected};
      const fail = detail => ({{success:false,detail}});
      function find(root) {{
        if (!root) return null;
        const direct = root.querySelector('[data-aether-id="' + CSS.escape(targetId) + '"]');
        if (direct) return direct;
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
        let el;
        while ((el = walker.nextNode())) {{
          if (el.shadowRoot) {{
            const child = find(el.shadowRoot);
            if (child) return child;
          }}
        }}
        return null;
      }}
      try {{
        const el = find(document);
        if (!el || !el.isConnected || el.matches(":disabled") ||
            el.getAttribute("aria-disabled") === "true") return fail("Toggle unavailable");
        const style = getComputedStyle(el), rect = el.getBoundingClientRect();
        if (rect.width < 2 || rect.height < 2 || style.display === "none" ||
            style.visibility === "hidden") return fail("Toggle is hidden");
        const native = el.tagName === "INPUT" && ["checkbox","radio"].includes(el.type);
        const role = native ? el.type : (el.getAttribute("role") || "").toLowerCase();
        if (!["checkbox","radio","switch"].includes(role)) return fail("Not a toggle");
        if (role === "radio" && !expected) return fail("Radio options cannot be unchecked");
        const current = native ? el.checked
          : el.getAttribute("aria-checked") === "true" ? true
          : el.getAttribute("aria-checked") === "false" ? false : null;
        if (current === null) return fail("Toggle has no observable checked state");
        if (current === expected) return {{success:true,detail:"Toggle already in requested state"}};
        el.scrollIntoView({{block:"center",inline:"nearest",behavior:"instant"}});
        el.click();
        const after = native ? el.checked : el.getAttribute("aria-checked") === "true";
        if (after !== expected) return fail("Toggle state did not change as requested");
        return {{success:true,detail:"Observed toggle state changed"}};
      }} catch (error) {{
        return fail("Toggle action failed: " + String(error).slice(0,120));
      }}
    }})();"#, id_json=id_json, expected=expected)
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
        BrowserAction::SetChecked { node_id, checked, .. } => {
            let req_id = Uuid::new_v4();
            let script = build_checked_script(node_id, *checked);
            eval_in_active_tab(app, webview_label, pending_results, script, req_id, 1800).await
        }
        BrowserAction::SelectOption { node_id, option_label } => {
            let req_id = Uuid::new_v4();
            let script = build_select_script(node_id, option_label);
            eval_in_active_tab(app, webview_label, pending_results, script, req_id, 1800).await
        }
        BrowserAction::PressKey { node_id, key } => {
            let req_id = Uuid::new_v4();
            let script = build_key_script(node_id, key);
            let result = eval_in_active_tab(
                app, webview_label, pending_results, script, req_id, 2200
            ).await;
            if result.success && key == "Escape" {
                // Native HTMLDialogElement.close() updates its open flag
                // immediately, but close event handlers run in a later task.
                // Allow page handlers to settle before the next observation.
                tokio::time::sleep(std::time::Duration::from_millis(180)).await;
            }
            result
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
    fn click_script_blocks_blank_search_forms_before_dispatch() {
        let script = build_click_script(Uuid::new_v4(), "search_submit");
        assert!(script.contains("if (submit && form)"));
        assert!(script.contains("input[name=\"search\"]"));
        assert!(script.contains("searchFields.every(field => !String(field.value || '').trim())"));
        assert!(script.contains("Search form has no query; enter the requested search text first"));
        assert!(script.find("Search form has no query").unwrap()
            < script.find("el.click()").unwrap());
    }

    #[test]
    fn enter_script_rejects_blank_search_before_request_submit() {
        let script = build_key_script("search_input", "Enter");
        let blank = script.find("Search field is empty; enter the requested query first").unwrap();
        let submission = script.find("form.requestSubmit()").unwrap();
        assert!(blank < submission);
        assert!(script.contains("!String(el.value || el.textContent || \"\").trim()"));
    }

    #[test]
    fn type_script_refuses_noneditable_or_readonly_controls() {
        let script = build_type_script(Uuid::new_v4(), "search", "NASA Artemis");
        assert!(script.contains("el.hasAttribute(\"readonly\")"));
        assert!(script.contains("el.isContentEditable"));
        assert!(script.contains("el.tagName !== \"INPUT\""));
        assert!(script.contains("el.value !== VALUE"));
        assert!(script.find("el.value !== VALUE").unwrap()
            < script.rfind("return postResult(true, `Typed").unwrap());
        assert!(script.find("el.textContent !== VALUE").unwrap()
            < script.find("return postResult(true, `Typed into contenteditable").unwrap());
    }

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
