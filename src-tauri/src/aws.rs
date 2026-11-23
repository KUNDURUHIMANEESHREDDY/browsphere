//! Optional AWS integrations using an existing local AWS CLI v2 profile.
//! No AWS credentials are stored by Browsphere. All transfers are user-initiated.
use anyhow::{anyhow, bail, Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use url::Url;

use crate::router::model_router::RouterConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AwsConfig {
    pub profile: String,
    pub region: String,
    pub bedrock_model: String,
    pub bedrock_enabled: bool,
    pub s3_bucket: String,
    pub s3_key: String,
    pub dynamodb_table: String,
    pub sync_namespace: String,
    pub cognito_client_id: String,
    pub lambda_function: String,
}

impl Default for AwsConfig {
    fn default() -> Self {
        Self {
            profile: "default".into(),
            region: "us-east-1".into(),
            bedrock_model: "amazon.nova-lite-v1:0".into(),
            bedrock_enabled: false,
            s3_bucket: String::new(),
            s3_key: "browsphere/settings-backup.json".into(),
            dynamodb_table: String::new(),
            sync_namespace: "default".into(),
            cognito_client_id: String::new(),
            lambda_function: String::new(),
        }
    }
}

fn safe_identifier(s: &str, allowed: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_alphanumeric() || allowed.as_bytes().contains(&b))
}

impl AwsConfig {
    pub fn validate(&self) -> Result<()> {
        if !safe_identifier(&self.profile, "._-/", 128) ||
            !safe_identifier(&self.region, "-", 32) ||
            !safe_identifier(&self.sync_namespace, "._-", 64) {
            bail!("Invalid AWS profile, region, or sync namespace");
        }
        if !safe_identifier(&self.bedrock_model, "._-:/", 256) {
            bail!("Invalid Bedrock model ID");
        }
        if !self.s3_bucket.is_empty() && !safe_identifier(&self.s3_bucket, ".-", 63) {
            bail!("Invalid S3 bucket");
        }
        if !safe_identifier(&self.s3_key, "/._-", 512) || self.s3_key.starts_with('/') || self.s3_key.contains("..") {
            bail!("Invalid S3 backup key");
        }
        if !self.dynamodb_table.is_empty() && !safe_identifier(&self.dynamodb_table, "._-", 255) {
            bail!("Invalid DynamoDB table");
        }
        if !self.cognito_client_id.is_empty() && !safe_identifier(&self.cognito_client_id, "", 128) {
            bail!("Invalid Cognito public app client ID");
        }
        if !self.lambda_function.is_empty() && !safe_identifier(&self.lambda_function, "._-:/", 256) {
            bail!("Invalid Lambda function name");
        }
        Ok(())
    }
}

/// Never use a shell; the profile and every parameter is passed as a separate argument.
/// The AWS CLI owns authentication through its configured SSO/role/profile chain.
pub async fn aws_cli(cfg: &AwsConfig, args: &[String], input: Option<&[u8]>, seconds: u64) -> Result<Vec<u8>> {
    cfg.validate()?;
    let mut command = Command::new("aws");
    command
        .args(["--profile", cfg.profile.as_str(), "--region", cfg.region.as_str(), "--no-cli-pager", "--output", "json"])
        .args(args)
        .env("AWS_PAGER", "")
        .env("AWS_CLI_AUTO_PROMPT", "off")
        .env("AWS_EC2_METADATA_DISABLED", "true")
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if input.is_some() {
        command.stdin(std::process::Stdio::piped());
    }
    let mut child = command.spawn().context("AWS CLI not found. Install AWS CLI v2, then configure your profile")?;
    if let Some(bytes) = input {
        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("AWS CLI stdin unavailable"))?;
        stdin.write_all(bytes).await?;
        drop(stdin);
    }
    let output = tokio::time::timeout(Duration::from_secs(seconds), child.wait_with_output())
        .await.context("AWS request timed out")??;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        bail!("AWS request failed: {}", message.trim().chars().take(450).collect::<String>());
    }
    if output.stdout.len() > 2_000_000 {
        bail!("AWS response is too large");
    }
    Ok(output.stdout)
}

pub async fn test_identity(cfg: &AwsConfig) -> Result<String> {
    let data = aws_cli(cfg, &["sts".into(), "get-caller-identity".into()], None, 20).await?;
    let body: Value = serde_json::from_slice(&data)?;
    let account = body["Account"].as_str().unwrap_or("");
    let suffix: String = account.chars().rev().take(4).collect::<String>().chars().rev().collect();
    Ok(format!("AWS profile connected (account ending {}). Region: {}", suffix, cfg.region))
}

pub async fn converse(cfg: &AwsConfig, message: &str) -> Result<String> {
    converse_with_system(cfg, "", message).await
}

