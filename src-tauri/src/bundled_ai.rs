//! Private bundled inference service. No system Ollama installation is used.
use anyhow::{anyhow, Context, Result};
use std::{
    net::{Ipv4Addr, TcpListener},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

pub const FAST: &str = "qwen3.5:0.8b";
pub const PLAN: &str = "lfm3:1.7b";
pub const VISION: &str = "lfm3-vl:2b";

// Experimental smaller model profile. The original release remains supported
// until runtime, licensing and repeated task acceptance gates pass.
pub const COMPACT_REASON: &str = "gemma3:1b";
pub const COMPACT_DECISION: &str = "tev1:0.8b";
pub const COMPACT_VISION: &str = "smolvlm2:500m";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundledProfile {
    Legacy,
    Compact,
}

fn profile_from_tags(body: &serde_json::Value) -> Option<BundledProfile> {
    let models = body.get("models")?.as_array()?;
    let has = |tags: &[&str]| {
        tags.iter()
            .all(|tag| models.iter().any(|m| m["name"].as_str() == Some(*tag)))
    };
    if has(&[COMPACT_REASON, COMPACT_DECISION, COMPACT_VISION]) {
        Some(BundledProfile::Compact)
    } else if has(&[FAST, PLAN, VISION]) {
        Some(BundledProfile::Legacy)
    } else {
        None
    }
}

pub struct BundledAi {
    child: Child,
    pub endpoint: String,
    pub profile: BundledProfile,
}

impl BundledAi {
    pub async fn launch(root: &Path) -> Result<Option<Self>> {
        let dir = root.join("bundled-ai");
        if !dir.exists() {
            return Ok(None);
        }
        let exe = dir.join("runtime").join("ollama.exe");
        let models = dir.join("models");
        anyhow::ensure!(
            exe.is_file() && models.is_dir(),
            "Offline AI resources are incomplete"
        );
        let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = reservation.local_addr()?;
        let mut cmd = Command::new(&exe);
        cmd.arg("serve")
            .current_dir(exe.parent().unwrap())
            .env("OLLAMA_HOST", addr.to_string())
            .env("OLLAMA_MODELS", &models)
            .env("OLLAMA_NO_CLOUD", "1")
            .env("OLLAMA_NOPRUNE", "true")
            .env("OLLAMA_MAX_LOADED_MODELS", "1")
            .env("OLLAMA_NUM_PARALLEL", "1")
            .env("OLLAMA_KEEP_ALIVE", "30s")
            .env("OLLAMA_ORIGINS", "http://127.0.0.1:1")
            .env("OLLAMA_LLM_LIBRARY", "cpu")
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        drop(reservation);
        let child = cmd.spawn().context("Start bundled inference service")?;
        let mut service = Self {
            child,
            endpoint: format!("http://{addr}"),
            profile: BundledProfile::Legacy,
        };
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(1))
            .build()?;
        for _ in 0..40 {
            if service.child.try_wait()?.is_some() {
                return Err(anyhow!("Bundled inference service exited"));
            }
            if let Ok(response) = client
                .get(format!("{}/api/tags", service.endpoint))
                .send()
                .await
            {
                if let Ok(response) = response.error_for_status() {
                    if let Ok(body) = response.json::<serde_json::Value>().await {
                        if let Some(profile) = profile_from_tags(&body) {
                            service.profile = profile;
                            return Ok(Some(service));
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Err(anyhow!(
            "Bundled inference service did not report a complete compact or legacy model profile"
        ))
    }
}

impl Drop for BundledAi {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = Command::new("taskkill.exe")
                    .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                    .creation_flags(0x08000000)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod compact_profile_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn compact_profile_only_when_all_three_models_present() {
        let all = json!({"models":[
            {"name":COMPACT_REASON},{"name":COMPACT_DECISION},{"name":COMPACT_VISION}
        ]});
        assert_eq!(profile_from_tags(&all), Some(BundledProfile::Compact));
        assert_eq!(
            profile_from_tags(&json!({"models":[
                {"name":COMPACT_REASON},{"name":COMPACT_VISION}
            ]})),
            None
        );
        assert_eq!(profile_from_tags(&json!({"models":[]})), None);
    }
    #[test]
    fn legacy_installer_remains_usable() {
        let legacy = json!({"models":[{"name":FAST},{"name":PLAN},{"name":VISION}]});
        assert_eq!(profile_from_tags(&legacy), Some(BundledProfile::Legacy));
        assert_eq!(profile_from_tags(&json!({"models":null})), None);
    }
}
