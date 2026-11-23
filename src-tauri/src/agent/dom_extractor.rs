use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub in_viewport: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomNodeSummary {
    pub node_id: String,
    pub tag: String,
    pub role: String,
    pub label: String,
    pub is_submit: bool,
    pub is_sensitive: bool,
    pub bbox: Option<BoundingBox>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomSnapshot {
    pub origin: String,
    pub title: String,
    pub text_excerpt: String,
    pub interactive_nodes: Vec<DomNodeSummary>,
}

/// Builds an IIFE injected into the active child WebView2 view.
/// - Traverses document + open Shadow DOM roots
/// - Skips hidden, display:none, visibility:hidden, zero-size, or aria-hidden nodes (prompt-injection hardening)
/// - Assigns stable `data-aether-id` attributes (`n_1`, `n_2`, ...)
/// - Computes viewport bounding boxes & flags submit controls and sensitive inputs
/// - Posts the structured snapshot back to Rust via `submit_dom_snapshot`
pub fn build_snapshot_script(request_id: Uuid, max_nodes: usize, max_text_chars: usize) -> String {
    format!(
        r#"(function() {{
  const REQUEST_ID = "{request_id}";
  const MAX_NODES = {max_nodes};
  const MAX_TEXT = {max_text_chars};

  const SENSITIVE_RE = /(password|passwd|passcode|otp|one[-_]?time|cvv|cvc|card[-_]?num|cc[-_]|ssn|secret|api[-_]?key|token)/i;
  const INTERACTIVE_TAGS = new Set(["A", "BUTTON", "INPUT", "SELECT", "TEXTAREA", "SUMMARY", "OPTION"]);
  const INTERACTIVE_ROLES = new Set([
    "button", "link", "textbox", "searchbox", "combobox", "listbox",
    "menuitem", "checkbox", "radio", "switch", "tab", "slider"
  ]);

  function isVisible(el) {{
    if (!el || el.nodeType !== 1) return false;
    if (el.getAttribute("aria-hidden") === "true" || el.hidden) return false;
    const style = window.getComputedStyle(el);
    if (!style) return false;
    if (style.display === "none" || style.visibility === "hidden" || parseFloat(style.opacity || "1") === 0) {{
      return false;
    }}
    const rect = el.getBoundingClientRect();
    if (rect.width < 2 || rect.height < 2) return false;
    return true;
  }}

  function isSensitiveField(el) {{
    const tag = el.tagName;
    if (tag !== "INPUT" && tag !== "TEXTAREA") return false;
    const type = (el.getAttribute("type") || "").toLowerCase();
    if (type === "password") return true;
    const ac = (el.getAttribute("autocomplete") || "").toLowerCase();
    if (ac.includes("password") || ac.includes("cc-") || ac.includes("one-time-code")) return true;
    const hint = [
      el.getAttribute("name") || "",
      el.getAttribute("id") || "",
      el.getAttribute("aria-label") || "",
      el.getAttribute("placeholder") || ""
    ].join(" ");
    return SENSITIVE_RE.test(hint);
  }}

  function computeAccessibleName(el) {{
    const ariaLabel = el.getAttribute("aria-label");
    if (ariaLabel && ariaLabel.trim()) return ariaLabel.trim().slice(0, 120);

    const labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {{
      const txt = labelledBy
        .split(/\s+/)
        .map((id) => document.getElementById(id)?.innerText || "")
        .join(" ")
        .trim();
      if (txt) return txt.slice(0, 120);
    }}

    if (el.id) {{
      const lbl = document.querySelector(`label[for="${{CSS.escape(el.id)}}"]`);
      if (lbl && lbl.innerText.trim()) return lbl.innerText.trim().slice(0, 120);
    }}

    const placeholder = el.getAttribute("placeholder");
    if (placeholder && placeholder.trim()) return placeholder.trim().slice(0, 120);

    const title = el.getAttribute("title");
    if (title && title.trim()) return title.trim().slice(0, 120);

    const text = (el.innerText || el.value || "").replace(/\s+/g, " ").trim();
    return text.slice(0, 120);
  }}

  function isInteractive(el) {{
    if (INTERACTIVE_TAGS.has(el.tagName)) return true;
    const role = (el.getAttribute("role") || "").toLowerCase();
    if (INTERACTIVE_ROLES.has(role)) return true;
    if (el.hasAttribute("onclick") || el.getAttribute("contenteditable") === "true") return true;
    const tabIdx = el.getAttribute("tabindex");
    if (tabIdx !== null && parseInt(tabIdx, 10) >= 0) return true;
    return false;
  }}

  const nodes = [];
  let counter = 1;
  const vw = window.innerWidth || 1280;
  const vh = window.innerHeight || 800;

  function walk(root) {{
    if (!root || nodes.length >= MAX_NODES) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    let current = walker.nextNode();
    while (current && nodes.length < MAX_NODES) {{
      if (isVisible(current) && isInteractive(current)) {{
        let nodeId = current.getAttribute("data-aether-id");
        if (!nodeId) {{
          nodeId = "n_" + counter++;
          try {{ current.setAttribute("data-aether-id", nodeId); }} catch (_) {{}}
        }}
        const rect = current.getBoundingClientRect();
        const inViewport = rect.bottom >= 0 && rect.right >= 0 && rect.top <= vh && rect.left <= vw;
        const typeAttr = (current.getAttribute("type") || "").toLowerCase();
        // A button without a type is only submit-capable when associated
        // with a form. Standalone buttons (tabs, menus, theme toggles) are
        // not implicit form submissions.
        const formSubmit =
          (current.tagName === "BUTTON" &&
            (typeAttr === "submit" || (typeAttr === "" && !!current.form))) ||
          (current.tagName === "INPUT" && (typeAttr === "submit" || typeAttr === "image"));
        // Custom JS submission controls may use type="button"; classify
        // them as submissions so the policy can assess their risk.
        const submissionLabel = /\b(submit|register|sign up|apply now|send application|place order|checkout|confirm purchase)\b/i;
        const isSubmit = formSubmit || (
          (current.tagName === "BUTTON" || current.getAttribute("role") === "button") &&
          submissionLabel.test(computeAccessibleName(current))
        );
        // A harmless-looking "Submit" can still commit sensitive form data.
        // Mark such submit controls sensitive using the observed containing form.
        const form = isSubmit ? (current.form || current.closest("form")) : null;
        const sensitiveForm = !!form && (
          Array.from(form.querySelectorAll("input, textarea")).some(isSensitiveField) ||
          /\b(payment|credit card|card number|cvv|billing|delete account|register|sign up|application|loan|identity|passport|contract|bank transfer|purchase|checkout|reservation)\b/i
            .test(form.innerText || "") ||
          (form.action && (() => {{
            try {{ return new URL(form.action, location.href).origin !== location.origin; }}
            catch (_) {{ return true; }}
          }})())
        );

        nodes.push({{
          node_id: nodeId,
          tag: current.tagName.toLowerCase(),
          role: (current.getAttribute("role") || current.tagName).toLowerCase(),
          label: computeAccessibleName(current),
          is_submit: isSubmit,
          is_sensitive: isSensitiveField(current) || sensitiveForm,
          bbox: {{
            x: Math.round(rect.x),
            y: Math.round(rect.y),
            width: Math.round(rect.width),
            height: Math.round(rect.height),
            in_viewport: inViewport
          }}
        }});
      }}
      if (current.shadowRoot) {{
        walk(current.shadowRoot);
      }}
      current = walker.nextNode();
    }}
  }}

  walk(document.body || document.documentElement);

  const visibleText = (document.body?.innerText || "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_TEXT);

  const payload = {{
    request_id: REQUEST_ID,
    snapshot: {{
      origin: window.location.href,
      title: document.title || window.location.hostname || "Untitled",
      text_excerpt: visibleText,
      interactive_nodes: nodes
    }}
  }};

  // Return to the HOST WebView2 ExecuteScript callback. Untrusted pages
  // are never granted Tauri commands or asked to send their own snapshots.
  return payload.snapshot;
}})();"#,
        request_id = request_id,
        max_nodes = max_nodes,
        max_text_chars = max_text_chars
    )
}

/// Evaluates the DOM serializer in the `active-tab` child WebView2 without stealing focus
/// and awaits the returned `DomSnapshot` with a bounded timeout.
pub async fn request_live_dom_snapshot(
    app: &tauri::AppHandle,
    webview_label: &str,
    _pending_snapshots: &Arc<Mutex<HashMap<Uuid, oneshot::Sender<DomSnapshot>>>>,
    timeout_ms: u64,
) -> Option<DomSnapshot> {
    // IPC from remote page origins is intentionally not permitted by Tauri.
    // Use the native WebView2 script-completion callback to read this result.
    let script = build_snapshot_script(Uuid::new_v4(), 120, 2400);
    let value = crate::agent::native_eval::eval_json(
        app, webview_label, script, timeout_ms
    ).await.ok()?;
    serde_json::from_value(value).ok()
}
