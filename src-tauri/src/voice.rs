use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::State;
use std::sync::Arc;

use crate::AppState;

const ELEVEN_KEY_ACCOUNT: &str = "voice:elevenlabs";
const KOKORO_ENDPOINT: &str = "http://127.0.0.1:8880/v1/audio/speech";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceConfig {
    pub provider: String,
    pub voice_id: String,
    pub model_id: String,
    pub allow_cloud_tts: bool,
    pub speak_replies: bool,
}
impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            provider: "elevenlabs".into(),
            voice_id: "JBFqnCBsd6RMkjVDRZzb".into(),
            model_id: "eleven_flash_v2_5".into(),
            allow_cloud_tts: false,
            speak_replies: true,
        }
    }
}
impl VoiceConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(matches!(self.provider.as_str(), "elevenlabs" | "kokoro"), "Invalid voice provider");
        anyhow::ensure!((12..=64).contains(&self.voice_id.len())
            && self.voice_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "Invalid voice ID");
        anyhow::ensure!(matches!(self.model_id.as_str(), "eleven_flash_v2_5" | "eleven_multilingual_v2"), "Unsupported ElevenLabs model");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceAudio {
    pub audio_base64: String,
    pub mime_type: String,
    pub provider: String,
}

#[tauri::command]
pub async fn get_voice_config(state: State<'_, Arc<AppState>>) -> Result<VoiceConfig, String> {
    let db = state.db.lock().await;
    Ok(db.load_voice_config().map_err(|e| e.to_string())?.unwrap_or_default())
}

#[tauri::command]
pub async fn update_voice_config(state: State<'_, Arc<AppState>>, config: VoiceConfig) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    let mut db = state.db.lock().await;
    db.save_voice_config(&config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn has_voice_api_key(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    state.creds.has_secret(ELEVEN_KEY_ACCOUNT).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_voice_api_key(state: State<'_, Arc<AppState>>, api_key: String) -> Result<(), String> {
    let key = api_key.trim();
    if !(16..=512).contains(&key.len()) || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("Invalid API key".into());
    }
    state.creds.set_secret(ELEVEN_KEY_ACCOUNT, key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_voice_api_key(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.creds.delete_secret(ELEVEN_KEY_ACCOUNT).map_err(|e| e.to_string())
}

async fn elevenlabs_speech(text: &str, voice: &VoiceConfig, api_key: &str, client: &reqwest::Client) -> anyhow::Result<Vec<u8>> {
    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}/stream", voice.voice_id);
    let res = client.post(url)
        .header("xi-api-key", api_key)
        .json(&serde_json::json!({
            "text": text,
            "model_id": voice.model_id,
            "output_format": "mp3_44100_128"
        }))
        .send().await?.error_for_status()?;
    let bytes = res.bytes().await?;
    anyhow::ensure!(!bytes.is_empty() && bytes.len() <= 5_000_000, "Voice output size is invalid");
    Ok(bytes.to_vec())
}

async fn kokoro_speech(text: &str, client: &reqwest::Client) -> anyhow::Result<Vec<u8>> {
    // Fixed loopback endpoint only. Never upload local speech to an arbitrary host.
    let res = client.post(KOKORO_ENDPOINT)
        .json(&serde_json::json!({
            "model":"kokoro",
            "voice":"af_heart",
            "input":text,
            "response_format":"mp3"
        })).send().await?.error_for_status()?;
    let bytes = res.bytes().await?;
    anyhow::ensure!(!bytes.is_empty() && bytes.len() <= 5_000_000, "Local voice output size is invalid");
    Ok(bytes.to_vec())
}

#[tauri::command]
pub async fn synthesize_voice(
    state: State<'_, Arc<AppState>>,
    text: String,
    is_private: Option<bool>,
) -> Result<VoiceAudio, String> {
    let clean = text.trim();
    let started = std::time::Instant::now();
    if clean.is_empty() || clean.len() > 1600 || clean.chars().any(|c| c == '\0') {
        return Err("Speech input must be 1–1600 characters".into());
    }
    let config = {
        let db = state.db.lock().await;
        db.load_voice_config().map_err(|e| e.to_string())?.unwrap_or_default()
    };
    config.validate().map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(18))
        .build().map_err(|e| e.to_string())?;
    let private = is_private.unwrap_or(false);
    if config.provider == "elevenlabs" && config.allow_cloud_tts && !private {
        if let Ok(Some(api_key)) = state.creds.get_secret(ELEVEN_KEY_ACCOUNT) {
            if !api_key.is_empty() {
                if let Ok(bytes) = elevenlabs_speech(clean, &config, &api_key, &client).await {
                    crate::diagnostics::record("voice", "synthesis_finished",
                        serde_json::json!({"kind": "elevenlabs",
                            "status": "ok",
                            "duration_ms": started.elapsed().as_millis() as u64}));
                    return Ok(VoiceAudio {
                        audio_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                        mime_type: "audio/mpeg".into(), provider: "elevenlabs".into()
                    });
                }
            }
        }
    }
    match kokoro_speech(clean, &client).await {
        Ok(bytes) => {
            if !private {
                crate::diagnostics::record("voice", "synthesis_finished",
                    serde_json::json!({"kind": "kokoro", "status": "ok",
                        "duration_ms": started.elapsed().as_millis() as u64}));
            }
            Ok(VoiceAudio {
                audio_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                mime_type: "audio/mpeg".into(), provider: "kokoro".into()
            })
        },
        Err(_) => {
            if !private {
                crate::diagnostics::record("errors", "voice_failure",
                    serde_json::json!({"error_kind": "voice_synthesis"}));
            }
            Err("ElevenLabs unavailable or not enabled; local Kokoro service is not running. Use Windows speech fallback.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_untrusted_voice_ids_and_providers() {
        assert!(VoiceConfig::default().validate().is_ok());
        let mut bad=VoiceConfig::default();bad.voice_id="../secret".into();
        assert!(bad.validate().is_err());
        bad=VoiceConfig::default();bad.provider="https://example.org".into();
        assert!(bad.validate().is_err());
        bad=VoiceConfig::default();bad.model_id="some-unexpected-model".into();
        assert!(bad.validate().is_err());
    }
    #[test]
    fn cloud_voice_is_opt_in() {
        let d=VoiceConfig::default();
        assert!(!d.allow_cloud_tts);
        assert!(d.speak_replies);
        assert_eq!(KOKORO_ENDPOINT,"http://127.0.0.1:8880/v1/audio/speech");
    }
}
