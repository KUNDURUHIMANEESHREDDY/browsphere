# Browsphere AWS integration

AWS is optional and disabled by default. Browsphere remains local-first. The integration uses AWS CLI v2 for Bedrock, S3, DynamoDB, and Lambda and a direct HTTPS Cognito API request for user sign-in. It does not create AWS resources or store AWS access keys.

## Configure

1. Set up an AWS CLI v2 profile with least-privilege, short-lived credentials (prefer SSO or an AWS role); this is managed by the AWS CLI, not Browsphere.
2. In Browsphere open Settings > AI & ML > Amazon Web Services.
3. Enter the profile and region. Save AWS settings, then select Test AWS profile.
4. Add existing resources only for services you actually need. Each service can incur AWS charges.

## Bedrock

Provide a regional model ID with Bedrock Converse access and enable the Bedrock Cloud AI switch. Select Cloud in Browsphere's AI router. The model is used for agent planning; local Ollama is still the default. Bedrock receives the prompt and browsing context when selected. The CLI process arguments can temporarily expose prompt text to other processes on the computer, so do not enable Bedrock for sensitive browsing content.

## S3 backup

Provide a private S3 bucket and backup object key. Backup now uploads only nonsecret router preferences (including domain routing rules) and limited memory metadata. It does not upload saved passwords, access keys, browser history, browsing cookies, screenshots, memory contents or the complete database.

Uploads use TLS in transit and S3-managed server-side encryption (SSE-S3 / AES256) at rest. This is NOT end-to-end or client-side encryption: anyone with suitable S3 permissions can read the object. Restore settings replaces only model preferences, after confirmation. Required access is limited to the chosen S3 prefix.

## DynamoDB sync

Create a DynamoDB table with PK as a String partition key and SK as a String sort key. Set your table name and a unique sync namespace. Browsphere stores an item under partition key BROWSPHERE#<namespace> and sort key SETTINGS. Push and Pull are manual and require confirmation; Pull replaces local model preferences only. Only preferences and memory metadata are synced; memory contents remain local.

## Cognito sign-in

Provide a public Cognito app-client ID in the configured region; the app client must permit USER_PASSWORD_AUTH. Browsphere sends the username/password directly to Cognito over HTTPS and stores the access token in Rust application memory only (not returned to web UI, not written to disk). Signing out discards this local token. Login is separate from the AWS CLI profile and does not authorize access to AWS services. Advanced challenges including MFA, hosted UI, token refresh and password recovery are not implemented yet; use an organization's normal Cognito login if those are required.

## Lambda

Enter an existing Lambda function name or ARN. Supply a JSON payload and confirm invocation. Lambda is never run automatically. Use permissions scoped to that particular function; invocation and any downstream actions may incur charges.

## Security and limitations

- No AWS account, buckets, tables, functions or Cognito user pools are created by the app.
- No cloud action runs automatically. Routing to Bedrock requires enabling it.
- AWS CLI subprocesses use separately validated arguments without a shell.
- Backups intentionally exclude memory content, browsing data and secret credentials.
- Provider keys remain stored in Windows Credential Manager; AWS tokens are managed by the CLI or kept only in memory for Cognito.
- Cloud backup and sync are opt-in and do not conflict with existing SQLite persistence.
- Live cloud verification requires an authorized AWS account and configured resources. Automated checks are local and do not incur AWS costs.


## One-click AWS connection (recommended)

Open Settings → AI & ML → Connect AWS, then select Connect AWS.

- Browsphere reuses the selected AWS CLI v2 profile (default unless changed in Advanced account settings).
- If the profile's session is already valid, it reuses the session. Otherwise it launches AWS CLI login and opens the official system browser. The user completes browser authentication. Browsphere does not capture or store passwords, OAuth callback codes, long-lived keys, or AWS session tokens.
- After authentication, Browsphere discovers existing private/encrypted Browsphere S3 buckets, DynamoDB tables with PK and SK string keys, public BrowsphereDesktop Cognito clients, Browsphere Lambda functions, and available Bedrock model IDs.
- Discovery uses read-only operations. The app saves resource identifiers, region, and the selected profile to local SQLite. It does not create cloud resources, automatically upload data, invoke Lambda, register customers, or enable Bedrock inference.
- Bedrock remains explicitly off after connecting. Local Ollama remains the preferred route unless separately changed by the user.
- Refresh connection repeats discovery. Unsupported/missing resources are reported in the UI.
- Advanced account settings allows choosing another AWS profile. Advanced AWS service settings retains manual configuration for resources with different naming conventions.

The AWS CLI sign-in authorizes a developer's AWS role. It does not register end users of Browsphere or replace customer Cognito login. Each developer device needs its own authorized profile and should never distribute that profile's credentials to other users. AWS resources may incur charges.
