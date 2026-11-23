//! Interactive, keyless AWS CLI sign-in and read-only resource discovery.
//! This module does not provision, invoke, sync, upload, or enable Bedrock.
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use crate::aws::{aws_cli, AwsConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsServiceDiscovery {
    pub service: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConnectResult {
    pub config: AwsConfig,
    pub account_hint: String,
    pub region: String,
    pub services: Vec<AwsServiceDiscovery>,
}

fn status(service: &str, state: &str, detail: impl Into<String>) -> AwsServiceDiscovery {
    AwsServiceDiscovery {
        service: service.into(),
        status: state.into(),
        detail: detail.into(),
    }
}

async fn try_json(cfg: &AwsConfig, args: &[&str]) -> Result<Value> {
    let args: Vec<String> = args.iter().map(|v| (*v).to_string()).collect();
    let out = aws_cli(cfg, &args, None, 22).await?;
    Ok(serde_json::from_slice(&out)?)
}

fn preferred_name(candidates: Vec<String>, current: &str, preferred: &str) -> Option<String> {
    if !current.is_empty() && candidates.iter().any(|s| s == current) {
        return Some(current.into());
    }
    candidates.iter().find(|name| *name == preferred)
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

fn names(v: &Value, pointer: &str, field: &str) -> Vec<String> {
    v.pointer(pointer).and_then(Value::as_array).map(|list| {
        list.iter()
            .filter_map(|entry| entry.get(field).and_then(Value::as_str))
            .map(ToOwned::to_owned).collect()
    }).unwrap_or_default()
}

fn cli_region(profile: &str) -> Result<Option<String>> {
    // "aws configure get" reads the local AWS profile; it never makes a cloud request.
    let output = std::process::Command::new("aws")
        .args(["configure", "get", "region", "--profile", profile])
        .env("AWS_PAGER", "")
        .stdin(Stdio::null())
        .output()
        .context("AWS CLI v2 is not installed")?;
    if !output.status.success() {
        return Ok(None);
    }
    let region = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let valid = !region.is_empty() && region.len() <= 32 &&
        region.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
    Ok(valid.then_some(region))
}

async fn browser_login(cfg: &AwsConfig) -> Result<()> {
    cfg.validate()?;
    // aws login opens the system browser and handles its OAuth callback.
    // Never return raw CLI stdout/stderr: they can contain temporary auth URLs.
    let child = Command::new("aws")
        .args(["login", "--profile", cfg.profile.as_str()])
        .env("AWS_PAGER", "")
        .env("AWS_CLI_AUTO_PROMPT", "off")
        .env("AWS_EC2_METADATA_DISABLED", "true")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("AWS CLI is unavailable. Install AWS CLI v2 first")?;
    let output = tokio::time::timeout(Duration::from_secs(180), child.wait_with_output())
        .await.context("AWS sign-in timed out; use the AWS CLI to sign in and reconnect")??;
    if !output.status.success() {
        bail!("AWS browser sign-in did not finish. Run 'aws login --profile {}' in your own terminal and try Connect AWS again.", cfg.profile);
    }
    Ok(())
}

async fn discover_s3(cfg: &mut AwsConfig) -> AwsServiceDiscovery {
    let all = match try_json(cfg, &["s3api", "list-buckets"]).await {
        Ok(v) => v,
        Err(_) => return status("S3", "unavailable", "Cannot list S3 buckets with this AWS role"),
    };
    let candidates: Vec<String> = names(&all, "/Buckets", "Name")
        .into_iter().filter(|name| name.starts_with("browsphere-")).collect();
    // A suitable bucket must actually block public access and encrypt data at rest.
    for name in candidates.iter().filter(|n| *n == &cfg.s3_bucket)
        .chain(candidates.iter().filter(|n| *n != &cfg.s3_bucket)) {
        let public = try_json(cfg, &["s3api", "get-public-access-block", "--bucket", name]).await;
        let encryption = try_json(cfg, &["s3api", "get-bucket-encryption", "--bucket", name]).await;
        let protected = public.ok()
            .and_then(|v| v.get("PublicAccessBlockConfiguration").cloned())
            .map(|v| ["BlockPublicAcls", "IgnorePublicAcls", "BlockPublicPolicy", "RestrictPublicBuckets"]
                .iter().all(|key| v.get(key).and_then(Value::as_bool) == Some(true)))
            .unwrap_or(false);
        let encrypted = encryption.ok()
            .and_then(|v| v.pointer("/ServerSideEncryptionConfiguration/Rules/0/ApplyServerSideEncryptionByDefault/SSEAlgorithm")
                .and_then(Value::as_str).map(str::to_owned))
            .map(|s| s == "AES256" || s == "aws:kms")
            .unwrap_or(false);
        if protected && encrypted {
            cfg.s3_bucket = name.clone();
            return status("S3", "ready", format!("Private encrypted bucket: {}", name));
        }
    }
    status("S3", "needs-setup", "No suitable private, encrypted Browsphere S3 bucket found")
}

async fn discover_dynamodb(cfg: &mut AwsConfig) -> AwsServiceDiscovery {
    let data = match try_json(cfg, &["dynamodb", "list-tables"]).await {
        Ok(value) => value,
        Err(_) => return status("DynamoDB", "unavailable", "Cannot list DynamoDB tables with this AWS role"),
    };
    let mut candidates: Vec<String> = data.pointer("/TableNames").and_then(Value::as_array)
        .into_iter().flatten().filter_map(Value::as_str)
        .filter(|name| name.starts_with("Browsphere")).map(str::to_owned).collect();
    candidates.sort_by_key(|name| if *name == cfg.dynamodb_table { 0 } else if name == "BrowsphereSync" { 1 } else { 2 });
    for name in candidates {
        if let Ok(detail) = try_json(cfg, &["dynamodb", "describe-table", "--table-name", &name]).await {
            let table = &detail["Table"];
            let schema = table["KeySchema"].as_array();
            let attrs = table["AttributeDefinitions"].as_array();
            let has_key = |key: &str, kind: &str| {
                schema.is_some_and(|items| items.iter().any(|entry| entry["AttributeName"] == key && entry["KeyType"] == kind)) &&
                attrs.is_some_and(|items| items.iter().any(|entry| entry["AttributeName"] == key && entry["AttributeType"] == "S"))
            };
            if table["TableStatus"] == "ACTIVE" && has_key("PK", "HASH") && has_key("SK", "RANGE") {
                cfg.dynamodb_table = name.clone();
                return status("DynamoDB", "ready", format!("PK/SK table: {}", name));
            }
        }
    }
    status("DynamoDB", "needs-setup", "No compatible Browsphere DynamoDB table found")
}

async fn discover_lambda(cfg: &mut AwsConfig) -> AwsServiceDiscovery {
    let data = match try_json(cfg, &["lambda", "list-functions", "--max-items", "50"]).await {
        Ok(value) => value,
        Err(_) => return status("Lambda", "unavailable", "Cannot list Lambda functions with this AWS role"),
    };
    let candidates = names(&data, "/Functions", "FunctionName")
        .into_iter().filter(|name| name.starts_with("Browsphere")).collect();
    if let Some(name) = preferred_name(candidates, &cfg.lambda_function, "BrowsphereWorker") {
        cfg.lambda_function = name.clone();
        return status("Lambda", "ready", format!("Function found (not invoked): {}", name));
    }
    status("Lambda", "needs-setup", "No Browsphere Lambda function found")
}

async fn discover_cognito(cfg: &mut AwsConfig) -> AwsServiceDiscovery {
    let pools = match try_json(cfg, &["cognito-idp", "list-user-pools", "--max-results", "60"]).await {
        Ok(value) => value,
        Err(_) => return status("Cognito", "unavailable", "Cannot list Cognito user pools with this AWS role"),
    };
    let mut pool_ids: Vec<(String, String)> = pools.pointer("/UserPools").and_then(Value::as_array)
        .into_iter().flatten()
        .filter_map(|entry| Some((entry["Name"].as_str()?.to_owned(), entry["Id"].as_str()?.to_owned())))
        .filter(|(name, _)| name.starts_with("Browsphere")).collect();
    pool_ids.sort_by_key(|(name, _)| if name == "BrowsphereUsers" { 0 } else { 1 });
    for (name, pool_id) in pool_ids {
        let clients = try_json(cfg, &["cognito-idp", "list-user-pool-clients", "--user-pool-id", &pool_id, "--max-results", "60"]).await;
        let Ok(clients) = clients else { continue };
        for client in clients.pointer("/UserPoolClients").and_then(Value::as_array).into_iter().flatten() {
            if client["ClientName"].as_str() != Some("BrowsphereDesktop") { continue }
            let Some(client_id) = client["ClientId"].as_str() else { continue };
            let full = try_json(cfg, &["cognito-idp", "describe-user-pool-client",
                "--user-pool-id", &pool_id, "--client-id", client_id]).await;
            if let Ok(full) = full {
                let info = &full["UserPoolClient"];
                let no_secret = info.get("ClientSecret").and_then(Value::as_str).is_none();
                let password_auth = info["ExplicitAuthFlows"].as_array()
                    .is_some_and(|flows| flows.iter().any(|f| f == "ALLOW_USER_PASSWORD_AUTH"));
                if no_secret && password_auth {
                    cfg.cognito_client_id = client_id.to_owned();
                    return status("Cognito", "ready", format!("Public desktop app client in {}", name));
                }
            }
        }
    }
    status("Cognito", "needs-setup", "No compatible public Browsphere Cognito app client found")
}

async fn discover_bedrock(cfg: &mut AwsConfig) -> AwsServiceDiscovery {
    // Model listings are metadata. Never perform inference or enable a paid route here.
    let data = match try_json(cfg, &["bedrock", "list-foundation-models", "--by-output-modality", "TEXT"]).await {
        Ok(value) => value,
        Err(_) => return status("Bedrock", "unavailable", "Cannot list Bedrock models with this AWS role"),
    };
    let models: Vec<String> = data.pointer("/modelSummaries").and_then(Value::as_array)
        .into_iter().flatten()
        .filter(|entry| entry.pointer("/modelLifecycle/status").and_then(Value::as_str) == Some("ACTIVE"))
        .filter_map(|entry| entry.get("modelId").and_then(Value::as_str).map(str::to_owned))
        .collect();
    for id in [cfg.bedrock_model.clone(), "amazon.nova-lite-v1:0".into(), "amazon.nova-2-lite-v1:0".into()] {
        if models.iter().any(|v| v == &id) {
            cfg.bedrock_model = id.clone();
            return status("Bedrock", "available", format!("Model listed: {} (quota/inference not tested; remains off)", id));
        }
    }
    status("Bedrock", "needs-setup", "No supported Bedrock model detected in this region")
}

/// Connect uses the existing profile (if valid) or opens the official AWS login.
/// Discovery is strictly read-only; configuration persistence belongs to the Tauri command.
pub async fn connect(mut cfg: AwsConfig, profile: &str) -> Result<AwsConnectResult> {
    let previous_profile = cfg.profile.clone();
    let previous_region = cfg.region.clone();
    cfg.profile = profile.trim().to_owned();
    cfg.validate()?;
    if let Some(region) = cli_region(&cfg.profile)? {
        cfg.region = region;
    }
    cfg.validate()?;
    // Never keep resource identifiers from a different AWS account context.
    if previous_profile != cfg.profile || previous_region != cfg.region {
        cfg.s3_bucket.clear();
        cfg.dynamodb_table.clear();
        cfg.cognito_client_id.clear();
        cfg.lambda_function.clear();
    }
    let mut auth = try_json(&cfg, &["sts", "get-caller-identity"]).await;
    if auth.is_err() {
        browser_login(&cfg).await?;
        auth = try_json(&cfg, &["sts", "get-caller-identity"]).await;
    }
    let identity = auth.context("AWS login did not produce a usable profile. Confirm your access in AWS CLI")?;
    let account = identity["Account"].as_str().ok_or_else(|| anyhow!("AWS identity response lacked an account"))?;
    let suffix: String = account.chars().rev().take(4).collect::<String>().chars().rev().collect();

    // Do not switch preferred AI routing, start cloud backups, or change data on AWS.
    cfg.bedrock_enabled = false;
    let mut services = Vec::new();
    services.push(discover_s3(&mut cfg).await);
    services.push(discover_dynamodb(&mut cfg).await);
    services.push(discover_cognito(&mut cfg).await);
    services.push(discover_lambda(&mut cfg).await);
    services.push(discover_bedrock(&mut cfg).await);
    cfg.validate()?;
    Ok(AwsConnectResult {
        config: cfg.clone(),
        account_hint: format!("AWS account ending {}", suffix),
        region: cfg.region.clone(),
        services,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picks_only_a_known_name_without_making_one_up() {
        let names = vec!["BrowsphereSync".into(), "Other".into()];
        assert_eq!(preferred_name(names.clone(), "Other", "BrowsphereSync"), Some("Other".into()));
        assert_eq!(preferred_name(names, "", "BrowsphereSync"), Some("BrowsphereSync".into()));
        assert_eq!(preferred_name(vec![], "", "any"), None);
    }
    #[test]
    fn parses_resource_names_without_missing_fields() {
        let v = serde_json::json!({"Buckets":[{"Name":"browsphere-x"}, {}, {"Name":"another"}]});
        assert_eq!(names(&v, "/Buckets", "Name"), vec!["browsphere-x", "another"]);
        assert!(names(&v, "/UserPools", "Name").is_empty());
    }
    #[test]
    fn no_sensitive_keys_in_connection_result() {
        let result = AwsConnectResult {
            config: AwsConfig::default(), account_hint: "AWS account ending 9999".into(),
            region: "us-east-1".into(), services: vec![],
        };
        let rendered = serde_json::to_string(&result).unwrap();
        assert!(!rendered.contains("AccessKeyId"));
        assert!(!rendered.contains("SecretAccessKey"));
    }
}
