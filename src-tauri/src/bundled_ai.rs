//! Private bundled inference service. No system Ollama installation is used.
use anyhow::{anyhow, Context, Result};
use std::{net::{Ipv4Addr, TcpListener}, path::Path, process::{Child, Command, Stdio}, time::Duration};

pub const FAST: &str = "qwen3.5:0.8b";
pub const PLAN: &str = "lfm3:1.7b";
pub const VISION: &str = "lfm3-vl:2b";

pub struct BundledAi {
    child: Child,
    pub endpoint: String,
}

impl BundledAi {
    pub async fn launch(root: &Path) -> Result<Option<Self>> {
        let dir = root.join("bundled-ai");
        if !dir.exists() { return Ok(None); }
        let exe = dir.join("runtime").join("ollama.exe");
        let models = dir.join("models");
        anyhow::ensure!(exe.is_file() && models.is_dir(), "Offline AI resources are incomplete");
        let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = reservation.local_addr()?;
        let mut cmd = Command::new(&exe);
        cmd.arg("serve").current_dir(exe.parent().unwrap())
            .env("OLLAMA_HOST", addr.to_string())
            .env("OLLAMA_MODELS", &models)
            .env("OLLAMA_NO_CLOUD", "1")
            .env("OLLAMA_NOPRUNE", "true")
            .env("OLLAMA_MAX_LOADED_MODELS", "1")
            .env("OLLAMA_NUM_PARALLEL", "1")
            .env("OLLAMA_KEEP_ALIVE", "30s")
            .env("OLLAMA_ORIGINS", "http://127.0.0.1:1")
            .env("OLLAMA_LLM_LIBRARY", "cpu")
            .env_remove("HTTP_PROXY").env_remove("HTTPS_PROXY").env_remove("ALL_PROXY")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        drop(reservation);
        let child = cmd.spawn().context("Start bundled inference service")?;
        let mut service = Self { child, endpoint: format!("http://{addr}") };
        let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(1)).build()?;
        for _ in 0..40 {
            if service.child.try_wait()?.is_some() { return Err(anyhow!("Bundled inference service exited")); }
            if let Ok(response) = client.get(format!("{}/api/tags", service.endpoint)).send().await {
                if let Ok(response) = response.error_for_status() {
                    if let Ok(body) = response.json::<serde_json::Value>().await {
                        if [FAST, PLAN, VISION].iter().all(|name| body["models"].as_array()
                            .is_some_and(|models| models.iter().any(|m| m["name"].as_str() == Some(*name)))) {
                            return Ok(Some(service));
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Err(anyhow!("Bundled inference service did not report all three models"))
    }
}

impl Drop for BundledAi {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = Command::new("taskkill.exe").args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                    .creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
            }
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
