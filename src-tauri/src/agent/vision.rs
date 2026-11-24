//! Optional screenshot perception. Images stay on the loopback Ollama API.
//! Vision output is untrusted context, never a direct browser action.
use crate::router::model_router::RouterConfig;

use super::multimodal_perception::{self as mm,Candidate,VisualObservation};
/// Read only one screenshot from the dedicated tab. The caller must
/// independently re-observe and validate the page before consuming output.
pub async fn observe(config:&RouterConfig,app:&tauri::AppHandle,label:&str,goal:&str,candidates:&[Candidate])->Result<VisualObservation,String>{
    let image=capture(app,label).await?;
    infer(config,&image,goal,candidates).await
}
pub async fn infer(config:&RouterConfig,image:&str,goal:&str,candidates:&[Candidate])->Result<VisualObservation,String>{
    let endpoint=url::Url::parse(&config.ollama_url).map_err(|_|"Invalid Ollama URL")?;
    if endpoint.scheme()!="http"||!matches!(endpoint.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"))||
        !endpoint.username().is_empty()||endpoint.password().is_some()||endpoint.query().is_some()||endpoint.fragment().is_some(){
        return Err("Vision requires a loopback Ollama endpoint".into());
    }
    if config.vision_model.trim().is_empty()||config.lucia_cloud_only||config.vision_model.to_ascii_lowercase().contains("cloud"){
        return Err("Local vision disabled".into());
    }
    if image.is_empty()||image.len()>12_000_000{return Err("Screenshot outside size limit".into());}
    let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(35)).build().map_err(|_|"Visual inference client unavailable")?;
    let response:serde_json::Value=client.post(format!("{}/api/chat",config.ollama_url.trim_end_matches('/')))
        .json(&serde_json::json!({
            "model":config.vision_model,"stream":false,"think":false,"keep_alive":0,
            "format":mm::schema(candidates.len()),
            "messages":[{"role":"user","content":mm::prompt(goal,candidates),"images":[image]}],
            "options":{"num_ctx":4096,"num_predict":260,"temperature":0}
        })).send().await.map_err(|_|"Local visual inference request failed")?
        .error_for_status().map_err(|_|"Local visual inference rejected")?.json().await.map_err(|_|"Invalid visual provider response")?;
    let answer=response["message"]["content"].as_str().ok_or("No structured visual observation")?;
    mm::parse(answer,candidates.len())
}

#[cfg(windows)]
async fn capture(app: &tauri::AppHandle, label: &str) -> Result<String, String> {
    use tauri::Manager;
    use std::sync::{Arc, Mutex};
    use tokio::sync::oneshot;
    use webview2_com::{CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR};
    let view = app.get_webview(label).ok_or("Task tab unavailable")?;
    let (tx, rx) = oneshot::channel::<Result<String, String>>();
    let output = Arc::new(Mutex::new(Some(tx)));
    view.with_webview(move |native| {
        let finish = |result: Result<String, String>| {
            if let Some(tx) = output.lock().unwrap().take() { let _ = tx.send(result); }
        };
        let core = match unsafe { native.controller().CoreWebView2() } {
            Ok(core) => core, Err(e) => { finish(Err(e.to_string())); return; }
        };
        let callback_output = output.clone();
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status, result| {
            if let Some(tx) = callback_output.lock().unwrap().take() {
                let _ = tx.send(status.map(|_| result).map_err(|e| e.to_string()));
            }
            Ok(())
        }));
        let method = CoTaskMemPWSTR::from("Page.captureScreenshot");
        let params = CoTaskMemPWSTR::from(r#"{"format":"jpeg","quality":65,"captureBeyondViewport":false}"#);
        if let Err(e) = unsafe { core.CallDevToolsProtocolMethod(*method.as_ref().as_pcwstr(), *params.as_ref().as_pcwstr(), &handler) } {
            finish(Err(e.to_string()));
        }
    }).map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(4), rx).await
        .map_err(|_| "Screenshot timed out")?.map_err(|_| "Screenshot callback closed")??;
    let value: serde_json::Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
    value["data"].as_str().filter(|s| !s.is_empty()).map(str::to_owned).ok_or_else(|| "Screenshot returned no image".into())
}

#[cfg(not(windows))]
async fn capture(_: &tauri::AppHandle, _: &str) -> Result<String, String> {
    Err("Screenshot perception requires Windows WebView2".into())
}
