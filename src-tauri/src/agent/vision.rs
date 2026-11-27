//! Opt-in live WebView2 visual perception. Sample a bounded native CDP video
//! stream and infer locally; no one-off screenshot is used by Lucia's visual agent.
//! Vision output is untrusted context, never a direct browser action.
use crate::router::model_router::RouterConfig;

use super::multimodal_perception::{self as mm,Candidate,VisualObservation};
#[derive(Debug,Default,serde::Serialize)]
pub struct InferenceTrace{pub attempts:u8,pub first_failure:Option<&'static str>,pub recovery:bool,pub capture_ms:u64,pub inference_ms:u64,pub provider_load_ms:u64,pub provider_prompt_ms:u64,pub provider_decode_ms:u64}

/// Read one recent frame from the live CDP stream on the dedicated tab.
/// The caller must
/// independently re-observe and validate the page before consuming output.
pub async fn observe(config:&RouterConfig,app:&tauri::AppHandle,label:&str,goal:&str,candidates:&[Candidate])->Result<(VisualObservation,mm::Viewport,InferenceTrace),String>{
    let started=tokio::time::Instant::now();
    let before=viewport(app,label).await?;
    let image=super::live_visual::sample(app,label).await?;
    let after=viewport(app,label).await?;
    if before!=after{return Err("Viewport changed during capture".into());}
    let capture_ms=started.elapsed().as_millis() as u64;
    let (observation,mut trace)=infer(config,&image,goal,candidates).await?;
    trace.capture_ms=capture_ms;
    Ok((observation,before,trace))
}
/// Ephemeral comparison for a same-document viewport. The hash is never
/// serialized or persisted. Frame inequality is only a diagnostic signal,
/// not proof of semantic progress or an action receipt.
pub(super) struct FrameSignature{viewport:mm::Viewport,digest:[u8;32]}
pub(super) async fn frame_signature(app:&tauri::AppHandle,label:&str,expected_url:&str)->Option<FrameSignature>{
    use sha2::{Digest,Sha256};
    let before=viewport(app,label).await.ok()?;
    if before.origin!=expected_url{return None;}
    let image=super::live_visual::sample(app,label).await.ok()?;
    if image.is_empty()||image.len()>12_000_000{return None;}
    if viewport(app,label).await.ok()?!=before{return None;}
    Some(FrameSignature{viewport:before,digest:Sha256::digest(image.as_bytes()).into()})
}
pub(super) fn frame_changed(before:&FrameSignature,after:&FrameSignature)->Option<bool>{
    (before.viewport==after.viewport).then_some(before.digest!=after.digest)
}
async fn viewport(app:&tauri::AppHandle,label:&str)->Result<mm::Viewport,String>{
    let raw=crate::agent::native_eval::eval_json(app,label,
        "({width:innerWidth,height:innerHeight,scroll_x:scrollX,scroll_y:scrollY,scale:devicePixelRatio,origin:location.href})".into(),1800).await?;
    let v:mm::Viewport=serde_json::from_value(raw).map_err(|_|"Invalid screenshot viewport")?;
    if !v.valid(){return Err("Invalid screenshot viewport".into());}
    Ok(v)
}
pub async fn infer(config:&RouterConfig,image:&str,goal:&str,candidates:&[Candidate])->Result<(VisualObservation,InferenceTrace),String>{
    let endpoint=url::Url::parse(&config.ollama_url).map_err(|_|"Invalid Ollama URL")?;
    if endpoint.scheme()!="http"||!matches!(endpoint.host_str(),Some("localhost"|"127.0.0.1"|"[::1]"))||
        !endpoint.username().is_empty()||endpoint.password().is_some()||endpoint.query().is_some()||endpoint.fragment().is_some(){
        return Err("Vision requires a loopback Ollama endpoint".into());
    }
    if config.vision_model.trim().is_empty()||config.lucia_cloud_only||config.vision_model.to_ascii_lowercase().contains("cloud"){
        return Err("Local vision disabled".into());
    }
    if image.is_empty()||image.len()>12_000_000{return Err("Screenshot outside size limit".into());}
    let lease=crate::router::inference_scheduler::acquire().await.map_err(|_|"Local inference resource queue unavailable")?;
    let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(35)).build().map_err(|_|"Visual inference client unavailable")?;
    let started=tokio::time::Instant::now();
    let deadline=started+std::time::Duration::from_secs(35);
    let prompt=mm::prompt(goal,candidates);
    let mut trace=InferenceTrace::default();
    for attempt in 0..2{
        let remaining=deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero(){return Err("Local visual inference timed out".into());}
        trace.attempts+=1;
        let content=if attempt==0{prompt.clone()}else{mm::recovery_prompt(goal,candidates)};
        let schema=if attempt==0{mm::schema(candidates.len())}else{mm::recovery_schema(candidates.len())};
        let response:serde_json::Value=client.post(format!("{}/api/chat",config.ollama_url.trim_end_matches('/')))
            .timeout(remaining)
            .json(&serde_json::json!({
                "model":config.vision_model,"stream":false,"think":false,"keep_alive":lease.keep_alive(),
                "format":schema,
                "messages":[{"role":"user","content":content,"images":[image]}],
                "options":{"num_ctx":4096,"num_predict":512,"temperature":0}
            })).send().await.map_err(|e|if e.is_timeout(){"Local visual inference timed out"}else if e.is_connect(){"Local vision runtime unreachable"}else{"Local visual inference request failed"})?
            .error_for_status().map_err(|_|"Local visual inference rejected")?.json().await.map_err(|_|"Invalid visual provider response")?;
        trace.provider_load_ms=trace.provider_load_ms.saturating_add(response["load_duration"].as_u64().unwrap_or(0)/1_000_000);
        trace.provider_prompt_ms=trace.provider_prompt_ms.saturating_add(response["prompt_eval_duration"].as_u64().unwrap_or(0)/1_000_000);
        trace.provider_decode_ms=trace.provider_decode_ms.saturating_add(response["eval_duration"].as_u64().unwrap_or(0)/1_000_000);
        let parsed=provider_observation(&response,candidates.len()).and_then(|value|{
            if attempt>0&&(value.controls.len()>1||!value.visible_text.is_empty()||value.controls.iter().any(|c|c.point.is_none())){return Err("Recovery observation violates compact schema".into());}
            Ok(value)
        });
        match parsed{
            Ok(value)=>{trace.recovery=attempt>0;trace.inference_ms=started.elapsed().as_millis() as u64;return Ok((value,trace));},
            Err(error) if attempt==0&&retryable_observation(&error)&&deadline.saturating_duration_since(tokio::time::Instant::now())>=std::time::Duration::from_secs(3)=>{
                trace.first_failure=Some(failure_detail(&error));continue;
            },
            Err(error)=>return Err(error),
        }
    }
    Err("Invalid structured visual observation".into())

}

