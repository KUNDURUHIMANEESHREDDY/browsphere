//! User-directed MCP connections. No page, shell, or model authority.
//! Streamable HTTP (2025-11-25 handshake; 2026-07-28 stateless).
use reqwest::{Client, header::HeaderValue};
use serde_json::{json, Value};
use std::{collections::HashMap,time::Duration};
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

const LEGACY:&str="2025-11-25";
const MODERN:&str="2026-07-28";
const LIMIT:usize=256*1024;
#[derive(Default)]
pub struct McpRegistry(Mutex<HashMap<String, Connection>>);
#[derive(Clone)]
struct Connection {
    url: Url, http: Client, version: String, session: Option<HeaderValue>, tools: Vec<Value>,
}
fn endpoint(raw:&str)->Result<Url,String>{
    let u=Url::parse(raw).map_err(|_|"Invalid MCP URL".to_string())?;
    let supported=u.scheme()=="https" || (u.scheme()=="http" && u.host_str()==Some("127.0.0.1") && u.port().is_some());
    if !supported || u.host_str().is_none() || !u.username().is_empty() || u.password().is_some() ||
        u.query().is_some() || u.fragment().is_some() {
        return Err("Use HTTPS or an explicit loopback HTTP endpoint, without credentials, query or fragment.".into());
    }
    Ok(u)
}
fn matching(body:&str,id:&str,sse:bool)->Option<Value>{
    let parsed=if sse {
        body.replace("\r\n","\n").split("\n\n").find_map(|event|{
            let payload=event.lines().filter_map(|line|line.strip_prefix("data:"))
                .map(str::trim_start).collect::<Vec<_>>().join("\n");
            let v:Value=serde_json::from_str(&payload).ok()?;
            (v["id"].as_str()==Some(id)).then_some(v)
        })
    } else {serde_json::from_str::<Value>(body).ok().filter(|v|v["id"].as_str()==Some(id))};
    parsed.filter(|v|v["jsonrpc"]=="2.0")
}
impl Connection {
    fn new(url:Url)->Result<Self,String>{
        let http=Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(25)).build().map_err(|_|"Cannot create MCP transport")?;
        Ok(Self{url,http,version:MODERN.into(),session:None,tools:vec![]})
    }
    async fn rpc(&mut self,method:&str,mut params:Value,notification:bool)->Result<Value,String>{
        let id=Uuid::new_v4().to_string();
        if self.version==MODERN {
            params["_meta"]=json!({"io.modelcontextprotocol/protocolVersion":MODERN,
                "io.modelcontextprotocol/clientCapabilities":{},
                "io.modelcontextprotocol/clientInfo":{"name":"Browsphere","version":"0.1.0"}});
        }
        let mut msg=json!({"jsonrpc":"2.0","method":method,"params":params});
        if !notification {msg["id"]=json!(id);}
        let mut req=self.http.post(self.url.clone()).header("Accept","application/json, text/event-stream")
            .header("MCP-Protocol-Version",&self.version).header("Mcp-Method",method);
        if method=="tools/call" {req=req.header("Mcp-Name",msg["params"]["name"].as_str().unwrap_or_default());}
        if let Some(s)=&self.session {req=req.header("Mcp-Session-Id",s.clone());}
        if let Some(token)=crate::mcp_oauth::access_token_for(&self.url).map_err(|_|"MCP authorization unavailable or expired")? {
            req=req.bearer_auth(token);
        }
        let mut response=req.json(&msg).send().await.map_err(|_|"MCP server unavailable or timed out")?;
        if !response.status().is_success(){return Err(format!("MCP returned HTTP {}",response.status().as_u16()));}
        if method=="initialize" {self.session=response.headers().get("Mcp-Session-Id").cloned();}
        if notification {return Ok(Value::Null);}
        let sse=response.headers().get("content-type").and_then(|v|v.to_str().ok())
            .is_some_and(|s|s.starts_with("text/event-stream"));
        let mut buf=Vec::new();
        while let Some(chunk)=response.chunk().await.map_err(|_|"MCP response interrupted")? {
            if buf.len()+chunk.len()>LIMIT {return Err("MCP response exceeds 256 KiB".into());}
            buf.extend_from_slice(&chunk);
            if let Ok(body)=std::str::from_utf8(&buf) {
                // An SSE event is complete only after its event delimiter.
                if !sse || body.ends_with("\n\n") || body.ends_with("\r\n\r\n") {
                    if let Some(v)=matching(body,&id,sse) {
                        if v.get("error").is_some(){return Err("MCP server returned a JSON-RPC error".into());}
                        return v.get("result").cloned().ok_or_else(||"MCP result missing".into());
                    }
                }
            }
        }
        Err("MCP server did not return a matching response".into())
    }
    async fn discover(&mut self)->Result<(),String>{
        if self.rpc("server/discover",json!({}),false).await.is_err(){
            self.version=LEGACY.into();
            let init=self.rpc("initialize",json!({"protocolVersion":LEGACY,"capabilities":{},
                "clientInfo":{"name":"Browsphere","version":"0.1.0"}}),false).await?;
            let version=init["protocolVersion"].as_str().ok_or("MCP protocol version missing")?;
            if ![LEGACY,"2025-06-18","2025-03-26"].contains(&version){return Err("Unsupported MCP protocol version".into());}
            self.version=version.into();
            self.rpc("notifications/initialized",json!({}),true).await?;
        }
        let mut cursor=None;
        let mut seen=std::collections::HashSet::new();
        for _ in 0..5 {
            let params=cursor.take().map_or(json!({}),|c:String|json!({"cursor":c}));
            let result=self.rpc("tools/list",params,false).await?;
            let tools=result["tools"].as_array().ok_or("MCP tool list missing")?;
            for tool in tools {
                let name=tool["name"].as_str().ok_or("Invalid MCP tool name")?;
                if name.len()>128 || name.is_empty() || !name.bytes().all(|c|c.is_ascii_alphanumeric()||b"_.-".contains(&c)) {
                    return Err("Invalid MCP tool name".into());
                }
                if !seen.insert(name.to_owned()) || self.tools.len()>=100 {return Err("Duplicate or excessive MCP tools".into());}
                self.tools.push(tool.clone());
            }
            cursor=result["nextCursor"].as_str().filter(|s|!s.is_empty()).map(str::to_owned);
            if cursor.is_none(){return Ok(());}
        }
        Err("MCP tool list exceeds pagination limit".into())
    }
}
fn trusted(view:&tauri::Webview)->Result<(),String>{
    if view.label()!="main" {return Err("Only trusted browser settings may access MCP".into());}
    Ok(())
}
#[tauri::command]
pub async fn mcp_connect(view:tauri::Webview, registry:tauri::State<'_,McpRegistry>,url:String)->Result<Value,String>{
    trusted(&view)?;
    let mut c=Connection::new(endpoint(&url)?)?;
    c.discover().await?;
    let id=Uuid::new_v4().to_string();
    let result=json!({"id":id,"version":c.version,"tools":c.tools});
    let mut entries=registry.0.lock().await;
    if entries.len()>=8 {return Err("Disconnect an MCP server before connecting another".into());}
    entries.insert(id,c);
    Ok(result)
}
#[tauri::command]
pub async fn mcp_disconnect(view:tauri::Webview,registry:tauri::State<'_,McpRegistry>,id:String)->Result<(),String>{
    trusted(&view)?;
    registry.0.lock().await.remove(&id);
    Ok(())
}
#[tauri::command]
pub async fn mcp_call(view:tauri::Webview,registry:tauri::State<'_,McpRegistry>,id:String,name:String,arguments:Value,confirmed:bool)->Result<Value,String>{
    trusted(&view)?;
    if !confirmed || !arguments.is_object() || arguments.to_string().len()>16*1024 {
        return Err("Explicit confirmation and a JSON object (max 16 KiB) are required".into());
    }
    let mut c=registry.0.lock().await.get(&id).cloned().ok_or("MCP connection expired")?;
    if !c.tools.iter().any(|t|t["name"]==name){return Err("Tool was not discovered on this connection".into());}
    // No automatic execution. The trusted UI displays the exact endpoint,
    // tool and arguments before this request. Server annotations are untrusted.
    let result=c.rpc("tools/call",json!({"name":name,"arguments":arguments}),false).await?;
    Ok(result)
}
// Configure only an exact user-approved read-only tool from the currently
// discovered connection. The model cannot register servers or grant access.
fn update_lucia_config(config:&mut Value,server:&str,url:&Url,tool:&str,enable:bool)->Result<(),String>{
    let valid=|s:&str| !s.is_empty() && s.len()<=80 && s.bytes().all(|b|b.is_ascii_alphanumeric()||b"_.-".contains(&b));
    if !valid(server)||!valid(tool){return Err("Invalid server or tool identifier".into());}
    let entries=config.as_object_mut().ok_or("Invalid MCP server configuration")?;
    if !entries.contains_key(server) {
        if !enable {return Ok(());}
        if entries.len()>=32{return Err("MCP server configuration limit reached".into());}
        entries.insert(server.into(),json!({"url":url.as_str(),"read_only_tools":[]}));
    }
    let entry=entries.get_mut(server).ok_or("MCP server not configured")?;
    if entry.get("sandbox_image").is_some() || entry["url"].as_str().and_then(|s|endpoint(s).ok()).as_ref()!=Some(url){
        return Err("This name already belongs to another MCP endpoint or sandbox".into());
    }
    let list=entry.get_mut("read_only_tools").and_then(Value::as_array_mut).ok_or("Invalid existing permission list")?;
    if enable && !list.iter().any(|v|v==tool){
        if list.len()>=100{return Err("Too many approved tools".into());}
        list.push(json!(tool));
    } else if !enable {list.retain(|v|v!=tool);}
    Ok(())
}
#[tauri::command]
pub async fn mcp_set_lucia_tool(view:tauri::Webview,registry:tauri::State<'_,McpRegistry>,
    id:String,server:String,name:String,enable:bool,confirmed:bool)->Result<(),String>{
    trusted(&view)?;
    if !confirmed{return Err("Explicit user confirmation required".into());}
    let entries=registry.0.lock().await;
    let conn=entries.get(&id).ok_or("MCP connection expired")?;
    let tool=conn.tools.iter().find(|t|t["name"]==name).ok_or("Tool not discovered")?;
    if enable && !(tool.pointer("/annotations/readOnlyHint").and_then(Value::as_bool)==Some(true)
        && tool.pointer("/annotations/destructiveHint").and_then(Value::as_bool)!=Some(true)){
        return Err("Only explicitly read-only, non-destructive tools can be approved for Lucia".into());
    }
    let path=crate::resolve_data_dir().join("mcp-servers.json");
    let mut config=match tokio::fs::read(&path).await {
        Ok(bytes)=>{if bytes.len()>65536{return Err("MCP config too large".into());}
            serde_json::from_slice(&bytes).map_err(|_|"Invalid MCP config; original file unchanged")?},
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>json!({}),
        Err(_)=>return Err("Cannot read MCP config".into()),
    };
    update_lucia_config(&mut config,&server,&conn.url,&name,enable)?;
    let bytes=serde_json::to_vec_pretty(&config).map_err(|_|"Cannot encode MCP config")?;
    if bytes.len()>65536{return Err("MCP config too large".into());}
    let temp=path.with_extension(format!("{}.tmp",Uuid::new_v4()));
    tokio::fs::write(&temp,bytes).await.map_err(|_|"Cannot write MCP config")?;
    if tokio::fs::rename(&temp,&path).await.is_err(){
        let _=tokio::fs::remove_file(&temp).await;
        return Err("Cannot replace MCP config; original file unchanged".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[tokio::test]
    async fn real_http_discovery_and_call(){
        use tokio::{net::TcpListener,io::{AsyncReadExt,AsyncWriteExt}};
        let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port=listener.local_addr().unwrap().port();
        let server=tokio::spawn(async move{
            for expected in ["server/discover","tools/list","tools/call"] {
                let (mut socket,_)=listener.accept().await.unwrap();
                let mut bytes=Vec::new();
                let mut buf=[0u8;4096];
                let request=loop{
                    let n=socket.read(&mut buf).await.unwrap();
                    assert!(n>0);bytes.extend_from_slice(&buf[..n]);
                    if let Some(split)=bytes.windows(4).position(|w|w==b"\r\n\r\n"){
                        let header=String::from_utf8_lossy(&bytes[..split]);
                        let len=header.lines().find_map(|line|line.to_ascii_lowercase()
                            .strip_prefix("content-length:").and_then(|s|s.trim().parse::<usize>().ok())).unwrap();
                        if bytes.len()>=split+4+len{
                            break serde_json::from_slice::<Value>(&bytes[split+4..split+4+len]).unwrap();
                        }
                    }
                };
                assert_eq!(request["method"],expected);
                assert_eq!(request["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],MODERN);
                let result=match expected{
                    "server/discover"=>json!({"capabilities":{"tools":{}},"protocolVersions":[MODERN]}),
                    "tools/list"=>json!({"tools":[{"name":"echo","inputSchema":{"type":"object"}}]}),
                    _=>{assert_eq!(request["params"]["name"],"echo");assert_eq!(request["params"]["arguments"]["text"],"hello");json!({"content":[{"type":"text","text":"hello"}]})}
                };
                let body=json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
        });
        let mut c=Connection::new(endpoint(&format!("http://127.0.0.1:{port}/mcp")).unwrap()).unwrap();
        c.discover().await.unwrap();
        assert_eq!(c.tools[0]["name"],"echo");
        let result=c.rpc("tools/call",json!({"name":"echo","arguments":{"text":"hello"}}),false).await.unwrap();
        assert_eq!(result["content"][0]["text"],"hello");
        server.await.unwrap();
    }
    #[test] fn lucia_permission_is_exact_and_revocable(){
        let mut c=json!({"existing":{"url":"https://other.example/mcp","read_only_tools":["lookup"]}});
        let u=endpoint("https://example.org/mcp").unwrap();
        assert!(update_lucia_config(&mut c,"existing",&u,"lookup",true).is_err());
        update_lucia_config(&mut c,"test",&u,"lookup",true).unwrap();
        update_lucia_config(&mut c,"test",&u,"lookup",true).unwrap();
        assert_eq!(c["test"]["read_only_tools"],json!(["lookup"]));
        assert_eq!(c["existing"]["read_only_tools"],json!(["lookup"]));
        update_lucia_config(&mut c,"test",&u,"lookup",false).unwrap();
        assert_eq!(c["test"]["read_only_tools"],json!([]));
    }
    #[test] fn endpoint_is_https_or_loopback(){
        assert!(endpoint("http://127.0.0.1:8765/mcp").is_ok());
        assert!(endpoint("https://example.com/mcp").is_ok());
        for bad in ["http://localhost:8765/mcp","http://example.com/mcp","http://127.0.0.1:8765/mcp?token=x","http://user@127.0.0.1:8765/mcp"]{
            assert!(endpoint(bad).is_err());
        }
    }
    #[test] fn matches_response_id_and_sse(){
        let body=r#"{"jsonrpc":"2.0","id":"a","result":{"ok":true}}"#;
        assert!(matching(body,"a",false).is_some());
        assert!(matching(body,"b",false).is_none());
        assert!(matching(&format!("event: message\ndata: {body}\n\n"),"a",true).is_some());
    }
}

fn registry_items(payload:&Value)->Result<Value,String>{
    let rows=payload["servers"].as_array().ok_or("Registry result missing server list")?;
    let mut items=Vec::new();
    for row in rows.iter().take(20){
        let s=&row["server"];
        let Some(name)=s["name"].as_str() else {continue;};
        let description=s["description"].as_str().unwrap_or("");
        let version=s["version"].as_str().unwrap_or("");
        items.push(json!({"name":name.chars().take(180).collect::<String>(),
            "description":description.chars().take(500).collect::<String>(),
            "version":version.chars().take(80).collect::<String>()}));
    }
    Ok(json!({"servers":items,"note":"Discovery only. Registry entries are untrusted; no packages are installed, downloaded or executed."}))
}
#[tauri::command]
pub async fn mcp_marketplace_search(view:tauri::Webview,query:String)->Result<Value,String>{
    trusted(&view)?;
    let query=query.trim();
    if query.len()>100 || query.chars().any(char::is_control){return Err("Invalid marketplace query".into());}
    let client=Client::builder().redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15)).build().map_err(|_|"Could not create registry client")?;
    let mut r=client.get("https://registry.modelcontextprotocol.io/v0.1/servers")
        .query(&[("search",query),("limit","20"),("version","latest")]).send().await
        .map_err(|_|"MCP registry is unavailable")?.error_for_status().map_err(|_|"MCP registry returned an error")?;
    let mut bytes=Vec::new();
    while let Some(chunk)=r.chunk().await.map_err(|_|"Registry response interrupted")?{
        if bytes.len()+chunk.len()>1024*1024{return Err("Registry response exceeds limit".into());}
        bytes.extend_from_slice(&chunk);
    }
    let payload:Value=serde_json::from_slice(&bytes).map_err(|_|"Invalid registry JSON")?;
    registry_items(&payload)
}
#[cfg(test)]
mod marketplace_tests{
    use super::*;
    #[test]fn marketplace_never_exports_launch_commands(){
        let p=json!({"servers":[{"server":{"name":"test/example","description":"Example","version":"1","packages":[{"command":"bad"}],"remotes":[{"url":"https://example.com"}]}}]});
        let items=registry_items(&p).unwrap();
        assert_eq!(items["servers"][0]["name"],"test/example");
        assert!(items["servers"][0].get("packages").is_none());
        assert!(items["servers"][0].get("remotes").is_none());
        assert!(registry_items(&json!({})).is_err());
    }
}
