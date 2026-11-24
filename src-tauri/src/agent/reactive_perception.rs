//! BRPR: untrusted page messages are ONLY hints to invalidate native observations.
use crate::agent::dom_extractor::{request_live_dom_snapshot, DomSnapshot};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::Manager;
use tokio::sync::mpsc;

#[derive(Default)]
struct Page {
    generation: u64,
    dirty: bool,
    cached: Option<DomSnapshot>,
    updated: Option<Instant>,
    signals: u64,
}
#[derive(Default)]
struct Inner {
    pages: Mutex<HashMap<String, Page>>,
    durations_us: Mutex<VecDeque<u64>>,
}
#[derive(Clone)]
pub struct ReactivePerception {
    shared: Arc<Inner>,
    wake: mpsc::Sender<String>,
}
#[derive(Serialize)]
pub struct PerceptionStatus {
    pub generation: u64,
    pub dirty: bool,
    pub cached: bool,
    pub age_ms: Option<u64>,
    pub signal_count: u64,
    pub host_p50_us: Option<u64>,
    pub host_p95_us: Option<u64>,
    pub host_p99_us: Option<u64>,
}
impl ReactivePerception {
    pub fn new() -> (Self, mpsc::Receiver<String>) {
        let (wake, rx) = mpsc::channel(128);
        (
            Self {
                shared: Arc::new(Inner::default()),
                wake,
            },
            rx,
        )
    }
    pub fn invalidate(&self, label: &str) {
        let started = Instant::now();
        let notify = {
            let mut pages = self.shared.pages.lock().unwrap();
            if !pages.contains_key(label) && pages.len() >= 64 {
                return;
            }
            let p = pages.entry(label.to_owned()).or_default();
            let notify = !p.dirty;
            p.generation = p.generation.wrapping_add(1);
            p.dirty = true;
            p.signals = p.signals.saturating_add(1);
            notify
        };
        if notify {
            let _ = self.wake.try_send(label.to_owned());
        }
        let elapsed = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        let mut samples = self.shared.durations_us.lock().unwrap();
        if samples.len() == 2048 {
            samples.pop_front();
        }
        samples.push_back(elapsed);
    }
    pub fn generation(&self, label: &str) -> u64 {
        self.shared
            .pages
            .lock()
            .unwrap()
            .get(label)
            .map_or(0, |p| p.generation)
    }
    pub fn accept(&self, label: &str, generation: u64, snapshot: DomSnapshot) -> bool {
        let mut pages = self.shared.pages.lock().unwrap();
        let Some(p) = pages.get_mut(label) else {
            return false;
        };
        if p.generation != generation {
            return false;
        }
        p.cached = Some(snapshot);
        p.updated = Some(Instant::now());
        p.dirty = false;
        true
    }
    pub fn cached_preview(&self, label: &str, max_age: Duration) -> Option<DomSnapshot> {
        let pages = self.shared.pages.lock().unwrap();
        let p = pages.get(label)?;
        if p.dirty || !p.updated.is_some_and(|t| t.elapsed() <= max_age) {
            return None;
        }
        p.cached.clone()
    }
    pub fn dirty_labels(&self) -> Vec<String> {
        self.shared
            .pages
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p)| p.dirty)
            .map(|(label, _)| label.clone())
            .collect()
    }
    pub fn forget(&self, label: &str) {
        self.shared.pages.lock().unwrap().remove(label);
    }
    pub fn status(&self, label: &str) -> PerceptionStatus {
        let (generation, dirty, cached, age_ms, signal_count) = {
            let pages = self.shared.pages.lock().unwrap();
            let p = pages.get(label);
            (
                p.map_or(0, |p| p.generation),
                p.is_none_or(|p| p.dirty),
                p.is_some_and(|p| p.cached.is_some()),
                p.and_then(|p| p.updated)
                    .map(|t| t.elapsed().as_millis() as u64),
                p.map_or(0, |p| p.signals),
            )
        };
        let mut samples: Vec<u64> = self
            .shared
            .durations_us
            .lock()
            .unwrap()
            .iter()
            .copied()
            .collect();
        samples.sort_unstable();
        let pct = |n: usize| {
            if samples.is_empty() {
                None
            } else {
                Some(samples[(samples.len() - 1) * n / 100])
            }
        };
        PerceptionStatus {
            generation,
            dirty,
            cached,
            age_ms,
            signal_count,
            host_p50_us: pct(50),
            host_p95_us: pct(95),
            host_p99_us: pct(99),
        }
    }
}

