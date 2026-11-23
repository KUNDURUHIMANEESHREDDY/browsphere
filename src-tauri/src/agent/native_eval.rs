//! Execute JavaScript and return its value through the host WebView2 COM API.
//! External webpages receive no Tauri command privileges. Results never rely
//! on JavaScript IPC installed in untrusted website origins.
use tauri::Manager;

#[cfg(windows)]
pub async fn eval_json(
    app: &tauri::AppHandle,
    webview_label: &str,
    script: String,
    timeout_ms: u64,
) -> Result<serde_json::Value, String> {
    use std::sync::{Arc, Mutex};
    use tokio::sync::oneshot;
    use webview2_com::{ExecuteScriptCompletedHandler, CoTaskMemPWSTR};

    let webview = app.get_webview(webview_label)
        .ok_or_else(|| "No active browser tab exists".to_string())?;
    let (tx, rx) = oneshot::channel::<Result<String, String>>();
    let output = Arc::new(Mutex::new(Some(tx)));
    let output_for_dispatch = output.clone();
    webview.with_webview(move |native| {
        let finish = |value: Result<String, String>| {
            if let Some(tx) = output_for_dispatch.lock().unwrap().take() {
                let _ = tx.send(value);
            }
        };
        let core = match unsafe { native.controller().CoreWebView2() } {
            Ok(webview) => webview,
            Err(err) => {
                finish(Err(format!("Cannot obtain native WebView2: {err}")));
                return;
            }
        };
        let output_for_callback = output_for_dispatch.clone();
        let handler = ExecuteScriptCompletedHandler::create(Box::new(move |status, result| {
            if let Some(tx) = output_for_callback.lock().unwrap().take() {
                let _ = tx.send(status.map(|_| result).map_err(|e| e.to_string()));
            }
            Ok(())
        }));
        let wide = CoTaskMemPWSTR::from(script.as_str());
        if let Err(err) = unsafe {
            core.ExecuteScript(*wide.as_ref().as_pcwstr(), &handler)
        } {
            finish(Err(format!("WebView2 ExecuteScript failed: {err}")));
        }
    }).map_err(|e| format!("Unable to schedule WebView2 script: {e}"))?;

    let raw = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms), rx
    ).await.map_err(|_| "WebView2 script result timed out".to_string())?
        .map_err(|_| "WebView2 result channel closed".to_string())??;
    serde_json::from_str(&raw)
        .map_err(|e| format!("WebView2 returned invalid script result: {e}"))
}

#[cfg(not(windows))]
pub async fn eval_json(
    _app: &tauri::AppHandle,
    _webview_label: &str,
    _script: String,
    _timeout_ms: u64,
) -> Result<serde_json::Value, String> {
    Err("Native WebView2 scripting is currently supported on Windows only".into())
}
