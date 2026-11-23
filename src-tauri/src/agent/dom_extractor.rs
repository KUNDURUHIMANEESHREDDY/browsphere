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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
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
/// - Assigns stable, page-unique `data-aether-id` attributes to interactive controls
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
        .map((id) => {{
          const root = el.getRootNode();
          const target = root?.getElementById?.(id) || document.getElementById(id);
          return target?.innerText || target?.textContent || "";
        }})
        .join(" ")
        .trim();
      if (txt) return txt.slice(0, 120);
    }}

    if (el.id) {{
      const root = el.getRootNode();
      const selector = `label[for="${{CSS.escape(el.id)}}"]`;
      const lbl = root?.querySelector?.(selector) || document.querySelector(selector);
      if (lbl && lbl.innerText.trim()) return lbl.innerText.trim().slice(0, 120);
    }}

    const placeholder = el.getAttribute("placeholder");
    if (placeholder && placeholder.trim()) return placeholder.trim().slice(0, 120);

    const title = el.getAttribute("title");
    if (title && title.trim()) return title.trim().slice(0, 120);

    // Image-only result cards and controls in Shadow DOM still need meaningful
    // accessible names; unlabelled clickable cards were otherwise skipped.
    const ancestorLabel = el.closest("label");
    const imageAlt = el.querySelector("img[alt]")?.getAttribute("alt") || "";
    // Text-only layout whitespace must not eclipse a meaningful image alt.
    const normalized = value => String(value || "").replace(/\s+/g, " ").trim();
    const text = normalized(el.innerText) || normalized(el.value)
      || normalized(ancestorLabel?.innerText) || normalized(imageAlt)
      || normalized(el.getAttribute("alt"));
    return text.slice(0, 120);
  }}

  function isInteractive(el) {{
    // Disabled controls cannot be selected as actionable targets.
    if (el.matches(":disabled") || el.getAttribute("aria-disabled") === "true") return false;
    if (INTERACTIVE_TAGS.has(el.tagName)) return true;
    const role = (el.getAttribute("role") || "").toLowerCase();
    if (INTERACTIVE_ROLES.has(role)) return true;
    if (el.hasAttribute("onclick") || el.getAttribute("contenteditable") === "true") return true;
    const tabIdx = el.getAttribute("tabindex");
    if (tabIdx !== null && parseInt(tabIdx, 10) >= 0) return true;
    return false;
  }}

  const nodes = [];
  // Dynamic SPAs replace elements between observations. Every new node must
  // receive a unique ID; restarting an n_1 counter could silently target a
  // different, older control with the same data-aether-id.
  const observedIds = new Set();
  // The same control may appear in both the high-priority main/search subtree
  // and a later document-wide scan. Count each element only once while still
  // assigning a new ID to *different* elements with duplicate attributes.
  const visitedElements = new WeakSet();
  let counter = 0;
  function freshNodeId() {{
    let id;
    do {{
      const nonce = window.crypto?.randomUUID?.()
        || (Date.now().toString(36) + "_" + Math.random().toString(36).slice(2));
      id = "agent_" + nonce + "_" + (++counter);
    }} while (observedIds.has(id) || document.querySelector('[data-aether-id="' + id + '"]'));
    return id;
  }}
  const vw = window.innerWidth || 1280;
  const vh = window.innerHeight || 800;

  function walk(root) {{
    if (!root || nodes.length >= MAX_NODES) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    let current = walker.nextNode();
    while (current && nodes.length < MAX_NODES) {{
      if (isVisible(current) && isInteractive(current) &&
          !visitedElements.has(current)) {{
        visitedElements.add(current);
        let nodeId = current.getAttribute("data-aether-id");
        if (!nodeId || observedIds.has(nodeId)) {{
          nodeId = freshNodeId();
          try {{ current.setAttribute("data-aether-id", nodeId); }} catch (_) {{}}
        }}
        observedIds.add(nodeId);
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
          // Retain same-origin destinations but strip arbitrary URL query
          // parameters, which might include OAuth codes or other secrets.
          // YouTube watch links retain only the video ID for DOM-based clicks.
          href: current.tagName === "A"
            ? (() => {{
                try {{
                  const address = new URL(current.href);
                  if (!/^https?:$/.test(address.protocol)
                      || address.origin !== location.origin) return null;
                  if (address.hostname === "www.youtube.com"
                      && address.pathname === "/watch") {{
                    const videoId = address.searchParams.get("v");
                    if (!videoId) return null;
                    return address.origin + "/watch?v="
                      + encodeURIComponent(videoId.slice(0, 60));
                  }}
                  return address.origin + address.pathname.slice(0, 480);
                }} catch (_) {{ return null; }}
              }})() : null,
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

  // On YouTube search results, inspect result cards before the numerous
  // navigation/toolbar controls exhaust the bounded interactive node budget.
  // This is still a fresh DOM observation, never a guessed result URL.
  if (/^([^.]+\.)?youtube\.com$/i.test(location.hostname) &&
      location.pathname === "/results") {{
    const resultsRoot = document.querySelector(
      'ytd-two-column-search-results-renderer #contents');
    if (resultsRoot) walk(resultsRoot);
  }}
  // Prioritize *general page semantics*, not site-specific selectors. Many
  // websites place hundreds of links in their header/sidebar before the
  // relevant article, results, search form, or dialog. Scan those regions
  // first within the bounded observation budget, then the rest of the page.
  for (const selector of [
    '[role="dialog"][aria-modal="true"]', '[role="search"]', 'form[role="search"]',
    'main', '[role="main"]', '#main', '#content', '.search-results',
    '[role="dialog"]'
  ]) {{
    const region = document.querySelector(selector);
    if (region && isVisible(region)) walk(region);
  }}
  walk(document.body || document.documentElement);

  // Give the actual content/dialog priority over enormous navigation menus,
  // while retaining a bounded slice of the remaining page for context.
  const primaryTextRoot = document.querySelector(
    '[role="dialog"][aria-modal="true"], main, [role="main"], article, #main, #content');
  const contentText = isVisible(primaryTextRoot)
    ? (primaryTextRoot.innerText || "") : "";
  const bodyText = document.body?.innerText || "";
  const visibleText = [contentText, bodyText]
    .filter(Boolean).join(" ")
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
