//! OpenAI-compatible model discovery. Never logs or persists provider secrets.
use anyhow::{anyhow, bail, ensure, Context, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;
use url::Url;

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_MODEL_COUNT: usize = 5000;

/// Derive the model-list URL from a chat-completions endpoint or an API base URL.
pub fn derive_models_url(raw_endpoint: &str) -> Result<Url> {
    let mut url = Url::parse(raw_endpoint.trim()).context("Enter a valid API endpoint URL")?;
    let host = url.host_str().ok_or_else(|| anyhow!("The endpoint needs a hostname"))?;
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1");
    ensure!(
        url.scheme() == "https" || (loopback && url.scheme() == "http"),
        "Remote model endpoints must use HTTPS (HTTP is only allowed on localhost)"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none() &&
        url.query().is_none() && url.fragment().is_none(),
        "Enter an API endpoint without passwords, query parameters or fragments"
    );

    let path = url.path().trim_end_matches('/');
    let destination = if path.ends_with("/models") {
        path.to_string()
    } else if let Some(base) = path.strip_suffix("/chat/completions") {
        format!("{base}/models")
    } else if let Some(base) = path.strip_suffix("/completions") {
        format!("{base}/models")
    } else if let Some(base) = path.strip_suffix("/responses") {
        format!("{base}/models")
    } else if path.is_empty() {
        // OpenAI's API root requires /v1. Most compatible providers do too.
        "/v1/models".to_string()
    } else {
        format!("{path}/models")
    };
    url.set_path(&destination);
    Ok(url)
}

/// Accept OpenAI {data:[{id}]} and compatible proxy {models:[...]} formats.
pub fn parse_model_ids(payload: &Value) -> Result<Vec<String>> {
    let rows = payload.get("data").or_else(|| payload.get("models"))
        .and_then(Value::as_array)
        .or_else(|| payload.as_array())
        .ok_or_else(|| anyhow!("Provider did not return a recognizable models list"))?;
    let mut found = HashSet::new();
    let mut names = Vec::new();
    for row in rows {
        let id = row.as_str().or_else(|| row.get("id").and_then(Value::as_str))
            .or_else(|| row.get("name").and_then(Value::as_str));
        if let Some(name) = id.map(str::trim) {
            if !name.is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
                && found.insert(name.to_string()) {
                names.push(name.to_string());
                if names.len() >= MAX_MODEL_COUNT {
                    break;
                }
            }
        }
    }
    ensure!(!names.is_empty(), "Provider returned no selectable model IDs");
    names.sort_unstable_by_key(|name| name.to_ascii_lowercase());
    Ok(names)
}

/// Fetches models with the saved provider key. Redirects are disabled so an API
/// key is never forwarded to a different URL. Only model IDs are returned.
pub async fn fetch_models(raw_endpoint: &str, api_key: Option<&str>) -> Result<Vec<String>> {
    let url = derive_models_url(raw_endpoint)?;
    let is_local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    ensure!(
        is_local || api_key.is_some_and(|key| !key.is_empty()),
        "Save an API key for this cloud endpoint before fetching models"
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = client.get(url.clone())
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(key) = api_key.filter(|key| !key.trim().is_empty()) {
        request = request.bearer_auth(key.trim());
    }
    let response = request.send().await
        .with_context(|| format!("Could not reach the provider's models API at {url}"))?;
    let status = response.status();
    if status.is_redirection() {
        bail!("Model discovery returned a redirect. Use the provider's final HTTPS API endpoint directly");
    }
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        bail!("Provider rejected the API key (HTTP {status}). Check the key and endpoint");
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        bail!("No models API found at {url}. Enter an OpenAI-compatible API endpoint");
    }
    ensure!(status.is_success(), "Provider's models API returned HTTP {status}");

    if response.content_length().is_some_and(|len| len > MAX_BODY_BYTES as u64) {
        bail!("Provider's models response is too large");
    }
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.context("Could not read models API response")? {
        ensure!(body.len() + chunk.len() <= MAX_BODY_BYTES, "Provider's models response is too large");
        body.extend_from_slice(&chunk);
    }
    let payload: Value = serde_json::from_slice(&body)
        .context("The models API did not return valid JSON")?;
    parse_model_ids(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn builds_provider_models_url_without_changing_the_host() {
        let cases = [
            ("https://api.openai.com/v1/chat/completions", "https://api.openai.com/v1/models"),
            ("https://api.groq.com/openai/v1/chat/completions", "https://api.groq.com/openai/v1/models"),
            ("https://openrouter.ai/api/v1/chat/completions", "https://openrouter.ai/api/v1/models"),
            ("https://proxy.test/v1", "https://proxy.test/v1/models"),
            ("http://127.0.0.1:4000/v1/chat/completions", "http://127.0.0.1:4000/v1/models"),
            ("https://proxy.test/v1/models", "https://proxy.test/v1/models"),
        ];
        for (input, expected) in cases {
            assert_eq!(derive_models_url(input).unwrap().as_str(), expected);
        }
    }

    #[test]
    fn rejects_insecure_remote_urls_and_embedded_secrets() {
        assert!(derive_models_url("http://example.com/v1/chat/completions").is_err());
        assert!(derive_models_url("https://user:pass@example.com/v1/chat/completions").is_err());
        assert!(derive_models_url("https://example.com/v1/chat/completions?key=secret").is_err());
    }

    #[test]
    fn parses_deduplicated_openai_model_ids() {
        let result = parse_model_ids(&serde_json::json!({
            "data": [{"id":"gpt-b"}, {"id":"gpt-a"}, {"id":"gpt-a"}, {"id":""}, {"not_id":"no"}]
        })).unwrap();
        assert_eq!(result, vec!["gpt-a", "gpt-b"]);
        let proxy = parse_model_ids(&serde_json::json!({"models": ["m2", {"id":"m1"}]})).unwrap();
        assert_eq!(proxy, vec!["m1", "m2"]);
    }

    #[tokio::test]
    async fn fetches_real_http_model_list_with_bearer_key() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut input = [0u8; 4096];
            let read = stream.read(&mut input).await.unwrap();
            let request = String::from_utf8_lossy(&input[..read]);
            assert!(request.starts_with("GET /v1/models HTTP/1.1"));
            assert!(request.to_ascii_lowercase().contains("authorization: bearer test-key"));
            let body = r#"{"data":[{"id":"model-one"},{"id":"model-two"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let models = fetch_models(&format!("http://{address}/v1/chat/completions"), Some("test-key"))
            .await.unwrap();
        assert_eq!(models, vec!["model-one", "model-two"]);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn never_follows_a_models_api_redirect_with_credentials() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let _ = stream.read(&mut buf).await.unwrap();
            stream.write_all(
                b"HTTP/1.1 302 Found\r\nlocation: https://other.example.com/models\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            ).await.unwrap();
        });
        assert!(fetch_models(&format!("http://{address}/v1/chat/completions"), Some("sensitive-key"))
            .await.unwrap_err().to_string().contains("redirect"));
        task.await.unwrap();
    }
}