fn provider_observation(response:&serde_json::Value,count:usize)->Result<VisualObservation,String>{
    if response["done_reason"].as_str()==Some("length"){return Err("Visual output truncated".into());}
    let answer=response["message"]["content"].as_str().filter(|s|!s.trim().is_empty()).ok_or("No structured visual observation")?;
    // A single exact Markdown fence is transport formatting, not a schema
    // change. Never extract a JSON substring from arbitrary model prose.
    let raw=answer.trim();
    let raw=raw.strip_prefix("```json\n").and_then(|s|s.strip_suffix("\n```"))
        .or_else(||raw.strip_prefix("```json\r\n").and_then(|s|s.strip_suffix("\r\n```"))).unwrap_or(raw);
    mm::parse(raw,count)
}
fn retryable_observation(error:&str)->bool{
    matches!(failure_category(error),"invalid_observation"|"output_truncated")
}

async fn capture(app: &tauri::AppHandle, label: &str) -> Result<String, String> {
    capture_with_params(app,label,r#"{"format":"jpeg","quality":65,"captureBeyondViewport":false}"#.into()).await
}
#[cfg(windows)]
pub(super) async fn capture_with_params(app: &tauri::AppHandle, label: &str, capture_params:String) -> Result<String, String> {
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
        let params = CoTaskMemPWSTR::from(capture_params.as_str());
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
pub(super) async fn capture_with_params(_: &tauri::AppHandle, _: &str, _:String) -> Result<String, String> {
    Err("Screenshot perception requires Windows WebView2".into())
}


// Emit only known error categories; never provider response bodies,
// untrusted frame content, arbitrary exception strings or private URLs.
pub fn failure_category(error:&str)->&'static str{
    match error{
        "Local visual inference timed out"|"Catalog visual recovery time limit"=>"inference_timeout",
        "Local vision runtime unreachable"=>"runtime_unreachable",
        "Local visual inference rejected"=>"provider_rejected",
        "Local visual inference request failed"=>"inference_transport",
        "Invalid visual provider response"=>"provider_response",
        "Visual output truncated"=>"output_truncated",
        "Invalid structured visual observation"|"Visual observation violates bounded schema"|"Duplicate visual control index"|"Visual output exceeds size limit"|"No structured visual observation"|"Visual JSON syntax invalid"|"Visual JSON field type or enum invalid"|"Too many visual controls"|"Visual text outside bounds"|"Visual control index outside bounds"|"Visual control label outside bounds"|"Visual coordinates outside bounds"|"Independent visual control missing point"|"Recovery observation violates compact schema"=>"invalid_observation",
        "Viewport changed during capture"|"Invalid screenshot viewport"=>"viewport_changed",
        "Screenshot timed out"|"Screenshot callback closed"|"Screenshot returned no image"|"Screenshot outside size limit"|"Live visual frame stream timed out"|"Visual stream command timed out"|"Visual stream command cancelled"|"Live vision already active on this tab"|"Live vision session capacity reached"=>"capture_failed",
        "Local vision disabled"|"Vision requires a loopback Ollama endpoint"|"Invalid Ollama URL"=>"configuration",
        "Local inference resource queue unavailable"=>"resource_unavailable",
        _=>"native_observation_failed"
    }
}
pub fn failure_detail(error:&str)->&'static str{
    match error{
        "Visual JSON syntax invalid"=>"json_syntax",
        "Visual JSON field type or enum invalid"=>"field_type_or_enum",
        "Too many visual controls"=>"control_count",
        "Visual text outside bounds"=>"text_bounds",
        "Visual control index outside bounds"=>"index_bounds",
        "Visual control label outside bounds"=>"label_bounds",
        "Visual coordinates outside bounds"=>"coordinate_bounds",
        "Independent visual control missing point"=>"missing_point",
        "Duplicate visual control index"=>"duplicate_index",
        "Recovery observation violates compact schema"=>"compact_schema",
        _=>failure_category(error)
    }
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn frame_comparison_requires_identical_viewport_and_origin(){
    let v=mm::Viewport{width:1000.,height:500.,scroll_x:0.,scroll_y:0.,scale:1.,origin:"https://example.org/".into()};
    let a=FrameSignature{viewport:v.clone(),digest:[1;32]};
    let mut b=FrameSignature{viewport:v,digest:[1;32]};
    assert_eq!(frame_changed(&a,&b),Some(false));
    b.digest=[2;32];assert_eq!(frame_changed(&a,&b),Some(true));
    b.viewport.scroll_y=100.;assert_eq!(frame_changed(&a,&b),None);
    b.viewport=a.viewport.clone();b.viewport.origin="https://example.org/other".into();
    assert_eq!(frame_changed(&a,&b),None);
 }
 #[test]fn only_strict_observation_failures_are_retried(){
  let bad=serde_json::json!({"message":{"content":"not JSON"}});
  assert!(retryable_observation(&provider_observation(&bad,0).unwrap_err()));
  assert!(!retryable_observation("Local vision runtime unreachable"));
  assert!(!retryable_observation("Local visual inference rejected"));
  let valid=serde_json::json!({"page_kind":"documentation","layout":"document","blocker":"none","controls":[],"visible_text":[]}).to_string();
  let good=serde_json::json!({"message":{"content":format!("```json\n{valid}\n```")}});
  assert!(provider_observation(&good,0).is_ok());
  let windows=serde_json::json!({"message":{"content":format!("```json\r\n{valid}\r\n```")}});
  assert!(provider_observation(&windows,0).is_ok());
  let prose=serde_json::json!({"message":{"content":format!("Here:\r\n```json\r\n{valid}\r\n```")}});
  assert!(provider_observation(&prose,0).is_err());
  let action=serde_json::json!({"message":{"content":valid.replace("\"controls\"","\"action\":\"click\",\"controls\"")}});
  assert!(provider_observation(&action,0).is_err());
 }
 #[tokio::test]
 async fn malformed_vision_response_recovers_with_compact_schema_and_exactly_one_retry(){
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr=listener.local_addr().unwrap();
    let server=tokio::spawn(async move{
        for i in 0..2{
            let(mut socket,_)=listener.accept().await.unwrap();
            let mut bytes=Vec::new();let mut buf=[0u8;4096];
            let end=loop{let n=socket.read(&mut buf).await.unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);if let Some(i)=bytes.windows(4).position(|x|x==b"\r\n\r\n"){break i+4;}};
            let headers=String::from_utf8_lossy(&bytes[..end]);
            let len=headers.lines().find_map(|l|l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v|v.trim().parse::<usize>().ok())).unwrap();
            while bytes.len()<end+len{let n=socket.read(&mut buf).await.unwrap();assert!(n>0);bytes.extend_from_slice(&buf[..n]);}
            let r:serde_json::Value=serde_json::from_slice(&bytes[end..end+len]).unwrap();
            assert_eq!(r["format"]["properties"]["controls"]["maxItems"],if i==0{3}else{1});
            let content=if i==0{serde_json::json!({"page_kind":"documentation","layout":"document","blocker":"none","controls":[],"visible_text":["x"]})}
                else{serde_json::json!({"page_kind":"documentation","layout":"document","blocker":"none","controls":[{"index":0,"label":"Field Guide","point":{"x":240,"y":130}}],"visible_text":[]})};
            let response=serde_json::json!({"message":{"content":content.to_string()}}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).as_bytes()).await.unwrap();
        }
    });
    let mut cfg=RouterConfig::default();cfg.ollama_url=format!("http://{addr}");cfg.vision_model="fixture-vision".into();cfg.lucia_cloud_only=false;
    let (o,trace)=infer(&cfg,"fixture-image","Find the Field Guide",&[]).await.unwrap();
    assert_eq!(trace.attempts,2);assert!(trace.recovery);assert_eq!(trace.first_failure,Some("text_bounds"));assert_eq!(o.controls[0].label,"Field Guide");
    server.await.unwrap();
 }
 #[test]fn vision_failure_diagnostics_do_not_expose_untrusted_errors(){
  assert_eq!(failure_category("Local vision runtime unreachable"),"runtime_unreachable");
  assert_eq!(failure_category("Local inference resource queue unavailable"),"resource_unavailable");
  assert!(!retryable_observation("Local inference resource queue unavailable"));
  assert_eq!(failure_category("Visual output truncated"),"output_truncated");
  assert_eq!(failure_category("Invalid structured visual observation"),"invalid_observation");
  assert_eq!(failure_category("unexpected provider details"),"native_observation_failed");
 }
}
