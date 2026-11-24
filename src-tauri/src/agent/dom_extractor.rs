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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// Only a search-state classification; never the actual input value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    pub is_submit: bool,
    pub is_sensitive: bool,
    pub bbox: Option<BoundingBox>,
}

/// Bounded structural counts from the native visible DOM. No input values,
/// HTML, hidden metadata, or page-supplied classification instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductObservation {pub heading:String,pub price:String,pub context:String}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PageStructure {
    pub product: Option<ProductObservation>,
    pub product_heading: Option<String>,
    pub product_price_status: Option<String>,
    pub headings: u16,
    pub paragraphs: u16,
    pub code_blocks: u16,
    pub articles: u16,
    pub result_regions: u16,
    pub result_links: u16,
    pub product_cards: u16,
    pub priced_cards: u16,
    pub offer_blocks: u16,
    pub loading: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomSnapshot {
    #[serde(default)]
    pub structure: PageStructure,
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
  const POINT_PROBE = null;
  const EXPLICIT_LINK_SOURCE = null;
  const FOCUS_TERMS = [];

  const SENSITIVE_RE = /(password|passwd|passcode|otp|one[-_]?time|cvv|cvc|card[-_]?num|cc[-_]|ssn|secret|api[-_]?key|token)/i;
  const INTERACTIVE_TAGS = new Set(["A", "BUTTON", "INPUT", "SELECT", "TEXTAREA", "SUMMARY", "OPTION", "DIALOG"]);
  const INTERACTIVE_ROLES = new Set([
    "button", "link", "textbox", "searchbox", "combobox", "listbox",
    "menuitem", "checkbox", "radio", "switch", "tab", "slider", "dialog", "alertdialog", "option"
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
    if (tag !== "INPUT" && tag !== "TEXTAREA" && tag !== "SELECT") return false;
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

  function calendarDateLabel(el) {{
    if (!el.matches('a,button,[role="gridcell"]')) return null;
    const cal = el.closest('.ui-datepicker,.p-datepicker,[role="grid"]');
    if (!cal || !isVisible(cal) || el.closest('.ui-datepicker-other-month,.p-datepicker-other-month,[aria-disabled="true"],.ui-state-disabled,.p-disabled')) return null;
    const day = (el.innerText || "").trim();
    if (!/^(?:[1-9]|[12]\d|3[01])$/.test(day)) return null;
    const monthEl=cal.querySelector('.ui-datepicker-month,.p-datepicker-month');
    const yearEl=cal.querySelector('.ui-datepicker-year,.p-datepicker-year');
    if (!monthEl||!yearEl||!isVisible(monthEl)||!isVisible(yearEl)) return null;
    const months=["january","february","march","april","may","june","july","august","september","october","november","december"];
    const name=(monthEl.innerText||"").trim().toLowerCase();
    const year=(yearEl.innerText||"").trim();
    const month=months.findIndex(m=>m===name||m.slice(0,3)===name)+1;
    if (!month||!/^20\d\d$/.test(year)) return null;
    const date=new Date(Date.UTC(Number(year),month-1,Number(day)));
    if(date.getUTCMonth()!==month-1)return null;
    return date.toISOString().slice(0,10);
  }}
  function computeAccessibleName(el) {{
    const calendarLabel=calendarDateLabel(el);
    if(calendarLabel)return calendarLabel;
    const ariaLabel = el.getAttribute("aria-label");
    // Framework serialization bugs can expose HTML or a sanitizer diagnostic
    // as every button's aria-label. Use the rendered label instead; never use
    // embedded markup as a control name.
    const usableLabel = value => value && value.trim() &&
      !/SafeValue must use|<\/?(?:body|html|div|h[1-6]|span|p)(?:\s|>)/i.test(value);
    if (usableLabel(ariaLabel)) return ariaLabel.trim().slice(0, 120);

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

    const wrappingLabel = el.closest("label");
    if (wrappingLabel && wrappingLabel.innerText.trim()) {{
      return wrappingLabel.innerText.replace(/\s+/g, " ").trim().slice(0, 120);
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
    // Custom form frameworks often render a short visual label beside the
    // input rather than a native label. Only use bounded nearby label text.
    if (el.matches('input,textarea,select,[role="combobox"],[role="searchbox"]')) {{
      const candidates=[el.previousElementSibling,el.parentElement?.previousElementSibling];
      for(const sibling of candidates) {{
        if(!sibling || sibling.querySelector('input,textarea,select,button,a')) continue;
        const label=(sibling.innerText||'').trim().replace(/\s+/g,' ');
        if(label.length>0&&label.length<=48&&/^(from|to|origin|destination|departure|arrival|location|station|city|date|check.in|check.out|start|end|name|search|email|address|phone)(\b|$)/i.test(label)) return label;
      }}
      const name=(el.getAttribute('name')||'').trim();
      if(name&&name.length<=48) return name;
    }}

    // Image-only result cards and controls in Shadow DOM still need meaningful
    // accessible names; unlabelled clickable cards were otherwise skipped.
    const ancestorLabel = el.closest("label");
    const imageAlt = el.querySelector("img[alt]")?.getAttribute("alt") || "";
    // Text-only layout whitespace must not eclipse a meaningful image alt.
    const normalized = value => String(value || "").replace(/\s+/g, " ").trim();
    // Editor content is not an accessible control label.
    if ((el.tagName === "TEXTAREA" || el.isContentEditable) &&
        !el.closest("form") && el.getBoundingClientRect().height >= 72)
      return "Document editor";
    const text = normalized(el.innerText) || (el.matches("input,textarea,select") ? "" : normalized(el.value))
      || normalized(ancestorLabel?.innerText) || normalized(imageAlt)
      || normalized(el.getAttribute("alt"));
    return text.slice(0, 120);
  }}

  // A card can have separate image and text links to the same product.
  // Use only a single rendered product heading from a bounded card that
  // contains no competing product destinations. Do not read hidden metadata.
  // Resolve only explicit, public retailer product URLs on known search
  // result pages. Opaque redirect tokens and all tracking parameters stay
  // private; never guess a destination or treat a snippet as evidence.
  function publicSearchProduct(el) {{
    if(!["https://www.google.com","https://search.brave.com"].includes(location.origin)||location.pathname!=="/search"||!isVisible(el)||el.closest('[aria-hidden="true"],[hidden]'))return null;
    try{{
      let u=new URL(el.href);
      if(u.origin==="https://www.google.com"&&["/url","/goto"].includes(u.pathname)){{
        const keys=u.pathname==="/goto"?["url"]:["q","url"];
        const values=keys.flatMap(k=>u.searchParams.getAll(k));
        if(values.length!==1||!values[0].startsWith("https://"))return null;
        u=new URL(values[0]);
      }}else if(u.origin==="https://search.brave.com"&&u.pathname==="/go/v1"){{
        const values=u.searchParams.getAll("url");
        if(values.length!==1||!values[0].startsWith("https://"))return null;
        u=new URL(values[0]);
      }}
      if(!["www.croma.com","www.reliancedigital.in"].includes(u.hostname)||u.protocol!=="https:"||u.port||u.username||u.password||u.search||u.hash||u.pathname.length>480||!/(?:\/product\/[^/]+\/?$|\/p\/[^/]+\/?$)/i.test(u.pathname))return null;
      return u.origin+u.pathname;
    }}catch(_e){{return null;}}
  }}
  // Preserve the canonical public product destination when the only
  // query keys are known marketing attribution keys. Variant or session
  // parameters are not silently stripped to manufacture a product link.
  function sameOriginProduct(el) {{
    if(el.tagName!=="A"||!isVisible(el)||el.closest('[aria-hidden="true"],[hidden]'))return null;
    try{{
      const u=new URL(el.href);
      if(u.origin!==location.origin||u.username||u.password||u.hash||
         u.pathname.length>480||!/(?:\/product\/[^/]+\/?$|\/p\/[^/]+\/?$)/i.test(u.pathname))return null;
      if([...u.searchParams.keys()].some(k=>!/^utm_(source|medium|campaign|term|content|id)$/i.test(k)))return null;
      return u.origin+u.pathname;
    }}catch(_e){{return null;}}
  }}
  function productLinkLabel(el) {{
    if(el.tagName!=="A"||el.closest('[aria-hidden="true"],[hidden],nav,header,footer,form'))return null;
    const productPath=a=>sameOriginProduct(a);
    const target=productPath(el);if(!target)return null;
    const cardSelector='article,li,[itemtype*="schema.org/Product"],[data-testid*="product-card"],[class*="product-card"],[class*="productCard"],[class*="product-tile"],[class*="product-item"]';
    const titleSelector='h2,h3,h4,[role="heading"],[class*="product-title"],[class*="productTitle"],[class*="product-name"],[data-testid*="product-title"],[itemprop="name"]';
    let card=el.closest(cardSelector)||el.parentElement;
    for(let depth=0;card&&depth<4;depth++,card=card.parentElement){{
      if(card.matches('main,body,html,nav,header,footer,form')||!isVisible(card)||card.closest('[aria-hidden="true"],[hidden]'))return null;
      const links=Array.from(card.querySelectorAll('a[href]')).filter(a=>isVisible(a)&&!a.closest('[aria-hidden="true"],[hidden]'));
      if(links.length>24)return null;
      const destinations=new Set(links.map(productPath).filter(Boolean));
      // Refuse an ancestor containing an ambiguous or uncanonicalizable
      // product URL, even if only one other URL is canonical.
      if(links.some(a=>/(?:\/product\/|\/p\/)/i.test(a.pathname||"")&&!productPath(a))||
         destinations.size!==1||!destinations.has(target))return null;
      const headings=Array.from(card.querySelectorAll(titleSelector)).filter(h=>isVisible(h)&&!h.closest('[aria-hidden="true"],[hidden]'));
      const labels=[...new Set(headings.map(h=>(h.innerText||"").replace(/\s+/g," ").trim()).filter(t=>t.length>=12&&t.length<=240))];
      if(labels.length>1)return null;
      if(labels.length===1)return labels[0].slice(0,200);
    }}
    return null;
  }}
  function paginationLabel(el) {{
    const label=computeAccessibleName(el).trim();
    if(!/^(next|next page|go to next page|next results page)$/i.test(label)||!el.matches('button,[role="button"]')||!el.closest('nav[aria-label*="pag" i],[class*="pagination" i],[data-testid*="pagination" i]'))return null;
    return "Next page";
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

  // A visible blocking dialog owns the interaction surface. Do not present
  // controls underneath it as actionable (including to the policy binder).
  const dialogs = Array.from(document.querySelectorAll(
    'dialog[open], [aria-modal="true"], [role="alertdialog"], [role="dialog"]'))
    .filter(el => {{
      if (!isVisible(el)) return false;
      const box = el.getBoundingClientRect();
      const x = Math.min(window.innerWidth - 1, Math.max(0, box.left + box.width / 2));
      const y = Math.min(window.innerHeight - 1, Math.max(0, box.top + box.height / 2));
      const top = document.elementFromPoint(x,y);
      return top && el.contains(top) && (el.matches('dialog[open], [aria-modal="true"], [role="alertdialog"]')
        || getComputedStyle(el).position === 'fixed'
        || Array.from(el.parentElement?.classList || []).some(c => /(?:^|[-_])(mask|overlay|backdrop)(?:$|[-_])/i.test(c)));
    }});
  if(!dialogs.length&&["www.croma.com","www.reliancedigital.in"].includes(location.hostname)){{
    const headings=Array.from(document.querySelectorAll('h1,h2,h3,h4,[role="heading"]')).filter(h=>isVisible(h)&&/^(select your location|choose your location|enter your pincode)$/i.test((h.innerText||"").trim()));
    for(const h of headings.slice(0,4)){{
      let el=h.parentElement;
      for(let depth=0;el&&depth<5;depth++,el=el.parentElement){{
        if(!isVisible(el)||!el.querySelector('input[placeholder*="pincode" i],input[aria-label*="pincode" i]'))continue;
        const r=el.getBoundingClientRect(),css=getComputedStyle(el);
        if(css.position!=="fixed"||r.width>=innerWidth*.95||r.height>=innerHeight*.95||r.width<100||r.height<80)continue;
        const hit=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2);
        if(hit&&el.contains(hit)){{dialogs.push(el);break;}}
      }}
    }}
  }}
  const activeDialog = dialogs.at(-1) || null;
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

  function walk(root, includeRoot = false) {{
    if (!root || nodes.length >= MAX_NODES) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    let current = includeRoot && root.nodeType === 1 ? root : walker.nextNode();
    while (current && nodes.length < MAX_NODES) {{
      if ((!activeDialog || activeDialog.contains(current)) && isVisible(current) && isInteractive(current) &&
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
          role: (current.getAttribute("role") ||
            (current.tagName==="INPUT" && current.matches('input[type="search"],input[name="search"],input[name="search_query"]') && !isSensitiveField(current) ? "searchbox" :
            current.isContentEditable ? "textbox" :
            ((current.tagName === "INPUT" && ["checkbox", "radio"].includes(typeAttr))
              ? typeAttr : current.tagName))).toLowerCase(),
          label: productLinkLabel(current) || paginationLabel(current) || computeAccessibleName(current),
          options: current.tagName === "SELECT"
            ? Array.from(current.options)
                .filter(option => !option.disabled && !option.parentElement?.disabled)
                .map(option => (option.textContent || "").replace(/\s+/g, " ").trim().slice(0, 100))
                .filter(Boolean).slice(0, 32)
            : [],
          checked: (current.tagName === "INPUT" && ["checkbox", "radio"].includes(typeAttr))
            ? current.checked === true
            : ["checkbox", "radio", "switch"].includes((current.getAttribute("role") || "").toLowerCase())
              ? (current.getAttribute("aria-checked") === "true" ? true
                : current.getAttribute("aria-checked") === "false" ? false : null)
              : null,
          // Read only the presence of a non-sensitive search query, not its value.
          search_state: (() => {{
            if (isSensitiveField(current) || sensitiveForm) return null;
            // Read back only a visible, already-enumerated public dropdown
            // option label. Never expose arbitrary input values.
            if (current.tagName === "SELECT" && !current.disabled) {{
              const option = current.selectedOptions?.[0];
              const label = (option?.textContent || "").replace(/\s+/g, " ").trim().slice(0, 100);
              return label && !option.disabled ? "selected_option:" + label : null;
            }}
            // Capture an ISO-only journey date on a visible public route
            // form with both route fields. Never read arbitrary inputs.
            if(current.tagName==="INPUT" && (/\b(?:journey date|date of journey|choose date|enter date)\b|dd\/mm\/yyyy/i.test(computeAccessibleName(current)+" "+(current.placeholder||"")))){{
              const routeInputs=Array.from(document.querySelectorAll('input[role="searchbox"],input[role="combobox"],input[aria-autocomplete]')).filter(n=>isVisible(n)&&!isSensitiveField(n));
              if(routeInputs.some(n=>/\bfrom\b/i.test(computeAccessibleName(n)))&&routeInputs.some(n=>/\bto\b/i.test(computeAccessibleName(n)))){{
                const raw=String(current.value||"").trim();
                const iso=raw.match(/^(\d{{4}})-(\d{{2}})-(\d{{2}})$/);
                const dmy=raw.match(/^(\d{{2}})\/(\d{{2}})\/(\d{{4}})$/);
                const value=iso?iso[0]:dmy?dmy[3]+"-"+dmy[2]+"-"+dmy[1]:null;
                if(value&&Number.isFinite(Date.parse(value+"T00:00:00Z"))&&new Date(value+"T00:00:00Z").toISOString().slice(0,10)===value)return "public_date:"+value;
              }}
            }}
            if (current.tagName === "INPUT" && (current.readOnly || current.hasAttribute("readonly"))) return "read_only";
            const selector = 'input[type="search"], input[role="searchbox"], input[name="search"], input[name="search_query"], input[aria-label*="search" i], input[placeholder*="search" i], textarea[role="searchbox"], textarea[name="search"], textarea[name="search_query"], textarea[aria-label*="Search"], textarea[placeholder*="Search"], textarea[name="q"]';
            // Public comboboxes and route fields may use textbox semantics.
            // Capture occupancy only, never arbitrary user-entered values.
            const publicRoute = current.tagName === "INPUT" &&
                /^(?:enter |select )?(?:from|to|origin|destination|departure|arrival)(?: station| city| location)?$/i.test(computeAccessibleName(current).trim());
            if ((current.matches(selector) || current.matches('input[role="combobox"], input[aria-autocomplete="list"], input[aria-autocomplete="both"]') || publicRoute) && !current.disabled && typeAttr !== "hidden") {{
              return String(current.value || "").trim() ? "filled" : "empty";
            }}
            if (formSubmit && form) {{
              const fields = Array.from(form.querySelectorAll(selector))
                .filter(field => !field.disabled && field.type !== "hidden" && !isSensitiveField(field));
              if (fields.length && fields.every(field => !String(field.value || "").trim()))
                return "blocked_submit";
            }}
            return null;
          }})(),
          // Retain same-origin destinations but strip arbitrary URL query
          // parameters, which might include OAuth codes or other secrets.
          // YouTube watch links retain only the video ID for DOM-based clicks.
          href: current.tagName === "A"
            ? (() => {{
                try {{
                  const discovered=publicSearchProduct(current);
                  if(discovered)return discovered;
                  const address = new URL(current.href);
                  if(/^https?:$/.test(address.protocol)&&address.origin===location.origin&&/(?:\/product\/[^/]+\/?$|\/p\/[^/]+\/?$)/i.test(address.pathname))return sameOriginProduct(current);
                  if(address.origin==="https://www.google.com"&&["/url","/goto"].includes(address.pathname))return null;
                  if (!/^https?:$/.test(address.protocol)) return null;
                  if(address.origin!==location.origin){{
                    // Only an explicit, read-only single-link objective may
                    // capture a public outbound destination. Never capture
                    // query strings, fragments, credentials or account links.
                    if(EXPLICIT_LINK_SOURCE!==location.href||address.protocol!=="https:"||
                        address.username||address.password||address.search||address.hash||
                        address.pathname.length>200||current.hasAttribute("download")||
                        /(?:login|sign[-_]?in|auth|oauth|token|password|account|checkout|payment|consent)/i.test(address.pathname)||
                        document.querySelectorAll('a[href]').length!==1)return null;
                    return address.origin+address.pathname;
                  }}
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

  // Scope the current observation to the top modal. Once dismissed, the
  // next snapshot exposes the rest of the page without guessing controls.
  let hitId=null;
  if (activeDialog) {{
    walk(activeDialog);
  }} else {{
  // A visual hint is not an action. Sample exactly one point on the same
  // viewport and prioritize the real semantic control under that point.
  // No arbitrary div, frame, canvas, hidden control or sensitive form.
  if(POINT_PROBE){{
    const v=POINT_PROBE.viewport,p=POINT_PROBE.point;
    if(location.href===v.origin&&innerWidth===v.width&&innerHeight===v.height&&scrollX===v.scroll_x&&scrollY===v.scroll_y&&devicePixelRatio===v.scale){{
      const x=p.x*innerWidth/1000,y=p.y*innerHeight/1000;
      let hit=document.elementFromPoint(x,y);
      for(let depth=0;hit?.shadowRoot?.elementFromPoint&&depth<8;depth++){{
        const child=hit.shadowRoot.elementFromPoint(x,y);if(!child||child===hit)break;hit=child;
      }}
      for(let depth=0;hit&&depth<7;depth++,hit=hit.parentElement||hit.getRootNode().host||null){{
        if(hit.matches('iframe,canvas,[contenteditable="true"],[inert],[hidden],[aria-hidden="true"]'))break;
        if(!isInteractive(hit))continue;
        if(!hit.matches('a[href],button,input,[role="button"],[role="link"],[role="checkbox"],[role="searchbox"]')||!isVisible(hit)||isSensitiveField(hit)||hit.closest('[inert],[hidden],[aria-hidden="true"]'))break;
        walk(hit,true);
        const id=hit.getAttribute('data-aether-id');
        if(nodes.some(n=>n.node_id===id&&!n.is_sensitive&&!n.is_submit))hitId=id;
        break;
      }}
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
  // Prioritize real form fields independent of site layout or navigation size.
  for (const field of Array.from(document.querySelectorAll('input:not([type="hidden"]), textarea, select, [role="combobox"], [role="searchbox"], [role="option"]')).slice(0, 60)) {{
    if (isVisible(field) && !isSensitiveField(field)) walk(field, true);
  }}
  // Direct public retailer result anchors from the public search page. No
  // tracking parameters, redirect unwrapping, or arbitrary cross-origin URLs.
  if(["https://www.google.com","https://search.brave.com"].includes(location.origin)&&location.pathname==="/search"){{
    for(const a of Array.from(document.querySelectorAll('a[href]')).slice(0,1500)){{
      if(nodes.length>=Math.min(MAX_NODES,70))break;
      if(publicSearchProduct(a))walk(a,true);
    }}
  }}
  // Reserve slots for product links before large filter panels, navigation
  // menus or facets consume the node budget. Only rendered same-origin links;
  // the product-path shape is a candidate hint, not proof of identity.
  const distinctProducts=new Map();
  for(const link of Array.from(document.querySelectorAll('a[href]')).slice(0,2500)){{
    if(!isVisible(link)||!isInteractive(link)||link.closest('nav,header,footer,form,[aria-hidden="true"]'))continue;
    const url=sameOriginProduct(link);if(!url)continue;
    // Prefer the card's grounded title, then a named anchor. A row with
    // image/title/rating duplicates consumes only one reserved slot.
    const title=productLinkLabel(link);
    const name=computeAccessibleName(link).trim();
    const quality=title?3:name.length>=12?2:name?1:0;
    const previous=distinctProducts.get(url);
    if(!previous||quality>previous.quality)distinctProducts.set(url,{{link,quality}});
  }}
  for(const item of distinctProducts.values()){{
    if(nodes.length>=Math.min(MAX_NODES,85))break;
    walk(item.link,true);
  }}
  // Reserve bounded slots for semantic pagination buttons. Do not infer
  // meaning from icon shapes, class names or arbitrary page text.
  if(/(?:\/products|\/search(?:\/|$)|\/search-results)/i.test(location.pathname)||new URL(location.href).searchParams.has("q")){{
    for(const el of Array.from(document.querySelectorAll('button,[role="button"]')).slice(0,1200)){{
      if(nodes.length>=Math.min(MAX_NODES,95))break;
      if(isVisible(el)&&!el.closest('[aria-hidden="true"],[hidden]')&&/^(next page|go to next page|next results page|next)$/i.test(computeAccessibleName(el).trim())&&el.closest('nav[aria-label*="pag" i],[class*="pagination" i],[data-testid*="pagination" i]'))walk(el,true);
    }}
  }}
  // Catalog filter lists can be hundreds of entries long. Keep a bounded
  // sample of rendered processor/model facets before the generic traversal.
  // This is observation only; native catalog intent must authorize a specific
  // exact matching chip before changing a filter.
  if(/(?:\/products|\/search(?:\/|$)|\/search-results)/i.test(location.pathname)||new URL(location.href).searchParams.has("q")){{
    let chipFacets=0;
    for(const field of Array.from(document.querySelectorAll('[role="checkbox"],input[type="checkbox"]')).slice(0,2200)){{
      if(chipFacets>=20||nodes.length>=Math.min(MAX_NODES,105))break;
      if(!isVisible(field)||isSensitiveField(field)||field.closest('nav,header,footer,[aria-hidden="true"]'))continue;
      const label=computeAccessibleName(field).trim();
      if(!/^(?:[A-Za-z]+\s+)?[A-Za-z]{{1,3}}\d{{1,3}}(?:\s+chip)?(?:[ /-]+\d{{1,2}}\s*c(?:ore)?\s*(?:cpu|gpu))*$/i.test(label))continue;
      walk(field,true);chipFacets++;
    }}
  }}
  // Capture visible multiline editors before toolbar controls exhaust the budget.
  for (const editor of Array.from(document.querySelectorAll('textarea, [contenteditable="true"]')).slice(0, 24)) {{
    if (isVisible(editor) && !isSensitiveField(editor) && !editor.closest('form') &&
        editor.getBoundingClientRect().height >= 72) walk(editor, true);
  }}
  // Reserve early capacity for visible public documentation and metadata
  // links. Large navigation trees and repository file lists must not crowd
  // out the links that lead to install/compatibility evidence.
  for (const link of Array.from(document.querySelectorAll('main a[href], [role="main"] a[href], article a[href]')).slice(0,1200)) {{
    if (nodes.length >= Math.min(MAX_NODES,48)) break;
    if (!isVisible(link) || link.closest('form,[role="menu"],[aria-hidden="true"]')) continue;
    try {{
      const u=new URL(link.href);
      if(u.origin!==location.origin || u.search || u.username || u.password)continue;
      const label=computeAccessibleName(link).trim();
      const file=u.pathname.split('/').pop()||"";
      if(/^(?:readme(?:\.(?:md|rst|txt))?|pyproject\.toml|setup\.(?:py|cfg)|(?:install|installation)(?:\.(?:md|rst|html))?|docs|documentation)$/i.test(file)||
        /^(?:current(?:\s*\(\d+(?:\.\d+)?\))?|(?:(?:part\s+)?[ivxlcdm]+\.\s*)?(?:the\s+)?sql language|(?:[ivxlcdm]+\.\s*)?sql commands|installation|getting started)$/i.test(label)){{
        walk(link,true);
      }}
    }}catch(_e){{}}
  }}
  // Prioritize *general page semantics*, not site-specific selectors. Many
  // websites place hundreds of links in their header/sidebar before the
  // relevant article, results, search form, or dialog. Scan those regions
  // first within the bounded observation budget, then the rest of the page.
  for (const selector of [
    '[role="listbox"]', '[role="search"]', 'form[role="search"]',
    'main', '[role="main"]', '#main', '#content', '.search-results',
    '[role="dialog"]'
  ]) {{
    const region = document.querySelector(selector);
    if (region && isVisible(region)) walk(region);
  }}
  walk(document.body || document.documentElement);
  }}

  // Give the actual content/dialog priority over enormous navigation menus,
  // while retaining a bounded slice of the remaining page for context.
  const primaryTextRoot = document.querySelector(
    '[role="dialog"][aria-modal="true"], main, [role="main"], article, #main, #content');
  const contentText = isVisible(primaryTextRoot)
    ? (primaryTextRoot.innerText || "") : "";
  const bodyText = document.body?.innerText || "";
  // Preserve actual readable content ahead of long menus and infoboxes.
  // Semantic HTML paragraphs and definition/list items are page observations,
  // not instructions. Do not read hidden nodes or input values.
  let prose = "";
  if (!activeDialog && isVisible(primaryTextRoot)) {{
    const blocks = [];
    const seenBlocks=new Set();
    // Read embedded documents (README, help article, API reference) ahead of
    // surrounding repository file trees and sidebars. Use rendered content
    // only; never fetch raw files or hidden metadata.
    const contentRoots=Array.from(primaryTextRoot.querySelectorAll('article,[role="article"],.markdown-body,[itemprop="text"]'))
      .filter(el=>isVisible(el)&&!el.closest('nav,aside,form,[aria-hidden="true"]'))
      .filter(el=>(el.innerText||"").trim().length>=80).slice(0,12);
    const orderedRoots=[...contentRoots,primaryTextRoot];
    for(const root of orderedRoots){{
    for (const el of root.querySelectorAll('p, dd, li, pre, blockquote, [role="code"]')) {{
      if (blocks.length >= 160 || !isVisible(el) ||
          el.closest('nav, aside, header, footer, table, form, [role="navigation"], [role="menu"], [role="listbox"], [role="dialog"], [aria-hidden="true"]')) continue;
      if (el.querySelector('p, dd, li, pre')) continue;
      const text = (el.innerText || "").replace(/\s+/g, " ").trim();
      if ((text.length>=45 || ((el.tagName==="PRE"||el.getAttribute("role")==="code")&&text.length>=12&&text.length<=800))
          && !seenBlocks.has(text)){{seenBlocks.add(text);blocks.push(text);}}
      if (blocks.join(" ").length >= MAX_TEXT) break;
    }}
    if(blocks.length>=160||blocks.join(" ").length>=MAX_TEXT)break;
    }}
    prose = blocks.join(" ");
    if (prose.length < 180) prose = "";
  }}
  // Observe a primary product heading and its nearest bounded offer
  // ancestor. Reject multiple distinct prices unless a single explicit
  // selling-price node identifies the active offer.
  let productHeading=null;
  let productPriceStatus=null;
  const product = (() => {{
    if(activeDialog)return null;
    const eligible=el=>isVisible(el)&&!el.closest('nav,header,footer,aside,[aria-hidden="true"]');
    let hs=Array.from(document.querySelectorAll('h1,[role="heading"][aria-level="1"]')).filter(eligible);
    if(!hs.length&&/(?:\/product\/[^/]+\/?$|\/p\/[^/]+\/?$)/i.test(location.pathname)){{
      hs=Array.from(document.querySelectorAll('[class*="product-title"],[class*="productTitle"],[class*="product-name"],[data-testid*="product-title"],[itemprop="name"]')).filter(eligible)
        .filter(el=>!el.closest('a[href],[class*="product-card"],[class*="productCard"],[class*="product-tile"]'));
    }}
    // Nested same-text heading wrappers are a single observed title.
    hs=hs.filter((h,i)=>!hs.some((x,j)=>j!==i&&h.contains(x)));
    if(hs.length!==1)return null;
    const h=hs[0],heading=(h.innerText||"").replace(/\s+/g," ").trim();
    if(heading.length<20||heading.length>360)return null;
    if(/(?:\/product\/[^/]+\/?$|\/p\/[^/]+\/?$)/i.test(location.pathname))productHeading=heading;
    productPriceStatus="no_scoped_selling_price";
    const re=/(?:\u20b9|Rs\.|INR)\s*\d[\d,]*(?:\.\d{{2}})?(?![\d.])/g;
    for(let el=h.parentElement,depth=0;el&&el!==document.body&&depth<7;el=el.parentElement,depth++){{
      const context=(el.innerText||"").replace(/\s+/g," ").trim();
      if(context.length>1600||!context.includes(heading))break;
      if(el.querySelectorAll('h1,h2,h3,[role="heading"]').length>5)break;
      if(!/\b(?:buy now|add to cart|add to basket|mrp|inclusive of all taxes|selling price|offer price)\b/i.test(context)&&
        !el.querySelector('[class*="product-price"],[class*="selling-price"],[data-testid*="price"],[itemprop="price"]'))continue;
      const matches=Array.from(context.matchAll(re)).map(m=>m[0]);
      if(!matches.length)continue;
      // Stop before crossing into a separate recommendation/card region.
      // Do not read hidden schema, data attributes or account-specific offers.
      if(Array.from(el.querySelectorAll('article,[class*="product-card"],[class*="productCard"],[class*="recommendation"],[class*="related-product"]')).some(n=>isVisible(n)&&!n.contains(h)))break;
      const excluded=n=>{{
        if(!isVisible(n)||n.closest('del,s,strike,[hidden],[aria-hidden="true"]'))return true;
        for(let x=n;x&&x!==el;x=x.parentElement){{
          const meta=(String(x.className||"")+" "+(x.getAttribute("aria-label")||"")).toLowerCase();
          if(/(?:^|[^a-z])(?:mrp|original-price|list-price|was-price|emi|installment|cashback|savings|discount-amount|exchange-price|monthly)(?:[^a-z]|$)/.test(meta))return true;
          if(getComputedStyle(x).textDecorationLine.includes("line-through"))return true;
        }}
        const t=(n.innerText||"").replace(/\s+/g," ").trim();
        return /\b(?:mrp|list price|original price|was price|emi|installments?|cashback|save|savings|per month|monthly|exchange|off)\b|\/\s*(?:mo|month)\b/i.test(t);
      }};
      const offers=Array.from(el.querySelectorAll('[class*="selling-price"],[class*="sale-price"],[class*="offer-price"],[class*="product-price"],[data-testid*="selling-price"],[itemprop="price"]')).filter(n=>!excluded(n));
      const marked=offers.flatMap(n=>Array.from((n.innerText||"").replace(/\s+/g," ").trim().matchAll(re)).map(m=>m[0]));
      const unique=[...new Set(marked)];
      const all=[...new Set(matches)];
      // Unmarked single amounts are admissible only in an explicitly
      // labelled selling-price context without a conditional price signal.
      const conditional=/\b(?:mrp|list price|original price|was price|emi|installments?|cashback|save|savings|per month|monthly|exchange|off|starting at|from)\b|\/\s*(?:mo|month)\b/i.test(context);
      const price=unique.length===1?unique[0]:unique.length===0&&all.length===1&&!conditional&&/\b(?:selling price|offer price|inclusive of all taxes)\b/i.test(context)?all[0]:null;
      if(price&&context.includes(price)){{productPriceStatus="observed_selling_price";return {{heading,price,context}};}}
      productPriceStatus=all.length>1||conditional?"ambiguous_or_conditional_price":"no_scoped_selling_price";
    }}
    return null;
  }})();
  // A read-only reference task can request exact lexical anchors. Read
  // rendered prose only, not hidden markup, inputs, or model-generated text.
  // Preserve literal sentences and place them before long navigation text.
  const focused = (()=>{{
    if(activeDialog||!FOCUS_TERMS.length)return "";
    const root=isVisible(primaryTextRoot)?primaryTextRoot:document.body;
    if(!root)return "";
    const found=[];
    for(const term of FOCUS_TERMS.slice(0,2)){{
      const matching=[];
      for(const el of Array.from(root.querySelectorAll('p,dd,li,blockquote')).slice(0,2500)){{
        if(!isVisible(el)||el.closest('nav,aside,header,footer,form,[hidden],[inert],[aria-hidden="true"],[role="navigation"],[role="dialog"],[contenteditable]')||el.querySelector('p,dd,li,blockquote'))continue;
        const text=(el.innerText||"").replace(/\s+/g," ").trim();
        if(text.length>12000||!text.toLowerCase().includes(term))continue;
        for(const sentence of text.match(/[^.!?]+[.!?]?/g)||[]){{
          const value=sentence.trim(),lower=value.toLowerCase();
          if(value.length<30||value.length>240||!lower.includes(term)||matching.includes(value))continue;
          const tail=lower.slice(lower.indexOf(term)+term.length);
          const direct=/^\s*(?:simply\s+|typically\s+|normally\s+)?(?:avoids?|skips?|updates?|performs?|inserts?|returns?|prevents?|causes?|means|does not|ignores?|allows?)\b/.test(tail);
          matching.push(value);
          if(direct){{found.push(value);break;}}
        }}
        if(matching.length>=10||found.length&&found[found.length-1].toLowerCase().includes(term))break;
      }}
      for(const value of matching.slice(0,4))if(!found.includes(value))found.push(value);
    }}
    return found.join(" ");
  }})();
  const visibleText = (activeDialog ? ["Active dialog: " + (activeDialog.innerText || "")] : [product?.context, productHeading, focused, prose, contentText, bodyText])
    .filter(Boolean).join(" ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_TEXT);

  // Structural signals supplement text. Only visible bounded regions count;
  // neither CSS names nor hidden schema alone prove a result or product.
  const structure = (() => {{
    const root = isVisible(primaryTextRoot) ? primaryTextRoot : (document.body || document.documentElement);
    const outsideChrome = el => isVisible(el) && !el.closest('nav,header,footer,aside,[role="navigation"],[aria-hidden="true"]');
    const scoped = selector => Array.from(root.querySelectorAll(selector)).slice(0,320).filter(outsideChrome);
    const counts = selector => Math.min(120,scoped(selector).length);
    const regionSelector = '[role="feed"],[role="list"],.search-results,[class*="search-results"],[data-testid*="search-results"],[class*="result-list"],[class*="results-list"]';
    const regions = scoped(regionSelector);
    const links = new Set();
    for(const region of regions.slice(0,16)){{
      for(const a of Array.from(region.querySelectorAll('a[href]')).slice(0,80)){{
        if(outsideChrome(a) && (a.innerText||a.querySelector('img[alt]')?.alt||"").trim().length>=3) links.add(a);
      }}
    }}
    const pricePattern = /(?:\u20b9|Rs\.?|INR|USD|EUR|GBP|\$|\u20ac|\u00a3)\s*\d[\d,.]*/i;
    const cards = scoped('[itemtype*="schema.org/Product"],[data-testid*="product-card"],[class*="product-card"],[class*="productCard"],[class*="product-item"],[class*="product-tile"],li[class*="product"],article');
    let productCards=0,pricedCards=0;
    for(const el of cards){{
      if(cards.some(other=>other!==el&&el.contains(other)))continue;
      const txt=(el.innerText||"").trim().slice(0,1200);
      const identity=el.querySelector('h2,h3,h4,[itemprop="name"],a[href]');
      if(!identity||!outsideChrome(identity)||txt.length<12||!pricePattern.test(txt))continue;
      pricedCards++;
      if(el.matches('[itemtype*="schema.org/Product"],[data-testid*="product-card"],[class*="product-card"],[class*="productCard"],[class*="product-item"],[class*="product-tile"],li[class*="product"]'))productCards++;
    }}
    const offers=scoped('[itemprop="price"],[itemtype*="schema.org/Offer"],[class*="product-price"],[data-testid*="price"],[class*="selling-price"]');
    const offerBlocks=offers.filter(el=>pricePattern.test((el.innerText||"").trim().slice(0,180))).length;
    return {{
      product,
      product_heading:productHeading,
      product_price_status:productPriceStatus,
      headings: counts('h1,h2,h3,h4,[role="heading"]'),
      paragraphs: scoped('p,dd').filter(el=>(el.innerText||"").trim().length>=35).length,
      code_blocks: counts('pre,[role="code"]'),
      articles: counts('article,[role="article"]'),
      result_regions: regions.length,
      result_links: Math.min(120,links.size),
      product_cards: Math.min(120,productCards),
      priced_cards: Math.min(120,pricedCards),
      offer_blocks: Math.min(120,offerBlocks),
      loading: document.readyState==="loading" || root.getAttribute('aria-busy')==="true" ||
        scoped('[role="progressbar"],[aria-busy="true"]').some(el=>/load|progress/i.test((el.getAttribute('aria-label')||"")+" "+(el.innerText||"").slice(0,80)))
    }};
  }})();
  const payload = {{
    request_id: REQUEST_ID,
    snapshot: {{
      origin: window.location.href,
      title: document.title || window.location.hostname || "Untitled",
      text_excerpt: visibleText,
      interactive_nodes: nodes,
      structure
    }}
  }};

  // Return to the HOST WebView2 ExecuteScript callback. Untrusted pages
  // are never granted Tauri commands or asked to send their own snapshots.
  return POINT_PROBE?{{snapshot:payload.snapshot,hit_id:hitId}}:payload.snapshot;
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
    let script = build_snapshot_script(Uuid::new_v4(), 120, 24000);
    let value = crate::agent::native_eval::eval_json(
        app, webview_label, script, timeout_ms
    ).await.ok()?;
    serde_json::from_value(value).ok()
}


// Only the compiled explicit-link task requests this observation. Ordinary
// snapshots continue to suppress cross-origin hrefs.
pub async fn request_explicit_link_snapshot(app:&tauri::AppHandle,label:&str,source:&str,timeout_ms:u64)->Option<DomSnapshot>{
    let u=super::browser_skills::public_url(source)?;
    if u.scheme()!="https"||u.query().is_some()||u.fragment().is_some(){return None;}
    let payload=serde_json::to_string(source).ok()?;
    let script=build_snapshot_script(Uuid::new_v4(),120,24000).replace("const EXPLICIT_LINK_SOURCE = null;",&format!("const EXPLICIT_LINK_SOURCE = {payload};"));
    let value=crate::agent::native_eval::eval_json(app,label,script,timeout_ms).await.ok()?;
    serde_json::from_value(value).ok()
}

// The ordinary serializer remains the authority for all fields; only its
// observation order is changed by an independently localized viewport point.
pub async fn request_point_snapshot(app:&tauri::AppHandle,label:&str,point:super::multimodal_perception::VisualPoint,viewport:&super::multimodal_perception::Viewport)->Option<(DomSnapshot,String)>{
    if !point.valid()||!viewport.valid(){return None;}
    #[derive(Deserialize)]
    struct Hit{snapshot:DomSnapshot,hit_id:Option<String>}
    let payload=serde_json::json!({"point":point,"viewport":viewport});
    let script=build_snapshot_script(Uuid::new_v4(),120,24000).replace("const POINT_PROBE = null;",&format!("const POINT_PROBE = {payload};"));
    let value=crate::agent::native_eval::eval_json(app,label,script,1800).await.ok()?;
    let result:Hit=serde_json::from_value(value).ok()?;
    Some((result.snapshot,result.hit_id?))
}


/// Goal-scoped native evidence capture. Terms originate from the user's
/// compiled read-only contrast, not from an untrusted model or web page.
pub async fn request_reference_snapshot(app:&tauri::AppHandle,label:&str,terms:&[String])->Option<DomSnapshot>{
    if terms.is_empty()||terms.len()>2||terms.iter().any(|s|s.is_empty()||s.chars().count()>60||!s.chars().all(|c|c.is_alphanumeric()||matches!(c,' '|'-'|'_'))){return None;}
    let payload=serde_json::to_string(terms).ok()?;
    let script=build_snapshot_script(Uuid::new_v4(),120,24000).replace("const FOCUS_TERMS = [];",&format!("const FOCUS_TERMS = {payload};"));
    let value=crate::agent::native_eval::eval_json(app,label,script,2500).await.ok()?;
    serde_json::from_value(value).ok()
}