pub async fn run_refresh_worker(
    app: tauri::AppHandle,
    store: ReactivePerception,
    mut rx: mpsc::Receiver<String>,
) {
    let mut retry = tokio::time::interval(Duration::from_millis(500));
    loop {
        let label = tokio::select! {
            item = rx.recv() => match item { Some(x) => x, None => return },
            _ = retry.tick() => match store.dirty_labels().into_iter().next() { Some(x) => x, None => continue },
        };
        tokio::time::sleep(Duration::from_millis(24)).await;
        let generation = store.generation(&label);
        let placeholder = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        if let Some(snapshot) = request_live_dom_snapshot(&app, &label, &placeholder, 1600).await {
            if !store.accept(&label, generation, snapshot) {
                let _ = store.wake.try_send(label);
            }
        } else if app.get_webview(&label).is_none() {
            store.forget(&label);
        }
    }
}
/// No page content crosses this channel. Page messages can be forged; only refresh
/// via native ExecuteScript is trusted for observation, NEVER for authorization.
pub const OBSERVER_SCRIPT: &str = r#"
(() => {
  if (window !== window.top) return;
  let observer, pending = false, last = 0;
  function send() {
    pending = false; last = performance.now();
    try { window.chrome?.webview?.postMessage("brpr:dirty"); } catch (_) {}
  }
  function dirty() {
    if (pending) return;
    pending = true;
    const wait = Math.max(0, 16 - (performance.now() - last));
    if (wait) setTimeout(send, wait);
    else queueMicrotask(send);
  }
  function install() {
    if (observer || !document.documentElement) return;
    observer = new MutationObserver(changes => {
      if (changes.some(m => m.attributeName !== "data-aether-id")) dirty();
    });
    observer.observe(document.documentElement, {
      childList: true, subtree: true, characterData: true, attributes: true,
      attributeFilter: ["aria-expanded", "aria-hidden", "hidden", "disabled", "style",
                        "class", "role", "title", "open"]
    });
    dirty();
  }
  install();
  document.addEventListener("readystatechange", install);
  for (const name of ["pageshow", "popstate", "hashchange", "resize"]) addEventListener(name, dirty);
  addEventListener("scroll", dirty, {passive: true, capture: true});
})();
"#;

#[cfg(windows)]
pub fn attach_native_hint_receiver(
    child: &tauri::Webview,
    label: String,
    store: ReactivePerception,
) -> Result<(), String> {
    use webview2_com::{CoTaskMemPWSTR, WebMessageReceivedEventHandler};
    child
        .with_webview(move |native| {
            let Ok(core) = (unsafe { native.controller().CoreWebView2() }) else {
                return;
            };
            let handler = WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
                if let Some(args) = args {
                    // COM writes a newly allocated PWSTR. Adopt it only after success,
                    // avoiding a mutable RAII borrow that would outlive its scope.
                    let mut raw = std::mem::MaybeUninit::zeroed();
                    if unsafe { args.TryGetWebMessageAsString(raw.as_mut_ptr()) }.is_ok() {
                        let value = CoTaskMemPWSTR::from(unsafe { raw.assume_init() });
                        if value.to_string() == "brpr:dirty" {
                            store.invalidate(&label);
                        }
                    }
                }
                Ok(())
            }));
            let mut token = Default::default();
            if let Err(err) = unsafe { core.add_WebMessageReceived(&handler, &mut token) } {
                tracing::warn!("BRPR WebView2 registration failed: {err}");
            }
        })
        .map_err(|err| format!("BRPR native hook: {err}"))
}
#[cfg(not(windows))]
pub fn attach_native_hint_receiver(
    _child: &tauri::Webview,
    _label: String,
    _store: ReactivePerception,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> DomSnapshot {
        DomSnapshot {
            structure: Default::default(), origin: "https://example.org/".into(),
            title: "Test".into(),
            text_excerpt: "Sample".into(),
            interactive_nodes: vec![],
        }
    }
    #[test]
    fn invalidation_requires_fresh_snapshot() {
        let (store, _) = ReactivePerception::new();
        store.invalidate("a");
        let generation = store.generation("a");
        assert!(store.accept("a", generation, sample()));
        assert!(store.cached_preview("a", Duration::from_secs(1)).is_some());
        store.invalidate("a");
        assert!(store.cached_preview("a", Duration::from_secs(1)).is_none());
    }
    #[test]
    fn stale_capture_cannot_replace_newer_page() {
        let (store, _) = ReactivePerception::new();
        store.invalidate("a");
        let generation = store.generation("a");
        store.invalidate("a");
        assert!(!store.accept("a", generation, sample()));
        assert!(store.status("a").dirty);
    }
    #[test]
    fn closed_tab_does_not_retain_private_cache() {
        let (store, _) = ReactivePerception::new();
        store.invalidate("private");
        assert!(store.accept("private", store.generation("private"), sample()));
        store.forget("private");
        assert!(!store.status("private").cached);
    }
    #[test]
    fn benchmark_host_invalidation_only() {
        let (store, _) = ReactivePerception::new();
        for _ in 0..10_000 {
            store.invalidate("bench");
        }
        let m = store.status("bench");
        println!(
            "Host invalidation ONLY (not browser reaction): p50={}us p95={}us p99={}us",
            m.host_p50_us.unwrap(),
            m.host_p95_us.unwrap(),
            m.host_p99_us.unwrap()
        );
        assert_eq!(m.signal_count, 10_000);
    }
}
