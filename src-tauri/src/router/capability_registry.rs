//! Model capability discovery, never a model-specific agent runtime.
//! Ollama advertises modality/tool/thinking support via /api/show. Providers
//! that do not publish metadata remain unknown, not optimistically supported.
//! Metadata is not evidence that a model can complete a Lucia browser task.
use anyhow::{anyhow, ensure, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const CACHE_TTL: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelCapabilities {
    pub provider: String,
    pub model: String,
    pub source: String,
    /// None means the provider did not disclose this attribute.
    pub completion: Option<bool>,
    pub vision: Option<bool>,
    pub tools: Option<bool>,
    pub thinking: Option<bool>,
    /// Ollama's capabilities field does not certify JSON-schema adherence.
    pub structured_json: Option<bool>,
    /// Only repeated independently verified task trials can set this.
    pub live_task_verified: bool,
}

impl ModelCapabilities {
    pub fn undisclosed(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            source: "not_disclosed".into(),
            completion: None,
            vision: None,
            tools: None,
            thinking: None,
            structured_json: None,
            live_task_verified: false,
        }
    }

    pub fn from_ollama_show(model: &str, payload: &Value) -> Self {
        let mut profile = Self::undisclosed("ollama", model);
        let Some(values) = payload.get("capabilities").and_then(Value::as_array) else {
            return profile;
        };
        // The advertised server metadata is about supported input modalities,
        // not authority to perform actions, permissions or factual verification.
        let names: HashSet<&str> = values.iter().filter_map(Value::as_str).collect();
        profile.source = "ollama_api_show".into();
        profile.completion = Some(names.contains("completion"));
        profile.vision = Some(names.contains("vision"));
        profile.tools = Some(names.contains("tools"));
        profile.thinking = Some(names.contains("thinking"));
        profile
    }
}

#[derive(Clone)]
struct CacheEntry {
    profile: ModelCapabilities,
    observed_at: Instant,
}

#[derive(Default)]
pub struct CapabilityRegistry {
    entries: Mutex<HashMap<(String, String), CacheEntry>>,
}
impl CapabilityRegistry {
    /// Shared across the native orchestrator, planner and vision specialist.
    pub fn shared() -> &'static Self {
        static REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::default)
    }

    pub async fn discover_ollama(&self, base: &str, model: &str) -> Result<ModelCapabilities> {
        let tag = model.trim();
        ensure!(
            !tag.is_empty() && tag.len() <= 256 && !tag.chars().any(char::is_control),
            "Invalid local model identifier"
        );
        let endpoint = local_show_url(base)?;
        let key = (endpoint.origin().ascii_serialization(), tag.to_string());
        if let Ok(cache) = self.entries.lock() {
            if let Some(hit) = cache.get(&key) {
                if hit.observed_at.elapsed() < CACHE_TTL {
                    return Ok(hit.profile.clone());
                }
            }
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()?;
        let mut response = client
            .post(endpoint)
            .json(&json!({"model": tag}))
            .send()
            .await
            .context("Local capability discovery unavailable")?;
        ensure!(
            response.status().is_success(),
            "Local model metadata unavailable (HTTP {})",
            response.status()
        );
        ensure!(
            !response
                .content_length()
                .is_some_and(|length| length as usize > MAX_RESPONSE_BYTES),
            "Local model metadata exceeds size limit"
        );
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("Reading local model metadata failed")?
        {
            ensure!(
                body.len().saturating_add(chunk.len()) <= MAX_RESPONSE_BYTES,
                "Local model metadata exceeds size limit"
            );
            body.extend_from_slice(&chunk);
        }
        let payload: Value = serde_json::from_slice(&body)
            .map_err(|_| anyhow!("Invalid model capability metadata"))?;
        let profile = ModelCapabilities::from_ollama_show(tag, &payload);
        if let Ok(mut cache) = self.entries.lock() {
            cache.insert(
                key,
                CacheEntry {
                    profile: profile.clone(),
                    observed_at: Instant::now(),
                },
            );
        }
        Ok(profile)
    }
}

fn local_show_url(base: &str) -> Result<Url> {
    let mut url = Url::parse(base.trim()).context("Invalid local model server URL")?;
    ensure!(
        url.scheme() == "http"
            && matches!(
                url.host_str(),
                Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
            )
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Capability discovery only permits credential-free loopback Ollama"
    );
    ensure!(
        url.path() == "/" || url.path().is_empty(),
        "Ollama model server must use a loopback API root"
    );
    url.set_path("/api/show");
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_capabilities_never_claim_task_reliability() {
        let entry = ModelCapabilities::undisclosed("openai-compatible", "some-new-model");
        assert_eq!(entry.completion, None);
        assert_eq!(entry.vision, None);
        assert!(!entry.live_task_verified);
        let older = ModelCapabilities::from_ollama_show("custom", &json!({"model_info":{}}));
        assert_eq!(older.vision, None);
        assert_eq!(older.structured_json, None);
    }
    #[test]
    fn advertised_capabilities_are_parsed_without_model_name_heuristics() {
        let entry = ModelCapabilities::from_ollama_show(
            "arbitrary-local-tag",
            &json!({"capabilities":["completion","vision","tools","thinking"]}),
        );
        assert_eq!(entry.completion, Some(true));
        assert_eq!(entry.vision, Some(true));
        assert_eq!(entry.tools, Some(true));
        assert_eq!(entry.thinking, Some(true));
        assert_eq!(entry.structured_json, None);
        assert!(!entry.live_task_verified);
        let text = ModelCapabilities::from_ollama_show(
            "unknown-custom",
            &json!({"capabilities":["completion"]}),
        );
        assert_eq!(text.vision, Some(false));
        assert_eq!(text.tools, Some(false));
    }
    #[test]
    fn local_discovery_rejects_untrusted_endpoints() {
        for invalid in [
            "https://provider.example/api",
            "http://example.org",
            "http://127.0.0.1:11434/?token=secret",
            "http://user:pass@127.0.0.1:11434",
            "http://127.0.0.1:11434/redirect",
        ] {
            assert!(local_show_url(invalid).is_err(), "{invalid}");
        }
        assert_eq!(
            local_show_url("http://127.0.0.1:11434").unwrap().path(),
            "/api/show"
        );
    }
}