/// Bedrock Converse exposes a native system field. Keep voice instructions in
/// that higher-priority field, never inside user/page content.
pub async fn converse_with_system(cfg: &AwsConfig, system_prompt: &str, message: &str) -> Result<String> {
    if !cfg.bedrock_enabled { bail!("Enable Amazon Bedrock in Browsphere AWS settings first"); }
    if message.len() > 32_000 { bail!("Prompt is too long for this Bedrock route"); }
    let messages = json!([{"role":"user","content":[{"text": message}]}]);
    let mut args = vec![
        "bedrock-runtime".into(), "converse".into(),
        "--model-id".into(), cfg.bedrock_model.clone(),
        "--messages".into(), serde_json::to_string(&messages)?,
        "--inference-config".into(), r#"{"maxTokens":1024,"temperature":0.1}"#.into(),
    ];
    if !system_prompt.trim().is_empty() {
        args.push("--system".into());
        args.push(serde_json::to_string(&json!([{"text":system_prompt}]))?);
    }
    let output = aws_cli(cfg, &args, None, 65).await?;
    let parsed: Value = serde_json::from_slice(&output)?;
    let text = parsed.pointer("/output/message/content")
        .and_then(Value::as_array)
        .and_then(|parts| parts.iter().find_map(|part| part.get("text").and_then(Value::as_str)))
        .ok_or_else(|| anyhow!("Bedrock response did not contain text"))?;
    Ok(text.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryMeta {
    pub kind: String,
    pub scope: String,
    pub updated_at: String,
    pub evidence_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloudSnapshot {
    pub version: u32,
    pub exported_at: String,
    pub router: RouterConfig,
    pub memories: Vec<MemoryMeta>,
}

impl CloudSnapshot {
    pub fn new(router: RouterConfig, memories: Vec<MemoryMeta>) -> Self {
        let mut router = router;
        router.litellm_api_key = None;
        Self { version: 1, exported_at: Utc::now().to_rfc3339(), router, memories }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 { bail!("Unsupported backup version"); }
        if self.memories.len() > 1000 { bail!("Cloud memory metadata exceeds limit"); }
        if self.router.ollama_model.is_empty() || self.router.litellm_model.is_empty() {
            bail!("Cloud model configuration is invalid");
        }
        if let Some(secret) = &self.router.litellm_api_key {
            if !secret.is_empty() { bail!("Refusing to import cloud backup containing credentials"); }
        }
        Ok(())
    }
}

pub async fn s3_backup(cfg: &AwsConfig, snapshot: &CloudSnapshot) -> Result<String> {
    if cfg.s3_bucket.is_empty() { bail!("Set an S3 bucket before backing up"); }
    let bytes = serde_json::to_vec(snapshot)?;
    if bytes.len() > 150_000 { bail!("Backup exceeds 150 KB limit"); }
    let url = format!("s3://{}/{}", cfg.s3_bucket, cfg.s3_key);
    aws_cli(cfg, &["s3".into(), "cp".into(), "-".into(), url,
        "--sse".into(), "AES256".into(), "--only-show-errors".into()], Some(&bytes), 45).await?;
    Ok("Settings and non-content memory metadata backed up with S3 server-side encryption (SSE-S3).".into())
}

pub async fn s3_restore(cfg: &AwsConfig) -> Result<CloudSnapshot> {
    if cfg.s3_bucket.is_empty() { bail!("Set an S3 bucket first"); }
    let url = format!("s3://{}/{}", cfg.s3_bucket, cfg.s3_key);
    let output = aws_cli(cfg, &["s3".into(), "cp".into(), url, "-".into(),
        "--only-show-errors".into()], None, 45).await?;
    if output.len() > 150_000 { bail!("Cloud backup exceeds size limit"); }
    let snapshot: CloudSnapshot = serde_json::from_slice(&output)?;
    snapshot.validate()?;
    Ok(snapshot)
}

/// Table key schema: PK (String) and SK (String). No memory text, passwords,
/// cookies, browser history, or access tokens are sent to DynamoDB.
pub async fn dynamodb_push(cfg: &AwsConfig, snapshot: &CloudSnapshot) -> Result<String> {
    if cfg.dynamodb_table.is_empty() { bail!("Set a DynamoDB table first"); }
    let data = serde_json::to_string(snapshot)?;
    if data.len() > 150_000 { bail!("Sync payload too large"); }
    let item = json!({
        "PK": {"S": format!("BROWSPHERE#{}", cfg.sync_namespace)},
        "SK": {"S": "SETTINGS"},
        "Payload": {"S": data},
    });
    aws_cli(cfg, &["dynamodb".into(), "put-item".into(),
        "--table-name".into(), cfg.dynamodb_table.clone(),
        "--item".into(), serde_json::to_string(&item)?], None, 30).await?;
    Ok("Cloud preference snapshot and memory metadata synced to DynamoDB.".into())
}

pub async fn dynamodb_pull(cfg: &AwsConfig) -> Result<CloudSnapshot> {
    if cfg.dynamodb_table.is_empty() { bail!("Set a DynamoDB table first"); }
    let key = json!({
        "PK": {"S": format!("BROWSPHERE#{}", cfg.sync_namespace)},
        "SK": {"S": "SETTINGS"}
    });
    let body = aws_cli(cfg, &["dynamodb".into(), "get-item".into(),
        "--consistent-read".into(), "--table-name".into(), cfg.dynamodb_table.clone(),
        "--key".into(), serde_json::to_string(&key)?], None, 30).await?;
    let value: Value = serde_json::from_slice(&body)?;
    let payload = value.pointer("/Item/Payload/S").and_then(Value::as_str)
        .ok_or_else(|| anyhow!("No cloud settings exist for this sync namespace"))?;
    let snapshot: CloudSnapshot = serde_json::from_str(payload)?;
    snapshot.validate()?;
    Ok(snapshot)
}

pub async fn invoke_lambda(cfg: &AwsConfig, payload: &str) -> Result<String> {
    if cfg.lambda_function.is_empty() { bail!("Set the Lambda function name first"); }
    let parsed: Value = serde_json::from_str(payload).context("Lambda payload must be valid JSON")?;
    let payload = serde_json::to_string(&parsed)?;
    if payload.len() > 8_192 { bail!("Lambda payload must be under 8 KB"); }
    let path = std::env::temp_dir().join(format!("browsphere-lambda-{}.json", uuid::Uuid::new_v4()));
    let args = vec![
        "lambda".into(), "invoke".into(), "--function-name".into(),
        cfg.lambda_function.clone(), "--cli-binary-format".into(), "raw-in-base64-out".into(),
        "--payload".into(), payload, path.to_string_lossy().into_owned()
    ];
    let result = aws_cli(cfg, &args, None, 45).await;
    let data = tokio::fs::read(&path).await;
    let _ = tokio::fs::remove_file(&path).await;
    let metadata: Value = serde_json::from_slice(&result?)?;
    if metadata.get("FunctionError").is_some() {
        bail!("Lambda returned a function error");
    }
    let response = data.context("Could not read Lambda response file")?;
    Ok(String::from_utf8_lossy(&response).chars().take(8000).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct CognitoSession {
    pub username: String,
    pub expires_at: String,
    #[serde(skip_serializing)]
    pub access_token: String,
}

pub async fn cognito_sign_in(cfg: &AwsConfig, username: &str, password: &str) -> Result<CognitoSession> {
    cfg.validate()?;
    if cfg.cognito_client_id.is_empty() { bail!("Set a Cognito public app client ID first"); }
    if username.trim().is_empty() || password.is_empty() { bail!("Enter username and password"); }
    if username.len() > 256 || password.len() > 1024 { bail!("Credentials exceed length limits"); }
    let endpoint = format!("https://cognito-idp.{}.amazonaws.com/", cfg.region);
    let host = Url::parse(&endpoint)?;
    if host.scheme() != "https" { bail!("Cognito requires HTTPS"); }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
    let response = client.post(endpoint)
        .header("X-Amz-Target", "AWSCognitoIdentityProviderService.InitiateAuth")
        .header("Content-Type", "application/x-amz-json-1.1")
        .json(&json!({
            "AuthFlow":"USER_PASSWORD_AUTH",
            "ClientId":cfg.cognito_client_id,
            "AuthParameters":{"USERNAME":username,"PASSWORD":password}
        })).send().await.context("Cognito authentication request failed")?;
    let status = response.status();
    let body: Value = response.json().await.context("Invalid Cognito reply")?;
    if !status.is_success() {
        let message = body.get("message").or_else(||body.get("Message")).and_then(Value::as_str)
            .unwrap_or("Cognito sign-in failed");
        bail!("{}", message.chars().take(250).collect::<String>());
    }
    if let Some(challenge) = body.get("ChallengeName").and_then(Value::as_str) {
        bail!("Cognito requested {}. Use a Cognito app client without additional challenges for this sign-in flow", challenge);
    }
    let auth = &body["AuthenticationResult"];
    let token = auth["AccessToken"].as_str().ok_or_else(|| anyhow!("No Cognito access token returned"))?;
    let expires = auth["ExpiresIn"].as_i64().unwrap_or(3600).clamp(60, 86_400);
    Ok(CognitoSession {
        username: username.to_string(),
        expires_at: (Utc::now() + ChronoDuration::seconds(expires)).to_rfc3339(),
        access_token: token.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_config_rejected() {
        let mut c = AwsConfig::default();
        c.profile = "default; echo danger".into();
        assert!(c.validate().is_err());
        c.profile = "default".into();
        c.s3_key = "../sensitive".into();
        assert!(c.validate().is_err());
        c.s3_key = "browsphere/settings-backup.json".into();
        assert!(c.validate().is_ok());
    }
    #[test]
    fn snapshots_never_contain_api_keys() {
        let mut router = RouterConfig::default();
        router.litellm_api_key = Some("secret".into());
        let snapshot = CloudSnapshot::new(router, vec![]);
        assert!(!serde_json::to_string(&snapshot).unwrap().contains("secret"));
        assert!(snapshot.validate().is_ok());
    }
    #[test]
    fn incompatible_backup_rejected() {
        let mut snapshot = CloudSnapshot::new(RouterConfig::default(), vec![]);
        snapshot.version = 99;
        assert!(snapshot.validate().is_err());
    }
}
