//! Official rmcp SDK interoperability probe. Does not execute tools, access
//! credentials, or replace Lucia's production transport before verification.
use anyhow::{bail, ensure, Result};
use rmcp::{
    ClientLifecycleMode, ClientServiceExt,
    model::{ClientCapabilities, ClientConfig, Implementation, ProtocolVersion},
    transport::StreamableHttpClientTransport,
};
use std::time::Duration;
use url::Url;

fn validate_endpoint(raw: &str) -> Result<Url> {
    let url = Url::parse(raw)?;
    ensure!(
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some()
            && url.path() == "/mcp"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Only an explicit local loopback MCP test endpoint is permitted"
    );
    Ok(url)
}

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = std::env::args().nth(1).ok_or_else(|| {
        anyhow::anyhow!("Usage: browsphere-rmcp-probe http://127.0.0.1:PORT/mcp")
    })?;
    let endpoint = validate_endpoint(&endpoint)?;
    let work = async {
        let config = ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("browsphere-rmcp-interop-probe", "0.1.0"),
        );
        let transport = StreamableHttpClientTransport::from_uri(endpoint.to_string());
        let client = config
            .serve_with_lifecycle(
                transport,
                ClientLifecycleMode::Auto {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                    legacy_version: Some(ProtocolVersion::V_2025_11_25),
                },
            )
            .await?;
        let result = client.list_tools(None).await;
        let close = client.cancel().await;
        let tools = result?;
        close?;
        if tools.tools.is_empty() { bail!("MCP server returned no tools"); }
        for tool in &tools.tools {
            // Tool metadata is untrusted. Do not grant permissions or call anything.
            println!("{}", serde_json::json!({"name":tool.name, "discovered_only":true}));
        }
        Ok::<(), anyhow::Error>(())
    };
    tokio::time::timeout(Duration::from_secs(25), work).await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loopback_is_mandatory() {
        assert!(validate_endpoint("http://127.0.0.1:18888/mcp").is_ok());
        for raw in [
            "https://example.com/mcp",
            "http://localhost:18888/mcp",
            "http://127.0.0.1:18888/other",
            "http://127.0.0.1:18888/mcp?token=secret",
            "http://user:pass@127.0.0.1:18888/mcp",
            "http://[::1]:18888/mcp",
        ] {
            assert!(validate_endpoint(raw).is_err(), "unexpectedly accepted {raw}");
        }
    }
}
