//! Live WebView2 video-frame observation for Lucia's local visual model.
//! Uses CDP Page.startScreencast + Page.screencastFrame events instead of
//! repeatedly requesting Page.captureScreenshot. Frames never leave the
//! local model path or appear in logs, task history or persisted caches.
//!
//! This is a bounded, short-lived frame stream used on already-authorized,
//! public pages. It does NOT grant the model new browser actions.
use std::{collections::HashMap,sync::{Mutex,OnceLock},time::{Duration,Instant}};
use tauri::Manager;

const MAX_FRAME_CHARS:usize=3_000_000; // max ~2.25 MB JPEG (base64)
const MAX_TABS:usize=8;
const FRAME_MAX_AGE:Duration=Duration::from_millis(1250);
const STREAM_WAIT:Duration=Duration::from_secs(4);

#[derive(Default)]
struct Slot {
    epoch:u64,
    active:bool,
    frame:Option<(Instant,String)>,
}
fn frames()->&'static Mutex<HashMap<String,Slot>>{
    static FRAMES:OnceLock<Mutex<HashMap<String,Slot>>>=OnceLock::new();
    FRAMES.get_or_init(||Mutex::new(HashMap::new()))
}
fn begin(label:&str)->Result<u64,String>{
    let mut map=frames().lock().unwrap();
    if !map.contains_key(label)&&map.len()>=MAX_TABS {
        return Err("Live vision session capacity reached".into());
    }
    let slot=map.entry(label.to_owned()).or_default();
    if slot.active{return Err("Live vision already active on this tab".into());}
    slot.epoch=slot.epoch.wrapping_add(1);
    slot.frame=None;
    slot.active=true;
    Ok(slot.epoch)
}
fn store(label:&str,encoded:&str){
    // Even malformed or oversized CDP packets cannot grow unbounded memory.
    if encoded.is_empty()||encoded.len()>MAX_FRAME_CHARS{return;}
    let mut map=frames().lock().unwrap();
    if let Some(slot)=map.get_mut(label){
        if slot.active{
            slot.frame=Some((Instant::now(),encoded.to_owned()));
        }
    }
}
fn latest(label:&str,epoch:u64)->Option<String>{
    let map=frames().lock().unwrap();
    let slot=map.get(label)?;
    if !slot.active||slot.epoch!=epoch{return None;}
    let (received,encoded)=slot.frame.as_ref()?;
    (received.elapsed()<=FRAME_MAX_AGE).then(||encoded.clone())
}
fn finish(label:&str,epoch:u64){
    let mut map=frames().lock().unwrap();
    if let Some(slot)=map.get_mut(label){
        if slot.epoch==epoch {
            slot.active=false;
            slot.frame=None;
        }
    }
}
pub fn forget(label:&str){frames().lock().unwrap().remove(label);}
fn accepting(label:&str)->bool{
    frames().lock().unwrap().get(label).is_some_and(|slot|slot.active)
}

/// One CDP listener per child WebView2. Subscription lives only as long as
/// that WebView; the screencast itself starts ONLY inside a gated visual
/// observation and is stopped before returning.
#[cfg(windows)]
pub fn attach(child:&tauri::Webview,label:String)->Result<(),String>{
    use webview2_com::{CallDevToolsProtocolMethodCompletedHandler,CoTaskMemPWSTR,
        DevToolsProtocolEventReceivedEventHandler};
    child.with_webview(move |native|{
        let Ok(core)=(unsafe{native.controller().CoreWebView2()}) else{return;};
        let name=CoTaskMemPWSTR::from("Page.screencastFrame");
        let Ok(receiver)=(unsafe{core.GetDevToolsProtocolEventReceiver(*name.as_ref().as_pcwstr())}) else {
            tracing::warn!("Live WebView2 frame event receiver unavailable");return;
        };
        let handler=DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |sender,args|{
            let Some(args)=args else{return Ok(());};
            // Frame packet is untrusted data. Restrict size before parsing;
            // store only compressed pixels, no metadata or page URLs.
            let mut raw=std::mem::MaybeUninit::zeroed();
            if unsafe{args.ParameterObjectAsJson(raw.as_mut_ptr())}.is_err(){return Ok(());}
            let payload=CoTaskMemPWSTR::from(unsafe{raw.assume_init()}).to_string();
            if payload.len()>MAX_FRAME_CHARS+1024{return Ok(());}
            let Ok(value)=serde_json::from_str::<serde_json::Value>(&payload) else{return Ok(());};
            let Some(frame_id)=value.get("sessionId").and_then(|v|v.as_u64()) else{return Ok(());};
            if accepting(&label){
                if let Some(data)=value.get("data").and_then(|v|v.as_str()){
                    store(&label,data);
                }
            }
            // Ack each frame so the streaming encoder can drop superseded
            // frames rather than stalling after its first delivery.
            if let Some(sender)=sender{
                let method=CoTaskMemPWSTR::from("Page.screencastFrameAck");
                let params=CoTaskMemPWSTR::from(format!(r#"{{"sessionId":{frame_id}}}"#).as_str());
                let cb=CallDevToolsProtocolMethodCompletedHandler::create(
                    Box::new(|_,_|Ok(())));
                let _=unsafe{sender.CallDevToolsProtocolMethod(
                    *method.as_ref().as_pcwstr(),
                    *params.as_ref().as_pcwstr(),&cb)};
            }
            Ok(())
        }));
        let mut token=Default::default();
        if let Err(e)=unsafe{receiver.add_DevToolsProtocolEventReceived(&handler,&mut token)}{
            tracing::warn!("Live WebView2 frame subscription failed: {e}");
        }
    }).map_err(|_|"Could not register native visual event receiver".to_owned())
}
#[cfg(not(windows))]
pub fn attach(_child:&tauri::Webview,_label:String)->Result<(),String>{Ok(())}

