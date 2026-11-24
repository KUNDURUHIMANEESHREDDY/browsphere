//! Opt-in OAuth authorization for a registered native public client.
//! No credentials, tokens, callbacks or metadata are passed to a model.
use anyhow::{ensure,Context,Result};
use base64::{Engine as _,engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Client;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::HashMap,time::{Duration,Instant}};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpListener,sync::Mutex};
use url::Url;
use uuid::Uuid;
use crate::storage::credentials::CredentialStore;

const LIMIT:usize=65536;
const FLOW_TTL:Duration=Duration::from_secs(180);
#[derive(Default)]
pub struct OAuthRegistry(Mutex<HashMap<String,Pending>>);
struct Pending {
    resource:Url, token_endpoint:Url, client_id:String, verifier:String,
    state:String, redirect:Url, issuer:Url, listener:TcpListener, created:Instant,
}
fn client()->Result<Client>{
    Ok(Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20)).build()?)
}
fn https(raw:&str)->Result<Url>{
    let u=Url::parse(raw)?;
    ensure!(u.scheme()=="https" && u.host_str().is_some() && u.username().is_empty()
        && u.password().is_none() && u.query().is_none() && u.fragment().is_none(),
        "Use a clean HTTPS URL without user info, query, or fragment");
    Ok(u)
}
fn well_known(u:&Url,name:&str,with_path:bool)->Url{
    let mut out=u.clone();
    let tail=if with_path && u.path()!="/" {u.path()} else {""};
    out.set_path(&format!("/.well-known/{name}{tail}"));
    out
}
fn pkce(verifier:&str)->String{
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn random_value()->String{
    // Two independent OS-random UUID v4 values; 244 random bits.
    format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple())
}
fn account(resource:&Url)->String{
    format!("mcp-oauth:{}",URL_SAFE_NO_PAD.encode(Sha256::digest(resource.as_str().as_bytes())))
}
fn creds()->CredentialStore{CredentialStore::new("BrowsphereAI")}
pub fn access_token_for(resource:&Url)->Result<Option<String>>{
    if resource.scheme()!="https"{return Ok(None);}
    let Some(raw)=creds().get_secret(&account(resource))? else{
        return if provider_preset(resource).is_some(){creds().get_secret(&format!("{}:api-key",account(resource)))}else{Ok(None)};
    };
    let v:Value=serde_json::from_str(&raw).context("Invalid saved MCP authorization")?;
    ensure!(v["resource"]==resource.as_str(),"MCP token resource mismatch");
    if let Some(exp)=v["expires_at"].as_i64(){
        ensure!(chrono::Utc::now().timestamp()+15<exp,"MCP authorization expired; authorize again");
    }
    let token=v["access_token"].as_str().context("Missing MCP token")?;
    ensure!(!token.is_empty() && token.len()<=8192 && !token.chars().any(char::is_whitespace),"Invalid MCP token");
    Ok(Some(token.into()))
}
async fn json_response(mut r:reqwest::Response)->Result<Value>{
    ensure!(r.status().is_success(),"Authorization HTTP status {}",r.status().as_u16());
    let mut bytes=Vec::new();
    while let Some(chunk)=r.chunk().await?{
        ensure!(bytes.len()+chunk.len()<=LIMIT,"Authorization response too large");
        bytes.extend_from_slice(&chunk);
    }
    // Never put raw server errors, authorization codes or tokens into diagnostics.
    serde_json::from_slice(&bytes).map_err(|_|anyhow::anyhow!("Invalid authorization metadata or token response"))
}
fn resource_metadata(v:&Value,resource:&Url,issuer:&Url)->Result<()>{
    ensure!(v["resource"].as_str().and_then(|s|https(s).ok()).as_ref()==Some(resource),
        "Protected resource metadata does not match the configured MCP URL");
    ensure!(v["authorization_servers"].as_array().is_some_and(|a|a.iter().any(|s|
        s.as_str().and_then(|s|https(s).ok()).as_ref()==Some(issuer))),
        "The configured issuer is not advertised by this MCP resource");
    Ok(())
}
fn endpoints(v:&Value,issuer:&Url)->Result<(Url,Url)>{
    ensure!(v["issuer"].as_str().and_then(|s|https(s).ok()).as_ref()==Some(issuer),"Issuer metadata mismatch");
    ensure!(v["code_challenge_methods_supported"].as_array().is_some_and(|a|a.iter().any(|x|x=="S256")),
        "Issuer must advertise S256 PKCE");
    let authorize=https(v["authorization_endpoint"].as_str().context("Missing authorization endpoint")?)?;
    let token=https(v["token_endpoint"].as_str().context("Missing token endpoint")?)?;
    // Conservative native-client subset: refuse cross-origin endpoints rather than
    // trusting metadata to redirect authorization codes to another service.
    ensure!(authorize.origin()==issuer.origin() && token.origin()==issuer.origin(),"Cross-origin OAuth endpoints not supported");
    Ok((authorize,token))
}
fn callback_code(target:&str,redirect:&Url,state:&str,issuer:&Url)->Result<String>{
    ensure!(target.starts_with("/oauth/callback?") && target.len()<=8192,"Invalid callback target");
    let u=redirect.join(target)?;
    ensure!(u.origin()==redirect.origin() && u.path()==redirect.path(),"Callback address mismatch");
    let mut fields=HashMap::new();
    for (k,v) in u.query_pairs(){
        ensure!(fields.insert(k.into_owned(),v.into_owned()).is_none(),"Duplicate callback field");
    }
    ensure!(fields.get("state").map(String::as_str)==Some(state),"OAuth state mismatch");
    if let Some(iss)=fields.get("iss"){ensure!(https(iss)?==*issuer,"Callback issuer mismatch");}
    ensure!(!fields.contains_key("error"),"Authorization declined or failed");
    let code=fields.remove("code").context("Authorization code missing")?;
    ensure!(!code.is_empty() && code.len()<=4096 && !code.chars().any(char::is_control),"Invalid authorization code");
    Ok(code)
}
async fn callback(p:&Pending)->Result<String>{
    let remaining=FLOW_TTL.checked_sub(p.created.elapsed()).context("Authorization expired")?;
    tokio::time::timeout(remaining,async{
        for _ in 0..12{
            let (mut socket,peer)=p.listener.accept().await?;
            ensure!(peer.ip().is_loopback(),"Invalid callback peer");
            let request=tokio::time::timeout(Duration::from_secs(4),async{
                let mut data=Vec::new();let mut buf=[0u8;1024];
                loop{
                    let n=socket.read(&mut buf).await?;
                    ensure!(n>0 && data.len()+n<=8192,"Invalid callback request");
                    data.extend_from_slice(&buf[..n]);
                    if data.windows(4).any(|w|w==b"\r\n\r\n"){break;}
                }
                let head=std::str::from_utf8(&data)?;
                let first=head.lines().next().context("Missing callback request")?;
                let mut parts=first.split_whitespace();
                ensure!(parts.next()==Some("GET"),"Invalid callback method");
                let target=parts.next().context("Missing callback path")?;
                let host=format!("127.0.0.1:{}",p.redirect.port().context("Missing callback port")?);
                ensure!(head.lines().any(|l|l.split_once(':').is_some_and(|(k,v)|k.eq_ignore_ascii_case("host")&&v.trim()==host)),
                    "Callback host mismatch");
                callback_code(target,&p.redirect,&p.state,&p.issuer)
            }).await;
            let result=match request {Ok(r)=>r,Err(_)=>Err(anyhow::anyhow!("Callback timed out"))};
            let body=if result.is_ok(){"Authorization response received. Return to Browsphere."}else{"Invalid or expired authorization response."};
            let status=if result.is_ok(){"200 OK"}else{"400 Bad Request"};
            let _=tokio::time::timeout(Duration::from_secs(1),socket.write_all(format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes())).await;
            if result.is_ok(){return result;}
        }
        anyhow::bail!("No valid OAuth callback received")
    }).await.context("Authorization expired")?
}
fn provider_preset(resource:&Url)->Option<(&'static str,&'static str)>{
    match resource.as_str(){
        "https://mcp.exa.ai/mcp/oauth"=>Some(("https://auth.exa.ai","mcp:tools")),
        "https://mcp.tavily.com/mcp"=>Some(("https://mcp.tavily.com/","openid")),
        _=>None
    }
}
fn registration_endpoint(metadata:&Value,issuer:&Url)->Result<Url>{
    ensure!(metadata["token_endpoint_auth_methods_supported"].as_array().is_some_and(|a|a.iter().any(|v|v=="none")),"Issuer does not advertise public clients");
    let endpoint=https(metadata["registration_endpoint"].as_str().context("Dynamic client registration unavailable")?)?;
    ensure!(endpoint.origin()==issuer.origin(),"Cross-origin registration endpoint not supported");
    Ok(endpoint)
}
async fn register_public_client(http:&Client,metadata:&Value,issuer:&Url,redirect:&Url)->Result<String>{
    let endpoint=registration_endpoint(metadata,issuer)?;
    let response=json_response(http.post(endpoint).json(&json!({
        "client_name":"Browsphere Lucia","application_type":"native",
        "redirect_uris":[redirect.as_str()],"grant_types":["authorization_code"],
        "response_types":["code"],"token_endpoint_auth_method":"none"
    })).send().await?).await?;
    ensure!(response["token_endpoint_auth_method"]=="none","Registration did not return a public client");
    ensure!(response["redirect_uris"].as_array().is_some_and(|a|a.iter().any(|v|v==redirect.as_str())),"Registered callback mismatch");
    ensure!(response.get("client_secret").is_none()||response["client_secret"].is_null(),"Unexpected confidential client");
    let id=response["client_id"].as_str().context("Registration missing client ID")?;
    ensure!(!id.trim().is_empty()&&id.len()<=512&&!id.chars().any(char::is_control),"Invalid registered client ID");
    Ok(id.to_owned())
}
fn trusted(view:&tauri::Webview)->Result<()>{
    ensure!(view.label()=="main","Only trusted browser settings may authorize MCP");
    Ok(())
}
#[tauri::command]
pub async fn mcp_oauth_begin(view:tauri::Webview,registry:tauri::State<'_,OAuthRegistry>,
    resource:String,issuer:String,client_id:String,scope:String)->Result<Value,String>{
    async {
        trusted(&view)?;
        let resource=https(&resource)?;
        let automatic=client_id.trim().is_empty();
        let (issuer,scope)=if automatic{
            let (expected,default_scope)=provider_preset(&resource).context("Automatic registration is limited to official search provider presets")?;
            ensure!(issuer.trim().is_empty()||https(&issuer)?==https(expected)?,"Preset issuer mismatch");
            (https(expected)?,default_scope.to_owned())
        }else{(https(&issuer)?,scope)};
        ensure!(client_id.len()<=512 && !client_id.chars().any(char::is_control),"Invalid public client ID");
        ensure!(scope.len()<=512 && !scope.chars().any(char::is_control),"Invalid scope");
        let http=client()?;
        let metadata=json_response(http.get(well_known(&resource,"oauth-protected-resource",true)).send().await?).await?;
        resource_metadata(&metadata,&resource,&issuer)?;
        let metadata=json_response(http.get(well_known(&issuer,"oauth-authorization-server",true)).send().await?).await?;
        let (mut authorize,token_endpoint)=endpoints(&metadata,&issuer)?;
        let listener=TcpListener::bind("127.0.0.1:0").await?;
        let redirect=Url::parse(&format!("http://127.0.0.1:{}/oauth/callback",listener.local_addr()?.port()))?;
        let client_id=if automatic{register_public_client(&http,&metadata,&issuer,&redirect).await?}else{client_id};
        let verifier=random_value();let state=random_value();
        authorize.query_pairs_mut().append_pair("response_type","code")
            .append_pair("client_id",&client_id).append_pair("redirect_uri",redirect.as_str())
            .append_pair("code_challenge",&pkce(&verifier)).append_pair("code_challenge_method","S256")
            .append_pair("state",&state).append_pair("resource",resource.as_str());
        if !scope.trim().is_empty(){authorize.query_pairs_mut().append_pair("scope",scope.trim());}
        let id=Uuid::new_v4().to_string();
        let result=json!({"id":id,"authorizationUrl":authorize.as_str(),"redirectUri":redirect.as_str(),"expiresIn":180});
        let mut entries=registry.0.lock().await;
        entries.retain(|_,p|p.created.elapsed()<FLOW_TTL);
        ensure!(entries.len()<4,"Too many pending authorizations");
        entries.insert(id,Pending{resource,token_endpoint,client_id,verifier,state,redirect,issuer,listener,created:Instant::now()});
        Ok(result)
    }.await.map_err(|e:anyhow::Error|e.to_string())
}
#[tauri::command]
pub async fn mcp_oauth_finish(view:tauri::Webview,registry:tauri::State<'_,OAuthRegistry>,id:String)->Result<(),String>{
    async{
        trusted(&view)?;
        let p=registry.0.lock().await.remove(&id).context("Authorization expired or already consumed")?;
        let code=callback(&p).await?;
        let token=json_response(client()?.post(p.token_endpoint.clone()).form(&[
            ("grant_type","authorization_code"),("client_id",p.client_id.as_str()),
            ("code",code.as_str()),("code_verifier",p.verifier.as_str()),
            ("redirect_uri",p.redirect.as_str()),("resource",p.resource.as_str())
        ]).send().await?).await?;
        ensure!(token["token_type"].as_str().is_some_and(|s|s.eq_ignore_ascii_case("bearer")),"Expected a bearer access token");
        let access=token["access_token"].as_str().context("Access token missing")?;
        ensure!(!access.is_empty() && access.len()<=8192 && !access.chars().any(char::is_whitespace),"Invalid access token");
        let expiry=token["expires_in"].as_i64().map(|n|chrono::Utc::now().timestamp().saturating_add(n.max(0)));
        // No refresh token is retained. Reauthorization is explicit.
        creds().set_secret(&account(&p.resource),&json!({"resource":p.resource.as_str(),"access_token":access,"expires_at":expiry}).to_string())?;
        Ok(())
    }.await.map_err(|_:anyhow::Error|"MCP authorization failed or expired. No token was returned to the UI.".into())
}
#[tauri::command]
pub async fn mcp_oauth_forget(view:tauri::Webview,resource:String)->Result<(),String>{
    (||{trusted(&view)?;creds().delete_secret(&account(&https(&resource)?))})()
        .map_err(|_|"Could not clear MCP authorization".into())
}
#[tauri::command]
pub async fn mcp_provider_key(view:tauri::Webview,resource:String,key:String)->Result<(),String>{
    (||{
        trusted(&view)?;
        let resource=https(&resource)?;
        ensure!(provider_preset(&resource).is_some(),"API keys supported only for exact official search provider endpoints");
        let account=format!("{}:api-key",account(&resource));
        if key.is_empty(){return creds().delete_secret(&account);}
        ensure!(key.len()<=1024&&!key.chars().any(|c|c.is_whitespace()||c.is_control()),"Invalid API key");
        creds().set_secret(&account,&key)
    })().map_err(|_|"Could not update provider API key".into())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn official_presets_and_registration_are_bound(){
        assert_eq!(provider_preset(&https("https://mcp.exa.ai/mcp/oauth").unwrap()),Some(("https://auth.exa.ai","mcp:tools")));
        assert_eq!(provider_preset(&https("https://mcp.tavily.com/mcp").unwrap()),Some(("https://mcp.tavily.com/","openid")));
        assert!(provider_preset(&https("https://mcp.exa.ai.evil.test/mcp/oauth").unwrap()).is_none());
        let issuer=https("https://auth.exa.ai").unwrap();
        let mut m=json!({"registration_endpoint":"https://auth.exa.ai/api/oauth/register","token_endpoint_auth_methods_supported":["none"]});
        assert!(registration_endpoint(&m,&issuer).is_ok());
        m["registration_endpoint"]=json!("https://evil.test/register");
        assert!(registration_endpoint(&m,&issuer).is_err());
    }
    #[test]fn pkce_and_callback_binding(){
        assert_eq!(pkce("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),"E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let redirect=Url::parse("http://127.0.0.1:12345/oauth/callback").unwrap();
        let issuer=https("https://auth.example/").unwrap();
        assert_eq!(callback_code("/oauth/callback?state=abc&code=xyz",&redirect,"abc",&issuer).unwrap(),"xyz");
        for bad in ["/oauth/callback?state=bad&code=x","/oauth/callback?state=abc&code=x&code=y","/oauth/callback?state=abc&code=x&iss=https%3A%2F%2Fevil.example","/other?state=abc&code=x"]{
            assert!(callback_code(bad,&redirect,"abc",&issuer).is_err());
        }
    }
    #[test]fn metadata_and_token_resource_are_bound(){
        let r=https("https://example.org/mcp").unwrap();let i=https("https://auth.example/").unwrap();
        assert_eq!(well_known(&r,"oauth-protected-resource",true).as_str(),"https://example.org/.well-known/oauth-protected-resource/mcp");
        assert!(resource_metadata(&json!({"resource":r.as_str(),"authorization_servers":[i.as_str()]}),&r,&i).is_ok());
        assert!(resource_metadata(&json!({"resource":"https://example.org/other","authorization_servers":[i.as_str()]}),&r,&i).is_err());
        assert_ne!(account(&r),account(&https("https://example.org/other").unwrap()));
        let m=json!({"issuer":i.as_str(),"code_challenge_methods_supported":["S256"],"authorization_endpoint":"https://auth.example/authorize","token_endpoint":"https://auth.example/token"});
        assert!(endpoints(&m,&i).is_ok());
        let mut bad=m;bad["token_endpoint"]=json!("https://other.example/token");
        assert!(endpoints(&bad,&i).is_err());
    }
}
