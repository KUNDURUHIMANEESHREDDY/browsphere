use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItpVerdict {
    pub host: String,
    pub blocked_trackers: u32,
    pub third_party_storage_partitioned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItpStats {
    pub total_blocked: u64,
    pub active_rules: usize,
    pub third_party_storage_partitioned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRecord {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub saved_path: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub status: String,
    pub error: Option<String>,
}

pub struct ItpEngine {
    tracker_domains: Vec<&'static str>,
    tracking_params: Vec<&'static str>,
    total_blocked: AtomicU64,
}

impl ItpEngine {
    pub fn new() -> Self {
        Self {
            tracker_domains: vec![
                "doubleclick.net",
                "googlesyndication.com",
                "google-analytics.com",
                "googletagmanager.com",
                "googleadservices.com",
                "hotjar.com",
                "segment.io",
                "segment.com",
                "mixpanel.com",
                "amplitude.com",
                "facebook.net",
                "connect.facebook.net",
                "clarity.ms",
                "fullstory.com",
                "sentry.io",
                "ads.twitter.com",
                "analytics.tiktok.com",
                "bat.bing.com",
                "scorecardresearch.com",
                "quantserve.com",
                "criteo.com",
                "taboola.com",
                "outbrain.com",
                "adnxs.com",
                "demdex.net",
                "krxd.net",
                "bluekai.com",
            ],
            tracking_params: vec![
                "utm_source",
                "utm_medium",
                "utm_campaign",
                "utm_term",
                "utm_content",
                "fbclid",
                "gclid",
                "mc_cid",
                "mc_eid",
                "msclkid",
                "twclid",
                "_ga",
            ],
            total_blocked: AtomicU64::new(0),
        }
    }

    pub fn is_tracker_host(&self, host: &str) -> bool {
        let h = host.trim().to_lowercase();
        if h.is_empty() {
            return false;
        }
        self.tracker_domains
            .iter()
            .any(|t| h == *t || h.ends_with(&format!(".{t}")))
    }

    pub fn record_block(&self, count: u64) -> u64 {
        self.total_blocked.fetch_add(count, Ordering::Relaxed) + count
    }

    pub fn stats(&self) -> ItpStats {
        ItpStats {
            total_blocked: self.total_blocked.load(Ordering::Relaxed),
            active_rules: self.tracker_domains.len(),
            third_party_storage_partitioned: true,
        }
    }

    /// Evaluates top-level WebView2 navigations at the Rust `on_navigation` hook.
    pub fn allow_navigation(&self, url: &Url) -> bool {
        match url.scheme() {
            "http" | "https" | "about" => {}
            _ => return false,
        }
        if let Some(host) = url.host_str() {
            if self.is_tracker_host(host) {
                self.record_block(1);
                return false;
            }
        }
        true
    }

    pub fn check_url(&self, raw_url: &str) -> ItpVerdict {
        let host = Url::parse(raw_url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_lowercase()))
            .unwrap_or_else(|| "unknown".to_string());

        let blocked = if self.is_tracker_host(&host) {
            self.record_block(1);
            1
        } else {
            0
        };

        ItpVerdict {
            host,
            blocked_trackers: blocked,
            third_party_storage_partitioned: true,
        }
    }

    /// Generates an initialization script injected into every child WebView2 before page scripts run.
    pub fn build_network_guard_script(&self) -> String {
        let domains_json =
            serde_json::to_string(&self.tracker_domains).unwrap_or_else(|_| "[]".to_string());
        let params_json =
            serde_json::to_string(&self.tracking_params).unwrap_or_else(|_| "[]".to_string());

        format!(
            r#"(function() {{
  if (window.__AETHER_ITP_ACTIVE__) return;
  window.__AETHER_ITP_ACTIVE__ = true;

  const TRACKERS = {domains_json};
  const TRACKING_PARAMS = new Set({params_json});
  const reportedKeys = new Set();

  function isTrackerHost(hostname) {{
    if (!hostname) return false;
    const h = hostname.toLowerCase();
    for (let i = 0; i < TRACKERS.length; i++) {{
      const t = TRACKERS[i];
      if (h === t || h.endsWith("." + t)) return true;
    }}
    return false;
  }}

  function parseUrl(input) {{
    try {{
      return new URL(String(input), window.location.href);
    }} catch (_) {{
      return null;
    }}
  }}

  function stripTrackingParams(urlObj) {{
    let changed = false;
    for (const key of Array.from(urlObj.searchParams.keys())) {{
      if (TRACKING_PARAMS.has(key.toLowerCase())) {{
        urlObj.searchParams.delete(key);
        changed = true;
      }}
    }}
    return changed ? urlObj.toString() : null;
  }}

  function reportBlocked(blockedHost, resourceType) {{
    const key = blockedHost + "|" + resourceType;
    if (reportedKeys.has(key) && reportedKeys.size > 200) return;
    reportedKeys.add(key);

    const payload = {{
      origin: window.location.href,
      blocked_host: blockedHost,
      resource_type: resourceType
    }};
    try {{
      if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {{
        window.__TAURI__.core.invoke("report_itp_blocked", payload).catch(() => {{}});
      }} else if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
        window.__TAURI_INTERNALS__.invoke("report_itp_blocked", payload).catch(() => {{}});
      }}
    }} catch (_) {{}}
  }}

  function sanitizeElementIfTracker(node) {{
    try {{
      if (!node || !node.tagName) return false;
      const tag = node.tagName.toUpperCase();
      if (tag === "SCRIPT" || tag === "IMG" || tag === "IFRAME" || tag === "LINK") {{
        const src = node.getAttribute ? (node.getAttribute("src") || node.getAttribute("href")) : null;
        const parsed = src ? parseUrl(src) : null;
        if (parsed && isTrackerHost(parsed.hostname)) {{
          if (tag === "SCRIPT") node.type = "javascript/blocked";
          if (node.removeAttribute) {{
            node.removeAttribute("src");
            if (tag === "LINK") node.removeAttribute("href");
          }}
          reportBlocked(parsed.hostname, tag.toLowerCase());
          return true;
        }}
      }}
    }} catch (_) {{}}
    return false;
  }}

  // 1. Intercept fetch()
  const origFetch = window.fetch;
  if (typeof origFetch === "function") {{
    window.fetch = function(input, init) {{
      const rawUrl = typeof input === "string" ? input : (input && input.url ? input.url : "");
      const parsed = parseUrl(rawUrl);
      if (parsed && isTrackerHost(parsed.hostname)) {{
        reportBlocked(parsed.hostname, "fetch");
        return Promise.resolve(new Response("", {{ status: 204, statusText: "Blocked by Browsphere ITP" }}));
      }}
      if (parsed && typeof input === "string") {{
        const cleaned = stripTrackingParams(parsed);
        if (cleaned) return origFetch.call(this, cleaned, init);
      }}
      return origFetch.call(this, input, init);
    }};
  }}

  // 2. Intercept XMLHttpRequest
  const origXhrOpen = XMLHttpRequest.prototype.open;
  const origXhrSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function(method, url) {{
    const parsed = parseUrl(url);
    if (parsed && isTrackerHost(parsed.hostname)) {{
      this.__aetherBlockedHost = parsed.hostname;
      return;
    }}
    this.__aetherBlockedHost = null;
    if (parsed) {{
      const cleaned = stripTrackingParams(parsed);
      if (cleaned) {{
        arguments[1] = cleaned;
      }}
    }}
    return origXhrOpen.apply(this, arguments);
  }};
  XMLHttpRequest.prototype.send = function() {{
    if (this.__aetherBlockedHost) {{
      reportBlocked(this.__aetherBlockedHost, "xhr");
      return;
    }}
    return origXhrSend.apply(this, arguments);
  }};

  // 3. Intercept navigator.sendBeacon()
  if (navigator && typeof navigator.sendBeacon === "function") {{
    const origBeacon = navigator.sendBeacon.bind(navigator);
    navigator.sendBeacon = function(url, data) {{
      const parsed = parseUrl(url);
      if (parsed && isTrackerHost(parsed.hostname)) {{
        reportBlocked(parsed.hostname, "beacon");
        return true;
      }}
      return origBeacon(url, data);
    }};
  }}

  // 4. Intercept DOM insertion & setAttribute on tracker elements
  const origAppendChild = Element.prototype.appendChild;
  Element.prototype.appendChild = function(node) {{
    if (sanitizeElementIfTracker(node)) return node;
    return origAppendChild.call(this, node);
  }};

  const origInsertBefore = Element.prototype.insertBefore;
  Element.prototype.insertBefore = function(node, refNode) {{
    if (sanitizeElementIfTracker(node)) return node;
    return origInsertBefore.call(this, node, refNode);
  }};

  // 5. Watch for declarative tracker tags injected via innerHTML/outerHTML/templates
  if (typeof MutationObserver !== "undefined") {{
    const obs = new MutationObserver((mutations) => {{
      for (const m of mutations) {{
        for (const added of m.addedNodes) {{
          sanitizeElementIfTracker(added);
          if (added && added.querySelectorAll) {{
            added.querySelectorAll("script, img, iframe, link").forEach(sanitizeElementIfTracker);
          }}
        }}
      }}
    }});
    obs.observe(document.documentElement || document, {{ childList: true, subtree: true }});
  }}
}})();"#,
            domains_json = domains_json,
            params_json = params_json
        )
    }

    pub fn sanitize_filename(&self, filename: &str) -> String {
        let clean_name = filename
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' '))
            .collect::<String>()
            .trim()
            .to_string();
        if clean_name.is_empty() || clean_name == "." || clean_name == ".." {
            "download.bin".to_string()
        } else {
            clean_name
        }
    }

    pub fn prepare_download(&self, raw_url: &str, filename: &str) -> DownloadRecord {
        let final_name = self.sanitize_filename(filename);
        DownloadRecord {
            id: Uuid::new_v4(),
            url: raw_url.to_string(),
            filename: final_name,
            saved_path: String::new(),
            bytes_downloaded: 0,
            total_bytes: None,
            status: "queued".to_string(),
            error: None,
        }
    }
}
