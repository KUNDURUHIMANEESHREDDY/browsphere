
// A user explicitly configures an already-installed, digest-pinned Linux image.
// Never launch a server suggested by a model or registry response.
use anyhow::{ensure,Context,Result};
use serde_json::{json,Value};
use std::{path::PathBuf,process::Stdio};
use tokio::{io::{AsyncBufReadExt,AsyncWriteExt,BufReader},process::{Child,ChildStdin,ChildStdout,Command}};
const LIMIT:usize=262144;
fn image_valid(image:&str)->bool {
    let Some((name,digest))=image.split_once("@sha256:") else{return false;};
    !name.is_empty() && name.len()<=200 && !name.starts_with('-') &&
        name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/:".contains(&b)) &&
        digest.len()==64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
}
fn docker_path()->Result<PathBuf>{
    #[cfg(windows)]
    let path=PathBuf::from(std::env::var_os("ProgramFiles").context("ProgramFiles not set")?).join("Docker/Docker/resources/bin/docker.exe");
    #[cfg(not(windows))]
    let path=PathBuf::from("/usr/bin/docker");
    ensure!(path.is_file(),"Docker CLI not installed at the expected path");
    Ok(path)
}
fn host()->&'static str{
    #[cfg(windows)] { "npipe:////./pipe/dockerDesktopLinuxEngine" }
    #[cfg(not(windows))] { "unix:///var/run/docker.sock" }
}
fn base()->Result<Command>{
    let mut c=Command::new(docker_path()?);
    c.arg("--host").arg(host()).env_remove("DOCKER_CONTEXT").env_remove("DOCKER_HOST").env_remove("DOCKER_TLS_VERIFY").env_remove("DOCKER_CERT_PATH");
    Ok(c)
}
fn flags(name:&str,image:&str)->Vec<String>{
    ["run","--rm","-i","--pull=never","--name",name,"--network=none","--read-only",
    "--cap-drop=ALL","--security-opt=no-new-privileges","--user=65534:65534",
    "--memory=256m","--cpus=0.5","--pids-limit=64","--tmpfs","/tmp:rw,noexec,nosuid,nodev,size=32m",
    "--init",image].iter().map(|s|s.to_string()).collect()
}
pub struct SandboxSession{
    child:Child,
    input:ChildStdin,
    output:BufReader<ChildStdout>,
    name:String,
    id:u64,
}
impl Drop for SandboxSession{
    fn drop(&mut self){
        let _=self.child.start_kill();
        // Cancellation can drop this future mid-RPC. Remove the uniquely named
        // container using the fixed local engine, never any model-supplied name.
        if let Ok(mut c)=base(){
            let _=c.args(["rm","-f",&self.name]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        }
    }
}
impl SandboxSession {
    pub async fn connect(image:&str)->Result<Self>{
        ensure!(image_valid(image),"Sandbox requires an already-installed image pinned to a SHA-256 digest");
        let name=format!("browsphere-mcp-{}",uuid::Uuid::new_v4());
        let mut cmd=base()?;
        let mut child=cmd.args(flags(&name,image)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn().context("Cannot start isolated MCP container")?;
        let input=child.stdin.take().context("Missing container stdin")?;
        let output=BufReader::new(child.stdout.take().context("Missing container stdout")?);
        let mut s=Self{child,input,output,name,id:0};
        let init=s.post("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"Browsphere","version":"0.1"}}),false).await?;
        ensure!(matches!(init["protocolVersion"].as_str(),Some("2025-11-25"|"2025-06-18"|"2025-03-26")) && init.pointer("/capabilities/tools").is_some(),"Unsupported MCP sandbox handshake");
        s.post("notifications/initialized",json!({}),true).await?;
        Ok(s)
    }
    pub async fn post(&mut self,method:&str,params:Value,notification:bool)->Result<Value>{
        self.id+=1;
        let mut msg=json!({"jsonrpc":"2.0","method":method,"params":params});
        if !notification{msg["id"]=json!(self.id);}
        let mut data=serde_json::to_vec(&msg)?;
        ensure!(data.len()<=LIMIT,"MCP input too large");
        data.push(b'\n');
        self.input.write_all(&data).await?;
        self.input.flush().await?;
        if notification{return Ok(Value::Null);}
        for _ in 0..16 {
            let mut line=Vec::new();
            loop{
                let buf=self.output.fill_buf().await?;
                ensure!(!buf.is_empty(),"Sandbox server exited before responding");
                let end=buf.iter().position(|b|*b==b'\n').map(|i|i+1);
                let n=end.unwrap_or(buf.len());
                ensure!(line.len()+n<=LIMIT,"Sandbox MCP response too large");
                line.extend_from_slice(&buf[..n]);
                self.output.consume(n);
                if end.is_some(){break;}
            }
            let v:Value=serde_json::from_slice(&line)?;
            if v["id"]!=self.id{continue;}
            ensure!(v["jsonrpc"]=="2.0" && v.get("error").is_none(),"Sandbox MCP protocol error");
            return v.get("result").cloned().context("Missing sandbox MCP result");
        }
        anyhow::bail!("Too many unmatched sandbox messages")
    }
    pub async fn close(&mut self){
        let _=self.input.shutdown().await;
        let _=self.child.start_kill();
        if let Ok(mut c)=base(){
            let _=tokio::time::timeout(std::time::Duration::from_secs(5),c.args(["rm","-f",&self.name]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()).await;
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn pinned_image_and_isolation_flags(){
        let image=format!("registry.example/test@sha256:{}","a".repeat(64));
        assert!(image_valid(&image));
        assert!(!image_valid("alpine:latest"));
        assert!(!image_valid(&format!("--privileged@sha256:{}","a".repeat(64))));
        let f=flags("test",&image);
        for key in ["--pull=never","--network=none","--read-only","--cap-drop=ALL","--security-opt=no-new-privileges","--user=65534:65534","--pids-limit=64"]{
            assert!(f.iter().any(|s|s==key));
        }
        assert!(!f.iter().any(|s|s=="--privileged" || s=="-v" || s=="--mount"));
    }
}
