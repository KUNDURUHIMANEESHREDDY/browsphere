/* Browsphere AI & ML preferences. Real form inputs consumed by the existing native bridge. */
(function () {
  'use strict';
  function esc(value) {
    return String(value == null ? '' : value)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }
  function input(id, value, placeholder, type) {
    return '<input class="mac-ai-input mac-ai-mono" id="' + id + '" type="' + (type || 'text') +
      '" value="' + esc(value) + '" placeholder="' + esc(placeholder || '') +
      '" spellcheck="false" autocomplete="off" />';
  }
  function row(title, description, control, id) {
    return '<div class="mac-ai-row"><div class="mac-ai-descriptor"><span class="mac-ai-label"' +
      (id ? ' id="' + id + '"' : '') + '>' + title + '</span>' +
      (description ? '<div class="mac-ai-help">' + description + '</div>' : '') +
      '</div><div class="mac-ai-controls">' + control + '</div></div>';
  }
  function section(title, note, rows) {
    return '<section class="mac-ai-group" aria-label="' + esc(title) + '"><h3 class="mac-ai-heading">' +
      esc(title) + '</h3>' + (note ? '<p class="mac-ai-group-note">' + note + '</p>' : '') +
      '<div class="mac-ai-surface">' + rows + '</div></section>';
  }
  function btn(action, label) {
    return '<button class="mac-ai-btn" type="button" data-action="' + action + '">' + label + '</button>';
  }
  // Keep Cloud Models presets in one registry shared with the settings event
  // handler. Selecting a preset never contacts the provider or spends credits.
  // 9Router serves an OpenAI-compatible API locally or from 9router.com.
  window.browsphereCloudPresets = Object.freeze({
    openai: ['https://api.openai.com/v1/chat/completions', 'gpt-4o-mini'],
    groq: ['https://api.groq.com/openai/v1/chat/completions', 'llama-3.3-70b-versatile'],
    openrouter: ['https://openrouter.ai/api/v1/chat/completions', 'inclusionai/ling-3.2-flash:free'],
    ninerouter_local: ['http://127.0.0.1:20128/v1/chat/completions', '9router/select-model'],
    ninerouter_cloud: ['https://9router.com/v1/chat/completions', '9router/select-model']
  });
  function providerOf(url) {
    try {
      var parsed = new URL(url);
      var host = parsed.hostname.toLowerCase();
      if (host === 'api.openai.com') return 'openai';
      if (host === 'api.groq.com') return 'groq';
      if (host === 'openrouter.ai') return 'openrouter';
      if (parsed.pathname === '/v1/chat/completions') {
        if (host === '9router.com' && parsed.protocol === 'https:') return 'ninerouter_cloud';
        if ((host === '127.0.0.1' || host === 'localhost') &&
            parsed.protocol === 'http:' && parsed.port === '20128') return 'ninerouter_local';
      }
    } catch (_) {}
    return 'custom';
  }
  window.renderMacAiPreferences = function (state) {
    var cfg = state.config || {};
    var aws = state.aws || {};
    var models = state.localModels || [];
    // Provider-specific catalogs are persisted in the router configuration, never API keys.
    var cloudCatalogs = cfg.cloud_model_catalogs || {};
    var cloudModels = Array.isArray(cloudCatalogs[cfg.litellm_url])
      ? cloudCatalogs[cfg.litellm_url] : [];
    var preferLocal = !!cfg.prefer_local_for_simple;
    var cloudOnly = !!cfg.lucia_cloud_only;
    var provider = providerOf(cfg.litellm_url || '');
    var providers = [
      ['custom', 'Custom / LiteLLM'], ['openai', 'OpenAI'],
      ['groq', 'Groq'], ['openrouter', 'OpenRouter'],
      ['ninerouter_local', '9Router (Local gateway)'],
      ['ninerouter_cloud', '9Router (Cloud)']
    ];
    var providerControl = '<select class="mac-ai-select" id="cfgProviderPreset" aria-label="Cloud provider">' +
      providers.map(function (item) {
        return '<option value="' + item[0] + '"' + (provider === item[0] ? ' selected' : '') +
          '>' + item[1] + '</option>';
      }).join('') + '</select>';
    var installedOptions = models.map(function (model) {
      return '<option value="' + esc(model) + '"></option>';
    }).join('');
    var localModel = '<div style="width:100%;max-width:360px">' +
      input('cfgOllamaModel', cfg.ollama_model, 'Select or enter a model ID') +
      '<datalist id="cfgInstalledOllama">' + installedOptions + '</datalist></div>';
    var is9Router = provider === 'ninerouter_local' || provider === 'ninerouter_cloud';
    var suggestions = cloudModels.length ? cloudModels :
      (is9Router ? [] : provider === 'openrouter'
        ? ['inclusionai/ling-3.2-flash:free']
        : ['gpt-4o-mini', 'gpt-4.1-mini', 'llama-3.3-70b-versatile', 'openai/gpt-4o-mini']);
    var cloudModel = '<div style="width:100%;max-width:360px">' +
      input('cfgLiteLlmModel', cfg.litellm_model === '9router/select-model' ? '' : cfg.litellm_model,
        is9Router ? 'Fetch Models, then choose a model' : 'Choose a model ID') +
      '<datalist id="cfgCloudModels">' + suggestions.map(function (id) {
        return '<option value="' + esc(id) + '"></option>';
      }).join('') + '</datalist>' +
      '<div class="mac-ai-model-hint">' +
      (cloudModels.length ? cloudModels.length + ' models fetched for this provider' :
        is9Router ? 'No model is selected yet. Click Fetch Models, then explicitly choose a model. Some combos may use paid providers.' :
        'Enter a model ID or fetch the provider\'s available models below.') +
      '</div></div>';
    // Link custom model inputs to their suggestions without hiding user-entered IDs.
    localModel = localModel.replace('id="cfgOllamaModel"', 'id="cfgOllamaModel" list="cfgInstalledOllama"');
    cloudModel = cloudModel.replace('id="cfgLiteLlmModel"', 'id="cfgLiteLlmModel" list="cfgCloudModels"');
    var engineControl = '<div class="mac-ai-segmented" role="group" aria-label="Preferred AI engine">' +
      '<button type="button" data-action="set-ai-preferred" data-engine="ollama" aria-pressed="' +
      preferLocal + '">Local</button>' +
      '<button type="button" data-action="set-ai-preferred" data-engine="litellm" aria-pressed="' +
      !preferLocal + '">Cloud</button>' +
      '<input type="checkbox" id="cfgPreferLocal" hidden tabindex="-1" ' +
      (preferLocal ? 'checked' : '') + ' /></div>';
    var luciaCloudOnly = '<label class="mac-ai-switch" aria-label="Use only cloud models for Lucia browser tasks">' +
      '<input id="cfgLuciaCloudOnly" type="checkbox"' + (cloudOnly ? ' checked' : '') +
      ' /><span class="track"><span class="thumb"></span></span></label>';
    var primary = section('Default Intelligence', 'Choose which model Browsphere uses for new agent tasks.',
      row('Preferred engine', 'Use local processing or your configured cloud API.', engineControl) +
      row('Lucia cloud-only reasoning', 'Use the selected cloud provider for planning, browser actions and verification. No silent Ollama fallback. Webpage excerpts and task requests are sent to the provider; API usage may be billed. Save a provider key and test cloud connection first.', luciaCloudOnly) +
      row('Local model', 'Ollama model used for local tasks.', localModel) +
      row('Cloud provider', 'OpenAI-compatible API providers.', providerControl) +
      row('Cloud model', 'Enter a supported model ID.', cloudModel));
    var chips = models.length
      ? '<div class="mac-ai-chip-list">' + models.map(function (model) {
          return '<button type="button" class="mac-ai-chip" data-action="choose-ai-local-model" data-model="' +
            esc(model) + '" title="Use this installed model">' + esc(model) + '</button>';
        }).join('') + '</div>'
      : '<span class="mac-ai-quiet">' + esc(state.localModelsError || 'No models detected.') + '</span>';
    var local = section('Local Models', '',
      row('Ollama server', 'Local API address.', input('cfgOllamaUrl', cfg.ollama_url, 'http://127.0.0.1:11434', 'url')) +
      row('Lucia vision model (Eyes)', 'Optional local visual perception on recovery. Leave empty to disable. Screenshots stay with local Ollama; visual hints are advisory.', input('cfgVisionModel', cfg.vision_model || '', 'lfm3-vl:2b')) +
      row('Installed models', 'Select an installed model to use it.',
        '<div class="mac-ai-installed-controls">' + chips + btn('refresh-ai-models', 'Refresh') + '</div>') +
      row('Connection', 'Send a small prompt to Ollama.', btn('test-local-model', 'Test local connection')) +
      row('Lucia fallback', 'Use an installed local model when cloud free-model access is unavailable. This changes Lucia to Local, without removing your cloud provider settings.',
        models.length ? btn('use-local-lucia-fallback', 'Use local model for Lucia') :
          '<span class="mac-ai-quiet">Install or refresh an Ollama model first.</span>'));
    var keyStatus = state.hasKey
      ? '<span class="mac-ai-security ready">● Key saved in Windows Credential Manager</span>'
      : '<span class="mac-ai-security empty">○ No API key saved for this endpoint</span>';
    var keyControl = '<div style="width:100%;max-width:360px">' + keyStatus +
      '<div class="mac-ai-inline">' +
      '<input class="mac-ai-input mac-ai-mono" id="cfgLiteLlmKey" type="password"' +
      ' spellcheck="false" autocomplete="new-password" placeholder="' +
      (state.hasKey ? 'Enter a key to replace the saved key' : 'Paste your provider API key') + '" />' +
      '<button class="mac-ai-btn mac-ai-key-btn" type="button" data-action="toggle-ai-key-visibility" aria-label="Show or hide API key" aria-pressed="false">Show</button></div>' +
      '<span class="mac-ai-lock-note">Stored securely on this computer. Never shown again after saving.</span></div>';
    var cloudSelector = '<div class="mac-ai-fetched-select">' +
      '<select id="cfgFetchedCloudModels" class="mac-ai-select" aria-label="Choose a fetched model"' +
      (!cloudModels.length ? ' disabled' : '') + '>' +
      '<option value="">' + (cloudModels.length ? 'Choose a fetched model…' : 'No models fetched yet') + '</option>' +
      cloudModels.map(function (id) {
        return '<option value="' + esc(id) + '"' +
          (id === cfg.litellm_model ? ' selected' : '') + '>' + esc(id) + '</option>';
      }).join('') + '</select>' +
      (cloudModels.length ? '<span class="mac-ai-fetch-count">' +
        cloudModels.length + ' saved for this endpoint</span>' : '') +
      '</div>';
    var cloud = section('Cloud Models', '',
      row('API endpoint', '9Router local gateway: http://127.0.0.1:20128/v1/chat/completions. Hosted 9Router: https://9router.com/v1/chat/completions. Other remote providers require HTTPS.', input('cfgLiteLlmUrl', cfg.litellm_url, 'https://api.example.com/v1/chat/completions', 'url')) +
      row('API key', 'Credentials are kept in Windows Credential Manager, not the settings database.', keyControl) +
      row('Fetch models', 'Retrieve model IDs from this provider using the endpoint and saved key.',
        '<div class="mac-ai-fetch-controls">' +
        btn('fetch-cloud-models', 'Fetch Models') +
        '<span id="cfgFetchModelsStatus" class="mac-ai-fetch-note" role="status" aria-live="polite">' +
        esc(state.fetchStatus || (cloudModels.length ? 'Saved models are available below.' : 'Ready to fetch')) +
        '</span></div>') +
      row('Available models', 'Choose a fetched model; your selection is saved.', cloudSelector) +
      row('Connection', 'Test the selected cloud model and key.', btn('test-cloud-model', 'Test cloud connection')) +
      row('Lucia model readiness', 'Send one small API request to confirm that the chosen cloud model generates a valid browser-action JSON response. Provider charges may apply; no webpage is opened.', btn('test-cloud-agent', 'Test Lucia reasoning')));
    var awsDiscovered = Array.isArray(state.awsServices) ? state.awsServices : [];
    var awsServiceSummary = awsDiscovered.length
      ? '<div class="mac-aws-services" aria-label="Discovered AWS services">' +
        awsDiscovered.map(function (entry) {
          var status = entry.status === 'ready' ? 'ready' :
            entry.status === 'available' ? 'available' : 'missing';
          return '<div class="mac-aws-service"><span class="mac-aws-service-indicator ' + status +
            '" aria-hidden="true"></span><span class="mac-aws-service-name">' + esc(entry.service) +
            '</span><span class="mac-aws-service-detail">' + esc(entry.detail) + '</span></div>';
        }).join('') + '</div>'
      : '<span class="mac-ai-quiet">Connect to find your existing resources. Nothing is provisioned automatically.</span>';
    var awsActionLabel = state.awsConnecting ? 'Connecting…' :
      (state.awsAccountHint ? 'Refresh connection' : 'Connect AWS');
    var awsAction = '<div class="mac-aws-connect-action">' +
      '<button class="mac-ai-btn primary" data-action="aws-connect" type="button"' +
      (state.awsConnecting ? ' disabled' : '') + '>' + awsActionLabel + '</button>' +
      '<span class="mac-ai-quiet">Uses your system browser. No API keys to paste.</span></div>';
    var awsAdvanced = '<details class="mac-aws-advanced"><summary>Advanced account settings</summary>' +
      '<div class="mac-ai-surface">' +
      row('AWS profile', 'Existing AWS CLI profile / identity.', input('cfgAwsProfile', aws.profile || 'default', 'default')) +
      row('Region', 'Defaults to the selected AWS profile region.', input('cfgAwsRegion', aws.region || 'us-east-1', 'us-east-1')) +
      row('Connection check', 'Verify access without changing saved resources.', btn('aws-test', 'Test profile')) +
      '</div></details>';
    var awsConnection = section('Connect AWS', 'Sign in once through AWS, automatically discover resources you can access, and save only resource IDs locally. Existing resources are never modified.',
      row('AWS account', 'Secure browser sign-in and read-only resource discovery.', awsAction) +
      row('Connection', 'No AWS access keys are stored in Browsphere.',
        '<span id="cfgAwsStatus" class="mac-aws-connection-status" role="status" aria-live="polite">' +
        esc(state.awsStatus || (state.awsAccountHint ? state.awsAccountHint : 'Not verified in this session')) + '</span>') +
      row('Resources', 'Only compatible Browsphere resources are selected.', awsServiceSummary) +
      awsAdvanced);

    var bedrock = section('Amazon Bedrock', 'Uses the Converse API for cloud intelligence. Model inference may incur AWS charges.',
      row('Bedrock model ID', 'Use a model enabled for your account and region.', input('cfgAwsModel', aws.bedrock_model || 'amazon.nova-lite-v1:0', 'Bedrock model ID')) +
      row('Cloud AI routing', 'When enabled, Bedrock replaces LiteLLM for cloud agent requests.',
        '<label class="mac-ai-switch" aria-label="Use Amazon Bedrock for cloud AI"><input id="cfgAwsBedrockEnabled" type="checkbox"' +
        (aws.bedrock_enabled ? ' checked' : '') + ' /><span class="track"><span class="thumb"></span></span></label>'));

    var s3 = section('Amazon S3', 'Manual, opt-in backup of AI settings and non-content memory metadata only. SSE-S3 encrypts the object at rest. No history, passwords, API keys or memory text is uploaded.',
      row('Bucket name', 'Your existing private S3 bucket.', input('cfgAwsBucket', aws.s3_bucket || '', 'my-private-bucket')) +
      row('Backup object key', 'Object path inside the bucket.', input('cfgAwsKey', aws.s3_key || 'browsphere/settings-backup.json', 'browsphere/settings-backup.json')) +
      row('Backup & restore', 'Restore replaces local model preferences, never memories or secrets.',
        btn('aws-s3-backup', 'Backup now') + ' ' + btn('aws-s3-restore', 'Restore settings')));

    var ddb = section('Amazon DynamoDB', 'Manual sync of preferences and memory metadata (no memory text). Your table must use String partition key PK and String sort key SK.',
      row('Table name', 'Existing DynamoDB table.', input('cfgAwsTable', aws.dynamodb_table || '', 'BrowsphereSync')) +
      row('Sync namespace', 'Separates your data from other users of the AWS account.', input('cfgAwsNamespace', aws.sync_namespace || 'default', 'private-id')) +
      row('Sync', 'Pull replaces local model preferences only.',
        btn('aws-dynamo-push', 'Push settings') + ' ' + btn('aws-dynamo-pull', 'Pull settings')));

    var signedIn = state.awsSession && state.awsSession.username
      ? 'Signed in as ' + esc(state.awsSession.username) + ' (until ' + esc(state.awsSession.expires_at || '') + ')'
      : 'Not signed in. This login is separate from AWS CLI profile permissions.';
    var cognito = section('Amazon Cognito', 'Optional user sign-in with a public Cognito app client that allows USER_PASSWORD_AUTH. Tokens remain in app memory and are not backed up.',
      row('App client ID', 'Cognito user pool public app client; no client secret.', input('cfgAwsCognitoClient', aws.cognito_client_id || '', 'Public app client ID')) +
      row('Username', '', input('cfgAwsCognitoUser', '', 'Email or username')) +
      row('Password', '', input('cfgAwsCognitoPass', '', 'Password', 'password')) +
      row('Session', signedIn,
        btn('aws-cognito-login', 'Sign in') + ' ' + btn('aws-cognito-logout', 'Sign out')));

    var lambda = section('AWS Lambda', 'Optional manual function invocation. Can trigger remote actions or AWS charges; review the JSON before executing.',
      row('Function name / ARN', 'Existing Lambda function.', input('cfgAwsLambda', aws.lambda_function || '', 'BrowsphereWorker')) +
      row('JSON payload', 'Only send information you intend to share with the function.',
        '<textarea class="mac-ai-input mac-ai-mono" id="cfgAwsLambdaPayload" rows="2" spellcheck="false" style="width:100%;max-width:360px">{}</textarea>') +
      row('Invoke', 'Explicit confirmation required.', btn('aws-lambda-invoke', 'Invoke function')));

    var awsSave = '<div class="mac-ai-footer"><span class="mac-ai-quiet">AWS sync and invocation only run when you select an action.</span>' +
      '<button type="button" class="mac-ai-btn primary" data-action="save-aws-settings">Save AWS settings</button></div>';
    var ruleEntries = Object.keys(cfg.domain_overrides || {});
    var rules = ruleEntries.length
      ? '<div class="mac-ai-chip-list">' + ruleEntries.map(function (domain) {
          return '<span class="mac-ai-chip"><span>' + esc(domain) + ' → ' +
            esc(cfg.domain_overrides[domain] === 'ollama' ? 'Local' : 'Cloud') +
            '</span><button type="button" data-action="remove-domain-rule" data-domain="' +
            esc(domain) + '" aria-label="Remove rule for ' + esc(domain) + '">×</button></span>';
        }).join('') + '</div>'
      : '<span class="mac-ai-quiet">No overrides configured</span>';
    var routeEntry = '<div class="mac-ai-routing-entry">' +
      '<input class="mac-ai-input" id="cfgRuleDomain" type="text" aria-label="Website domain" placeholder="example.com" />' +
      '<select class="mac-ai-select" id="cfgRuleBackend" aria-label="Backend for domain">' +
      '<option value="ollama">Local</option><option value="litellm">Cloud</option></select>' +
      '<button class="mac-ai-btn" type="button" data-action="add-domain-rule">Add</button></div>';
    var preferSwitch = '<label class="mac-ai-switch" aria-label="Prefer local AI for lightweight tasks">' +
      '<input type="checkbox" id="cfgPreferLocalToggle" ' + (preferLocal ? 'checked' : '') + ' />' +
      '<span class="track"><span class="thumb"></span></span></label>';
    var routing = section('Routing', '',
      row('Prefer local AI', 'Use Ollama as the default engine. Website rules can override it.', preferSwitch) +
      row('Add website override', 'Route a specific domain to Local or Cloud.', routeEntry) +
      row('Current overrides', '', rules));
    var testStatus = state.testStatus ||
      'No connection tested yet. Select a test button to check the model.';
    var statusState = /^Connected:/.test(testStatus) ? 'success' :
      /^Connection failed:/.test(testStatus) ? 'error' :
      /^Contacting/.test(testStatus) ? 'testing' : 'idle';
    var diagnostic = '<section class="mac-ai-group" aria-label="Diagnostics">' +
      '<h3 class="mac-ai-heading">Diagnostics</h3>' +
      '<div class="mac-ai-status" id="cfgConnectionIndicator" data-state="' + statusState + '">' +
      '<span class="mac-ai-status-dot"></span>' +
      '<span class="mac-ai-status-text" id="cfgConnectionStatus" role="status" aria-live="polite">' +
      esc(testStatus) + '</span></div></section>';
    var localLogs = section('Local Logs', 'Automatically captures browser events, Lucia agent traces, model status, errors, and crashes. No messages, credentials, or private browsing activity are stored.',
      row('Diagnostics files', 'Rotating JSONL logs stay on this device for up to 14 days.',
        btn('open-diagnostics', 'Open Logs') + ' ' + btn('export-diagnostics', 'Export Diagnostics')));
    var footer = '<div class="mac-ai-footer">' +
      '<button class="mac-ai-danger" type="button" data-action="remove-model-key">Remove Saved Key</button>' +
      '<div class="mac-ai-footer-actions">' +
      '<button class="mac-ai-btn" type="button" data-action="test-model-connection">Test Default</button>' +
      '<button class="mac-ai-btn primary" type="button" data-action="save-router-settings">Save</button>' +
      '</div></div>';
    return '<div class="mac-ai" id="macAiSettings">' +
      '<header class="mac-ai-intro"><h2>AI &amp; ML</h2>' +
      '<p>Configure the models Browsphere uses for local and cloud intelligence.</p></header>' +
      awsConnection + primary + local + cloud + routing + diagnostic + localLogs +
      (window.LuciaDiagnosticsDashboard ? window.LuciaDiagnosticsDashboard.render() : '') + footer +
      '<details class="mac-aws-advanced"><summary>Advanced AWS service settings</summary>' +
      bedrock + s3 + ddb + cognito + lambda + awsSave + '</details></div>';
  };
})();