#[cfg(windows)]
async fn cdp(app:&tauri::AppHandle,label:&str,method_name:&'static str,parameters:&'static str)->Result<(),String>{
    use webview2_com::{CallDevToolsProtocolMethodCompletedHandler,CoTaskMemPWSTR};
    use tokio::sync::oneshot;
    use std::sync::{Arc,Mutex as StdMutex};
    let child=app.get_webview(label).ok_or("Visual task tab unavailable")?;
    let (tx,rx)=oneshot::channel::<Result<(),String>>();
    let out=Arc::new(StdMutex::new(Some(tx)));
    child.with_webview(move |native|{
        let complete=|result:Result<(),String>|{
            if let Some(tx)=out.lock().unwrap().take(){let _=tx.send(result);}
        };
        let Ok(core)=(unsafe{native.controller().CoreWebView2()}) else{
            complete(Err("Visual WebView2 interface unavailable".into()));return;
        };
        let callback_out=out.clone();
        let handler=CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status,_|{
            if let Some(tx)=callback_out.lock().unwrap().take(){
                let _=tx.send(status.map_err(|_|"Visual CDP method failed".to_string()));
            }
            Ok(())
        }));
        let method=CoTaskMemPWSTR::from(method_name);
        let params=CoTaskMemPWSTR::from(parameters);
        if unsafe{core.CallDevToolsProtocolMethod(
            *method.as_ref().as_pcwstr(),*params.as_ref().as_pcwstr(),&handler)}.is_err(){
            complete(Err("Visual CDP request could not be dispatched".into()));
        }
    }).map_err(|_|"Visual WebView2 dispatch unavailable".to_owned())?;
    tokio::time::timeout(Duration::from_secs(3),rx).await
        .map_err(|_|"Visual stream command timed out".to_owned())?
        .map_err(|_|"Visual stream command cancelled".to_owned())?
}
#[cfg(not(windows))]
async fn cdp(_: &tauri::AppHandle,_:&str,_:&'static str,_:&'static str)->Result<(),String>{
    Err("Live visual frames require Windows WebView2".into())
}

struct Session<'a> {
    app:&'a tauri::AppHandle,label:&'a str,epoch:u64,started:bool,
}
impl Drop for Session<'_>{
    fn drop(&mut self){
        finish(self.label,self.epoch);
        if self.started {
            // A best-effort stop is also issued on timeout/error paths. The
            // receiver stays registered until the disposable tab is closed.
            let label=self.label.to_owned();
            if let Some(view)=self.app.get_webview(&label){
                let _=view.with_webview(move |native|{
                    #[cfg(windows)]{
                        use webview2_com::{CallDevToolsProtocolMethodCompletedHandler,CoTaskMemPWSTR};
                        if let Ok(core)=unsafe{native.controller().CoreWebView2()}{
                            let method=CoTaskMemPWSTR::from("Page.stopScreencast");
                            let params=CoTaskMemPWSTR::from("{}");
                            let cb=CallDevToolsProtocolMethodCompletedHandler::create(Box::new(|_,_|Ok(())));
                            let _=unsafe{core.CallDevToolsProtocolMethod(
                                *method.as_ref().as_pcwstr(),*params.as_ref().as_pcwstr(),&cb)};
                        }
                    }
                    #[cfg(not(windows))]{let _=native;}
                });
            }
        }
    }
}

/// Sample the freshest frame from a REAL live WebView2 CDP screencast.
/// No Page.captureScreenshot is issued here; no raw frame is persisted.
/// Caller must independently check URL/viewport and live DOM after inference.
pub async fn sample(app:&tauri::AppHandle,label:&str)->Result<String,String>{
    let epoch=begin(label)?;
    let mut session=Session{app,label,epoch,started:false};
    // Ask for the first available frame even on an otherwise static page.
    // EveryNthFrame=15 can suppress every frame on a low-redraw page.
    // The host keeps only the latest compressed frame (no frame queue).
    cdp(app,label,"Page.startScreencast",
        r#"{"format":"jpeg","quality":45,"maxWidth":960,"maxHeight":640,"everyNthFrame":1}"#).await?;
    session.started=true;
    let deadline=tokio::time::Instant::now()+STREAM_WAIT;
    loop {
        if let Some(frame)=latest(label,epoch){
            // Await the real stop acknowledgment before the next session can
            // start: an asynchronous stop from Drop could cancel that stream.
            cdp(app,label,"Page.stopScreencast","{}").await?;
            session.started=false;
            return Ok(frame);
        }
        if tokio::time::Instant::now()>=deadline{
            return Err("Live visual frame stream timed out".into());
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_latest_frame_and_epoch_are_readable(){
        let id="live-frame-unit-test";
        forget(id);
        let first=begin(id).unwrap();
        store(id,"first");
        store(id,"second");
        assert_eq!(latest(id,first).as_deref(),Some("second"));
        finish(id,first);
        let next=begin(id).unwrap();
        assert!(latest(id,first).is_none());
        assert!(latest(id,next).is_none());
        store(id,"new");
        finish(id,first); // stale teardown cannot revoke a newer session
        assert_eq!(latest(id,next).as_deref(),Some("new"));
        forget(id);
        assert!(latest(id,next).is_none());
    }
    #[test]
    fn bounded_frames_and_no_private_payload_retention(){
        let id="live-frame-bounds-test";
        forget(id);
        let epoch=begin(id).unwrap();
        store(id,&"X".repeat(MAX_FRAME_CHARS+1));
        assert!(latest(id,epoch).is_none());
        finish(id,epoch);
        store(id,"should_not_retain");
        assert!(latest(id,epoch).is_none());
        forget(id);
    }
}
