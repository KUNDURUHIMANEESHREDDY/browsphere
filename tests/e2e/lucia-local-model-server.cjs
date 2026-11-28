'use strict';
const {reviewFixtureEvidence} = require('./semantic-review-fixture.cjs');
const {closeObservedDetailsDialog,escapeObservedKeyboardDialog} = require('./fixture-modal-action.cjs');
// A loopback-only, deterministic model simulator for native Windows integration
// testing. It never accesses public sites and never substitutes for real-LLM tests.
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');

const bind = '127.0.0.1';
const fixturePort = Number(process.env.LUCIA_FIXTURE_PORT || 18764);
const modelPort = Number(process.env.LUCIA_MODEL_PORT || 18766);
const fixtureRoot = path.join(__dirname, 'fixture');
const MAX_BODY = 120000;
// Count request categories only. Never retain prompts, page text, model
// responses, authorization headers, or other credentials in diagnostics.
const providerStats = {planner:0,actions:0,verifier:0,other:0,ollama:0};

function cloudPhase(messages) {
  const system = String((messages || []).find(m=>m.role==='system')?.content || '').toLowerCase();
  if (system.includes('autonomous browser task planner')) return 'planner';
  if (system.includes('independent, read-only evidence-review specialist') ||
      system.includes('check whether these source-backed findings answer the user')) return 'verifier';
  if (system.includes('browser agent working under lucia') ||
      system.includes('you propose exactly one browser action for lucia')) return 'actions';
  return 'other';
}


function send(res, status, value, contentType = 'application/json; charset=utf-8') {
  const payload = typeof value === 'string' ? value : JSON.stringify(value);
  res.writeHead(status, {
    'content-type': contentType,
    'content-length': Buffer.byteLength(payload),
    'cache-control': 'no-store'
  });
  res.end(payload);
}

function boundedBody(req) {
  return new Promise((resolve, reject) => {
    let text = '';
    req.setEncoding('utf8');
    req.on('data', chunk => {
      text += chunk;
      if (text.length > MAX_BODY) {
        req.destroy();
        reject(Error('Request body exceeds local test limit'));
      }
    });
    req.on('error', reject);
    req.on('end', () => resolve(JSON.parse(text || '{}')));
  });
}

// The shared runtime sends *request-local aliases* for every model.
// This deterministic mock must mirror the observed alias contract, not
// bypass it with stale native DOM node IDs. Legacy fixture paths remain.
function observedNodes(prompt) {
  const markers = [
    'Fresh controls (untrusted labels/data): ',
    'Interactive nodes JSON: ',
    'Freshly observed controls JSON: '
  ];
  const position = markers
    .map(marker => ({marker, at: prompt.lastIndexOf(marker)}))
    .sort((a,b) => b.at - a.at)[0];
  if (!position || position.at < 0) return [];
  const start = position.at + position.marker.length;
  const line = prompt.slice(start).split(/\r?\n/,1)[0].trim();
  try {
    const value = JSON.parse(line);
    if (!Array.isArray(value)) return [];
    return value.map(node => ({...node,node_id:node.id || node.node_id}))
      .filter(node => node && typeof node.node_id === 'string' &&
        typeof node.label === 'string');
  } catch (_) {
    return [];
  }
}

function currentPageText(prompt) {
  const compactMarker = '<untrusted_page_data>';
  const detailedMarker = '<untrusted_page_data origin="';
  const compactIndex = prompt.lastIndexOf(compactMarker);
  const detailedIndex = prompt.lastIndexOf(detailedMarker);
  const index = Math.max(compactIndex,detailedIndex);
  if (index < 0) return prompt;
  const start = prompt.indexOf('>\n',index);
  if (start < 0) return prompt;
  const end = prompt.indexOf('\n</untrusted_page_data>',start + 2);
  return end < 0 ? prompt : prompt.slice(start + 2,end);
}

function decideAction(prompt) {
  const page = currentPageText(prompt);
  const nodes = observedNodes(prompt);
  const has = label => nodes.find(n => n.label === label);
  const reply = action => ({
    thought: 'Select the next action from the freshly observed local test page.',
    action
  });
  if (prompt.includes('Original user goal: On this local dictionary page, search for nebula')) {
    // Local mock decisions ONLY; the native runner must bind real live controls
    // and independently verify the resulting article URL, text and title.
    if (page.includes('Fixture Dictionary article: nebula')) {
      return reply({type:'complete',summary:'Observed the requested named dictionary article'});
    }
    const input = has('Search dictionary');
    if (page.includes('Query: nebula') && input) {
      return reply({type:'press_key',node_id:input.node_id,key:'Enter'});
    }
    if (input) {
      return reply({type:'type_text',node_id:input.node_id,
        text:'nebula',is_sensitive:false});
    }
    return reply({type:'scroll',delta_y:100});
  }
  if (prompt.includes('Original user goal: Open keyboard dialog')) {
    if (page.includes('KEYBOARD_DIALOG_CLOSED'))
      return reply({type:'complete',summary:'Observed dialog closed after Escape'});
    const keyboardEscape=escapeObservedKeyboardDialog(page,nodes);
    if (keyboardEscape) return reply(keyboardEscape);
    if (page.includes('Keyboard dialog not opened') && has('Open keyboard dialog'))
      return reply({type:'click',node_id:has('Open keyboard dialog').node_id,
        element_label:'Open keyboard dialog',is_submit:false});
    return reply({type:'scroll',delta_y:0});
  }
  if (prompt.includes('Original user goal: Load delayed documentation')) {
    if (page.includes('DELAYED_DOCS_CONFIRMED'))
      return reply({type:'complete',summary:'Opened documentation after delayed update'});
    if (has('Open delayed documentation'))
      return reply({type:'click',node_id:has('Open delayed documentation').node_id,
        element_label:'Open delayed documentation',is_submit:false});
    if (has('Load delayed documentation'))
      return reply({type:'click',node_id:has('Load delayed documentation').node_id,
        element_label:'Load delayed documentation',is_submit:false});
    return reply({type:'scroll',delta_y:0});
  }
  if (page.includes('CONTROLS_FLOW_CONFIRMED')) {
    return reply({type:'complete',summary:'Observed full browser controls confirmation'});
  }
  const detailsClose=closeObservedDetailsDialog(page,nodes);
  if (detailsClose) return reply(detailsClose);
  // The compact model prompt can end before the later dialog-status text.
  // Do not require a clipped "Dialog not opened" marker: the currently
  // observed, uniquely aliased Open details control is the action authority.
  // The independent native binding and final page evidence still decide
  // whether the click actually succeeded.
  if (page.includes('Advanced is enabled') &&
      has('Open details') &&
      !page.includes('Dialog is open') &&
      !page.includes('Dialog closed successfully')) {
    return reply({type:'click',node_id:has('Open details').node_id,
      element_label:'Open details',is_submit:false});
  }
  if (has('Category') && !page.includes('Selected category: Engineering')) {
    return reply({type:'select_option',node_id:has('Category').node_id,option_label:'Engineering'});
  }
  if (has('Include examples') && !page.includes('Examples enabled')) {
    return reply({type:'set_checked',node_id:has('Include examples').node_id,
      element_label:'Include examples',checked:true});
  }
  if (has('Intermediate') && !page.includes('Mode: Intermediate')) {
    return reply({type:'set_checked',node_id:has('Intermediate').node_id,
      element_label:'Intermediate',checked:true});
  }
  if (has('Draft note') && !page.includes('Note drafted')) {
    return reply({type:'type_text',node_id:has('Draft note').node_id,
      text:'Automated note',is_sensitive:false});
  }
  if (has('Show advanced') && !page.includes('Advanced is enabled')) {
    return reply({type:'set_checked',node_id:has('Show advanced').node_id,
      element_label:'Show advanced',checked:true});
  }
  if (page.includes('GENERIC_DOCS_CONFIRMED')) {
    return reply({type:'complete',summary:'Observed documentation reference confirmation'});
  }
  if (has('Open API reference')) {
    return reply({type:'click', node_id:has('Open API reference').node_id,
      element_label:'Open API reference',is_submit:false});
  }
  if (page.includes('Typed query: systems')) {
    const input = has('Search documentation');
    if (input) return reply({type:'press_key',node_id:input.node_id,key:'Enter'});
  }
  if (has('Search documentation')) {
    return reply({type:'type_text',node_id:has('Search documentation').node_id,
      text:'systems',is_sensitive:false});
  }
  if (page.includes('Recovery complete: fallback button succeeded')) {
    return reply({type:'complete', summary:'Observed local fallback confirmation'});
  }
  if (has('Use recovery fallback')) {
    return reply({type:'click', node_id:has('Use recovery fallback').node_id,
      element_label:'Use recovery fallback', is_submit:false});
  }
  if (has('Start recovery test')) {
    return reply({type:'click', node_id:has('Start recovery test').node_id,
      element_label:'Start recovery test', is_submit:false});
  }
  if (page.includes('VIDEO_PAGE_CONFIRMED')) {
    return reply({type:'complete', summary:'Observed robotics video success confirmation'});
  }
  if (has('Open robotics video')) {
    return reply({type:'click',node_id:has('Open robotics video').node_id,
      element_label:'Open robotics video',is_submit:false});
  }
  if (page.includes('Query: robotics video')) {
    // Deliberately emit an obsolete node ID to test exact-label DOM rebinding.
    return reply({type:'click',node_id:'invalid-search-button-id',
      element_label:'Search',is_submit:false});
  }
  if (has('Search')) {
    // This is allowed only because the original user goal includes this text.
    // The production runner independently rebinds and authorizes the action.
    return reply({type:'type_text',node_id:'invalid-search-input-id',
      text:'robotics video',is_sensitive:false});
  }
  return reply({type:'scroll',delta_y:180});
}

function verifyEvidence(payload) {
  const data = JSON.parse(payload || '{}');
  const sources = Array.isArray(data.observed_web_pages_untrusted)
    ? data.observed_web_pages_untrusted : [];
  const accepted = [
    {
      token:'KEYBOARD_DIALOG_CLOSED',
      quote:'KEYBOARD_DIALOG_CLOSED. Native dialog dismissed by Escape.',
      claim:'Native dialog closed after observed Escape action'
    },
    {
      token:'CONTROLS_FLOW_CONFIRMED',
      quote:'CONTROLS_FLOW_CONFIRMED. Engineering selected, examples enabled, Intermediate radio selected, advanced switch enabled and dialog opened then closed, with note drafted.',
      claim:'Browser dropdown, checkbox, radio, switch and dialog workflow completed'
    },
    {
      token:'DELAYED_DOCS_CONFIRMED',
      quote:'DELAYED_DOCS_CONFIRMED. Opened documentation after delayed page update.',
      claim:'Documentation opened after delayed page update'
    },
    {
      token:'GENERIC_DOCS_CONFIRMED',
      quote:'GENERIC_DOCS_CONFIRMED. API reference opened successfully after an Enter-key search.',
      claim:'API reference opened after autonomous documentation search'
    },
    {
      token:'VIDEO_PAGE_CONFIRMED',
      quote:'VIDEO_PAGE_CONFIRMED. Robotics video opened successfully after searching and selecting the first result.',
      claim:'Robotics video opened successfully'
    },
    {
      token:'Recovery complete: fallback button succeeded',
      quote:'Recovery complete: fallback button succeeded after the first click failed.',
      claim:'Recovery complete after fallback button succeeded'
    }
  ];
  for (const e of accepted) {
    const page = [...sources].reverse().find(s =>
      typeof s?.url === 'string' && typeof s?.excerpt === 'string'
      && s.excerpt.includes(e.quote));
    if (page) {
      return {status:'verified', findings:[{
        claim:e.claim,url:page.url,quote:e.quote
      }],uncertainty:'Local deterministic fixture only; no external website claims'};
    }
  }
  return {status:'insufficient',findings:[],uncertainty:'No observed fixture success quote'};
}

function chat(body) {
  const messages = Array.isArray(body.messages) ? body.messages : [];
  const system = String(messages.find(m => m.role === 'system')?.content || '');
  const user = String(messages.find(m => m.role === 'user')?.content || '');
  let response;
  if (system.includes('autonomous browser task planner')) {
    response = {stages:[{
      specialist:'browser',
      objective:'Use the local website controls to complete the user-requested test',
      success_criteria:'Observe the actual result confirmation after the browser actions'
    }]};
  } else if (system.includes('independent, READ-ONLY evidence-review specialist')) {
    response = verifyEvidence(user);
  } else if (system.includes('Check whether these source-backed findings answer the user')) {
    // Second-pass semantic review must be a separate response: returning a
    // browser_task object here fails closed in Rust, but disguises a broken
    // deterministic fixture as a browser-navigation/recovery regression.
    response = reviewFixtureEvidence(user);
  } else {
    response = {mode:'browser_task'};
  }
  return {model:'lucia-local-e2e',message:{role:'assistant',content:JSON.stringify(response)},done:true};
}

function runLocalModel(req, res) {
  if (req.method === 'GET' && req.url === '/health')
    return send(res, 200, {ok:true,kind:'mock-provider'});
  if (req.method === 'GET' && req.url === '/stats')
    return send(res, 200, { ...providerStats });
  if (req.method === 'GET' && req.url === '/v1/models')
    return send(res, 200, {object:'list',data:[
      {id:'lucia-cloud-e2e',object:'model'}
    ]});
  if (req.method === 'GET' && req.url === '/api/tags') {
    return send(res, 200, {models:[{name:'lucia-local-e2e',model:'lucia-local-e2e'}]});
  }
  if (req.method !== 'POST' ||
      !['/api/chat','/api/generate','/v1/chat/completions'].includes(req.url)) {
    return send(res, 404, {error:'Not found'});
  }
  boundedBody(req).then(body => {
    if (req.url === '/v1/chat/completions') {
      const model = String(body.model || '');
      const failures = {
        'lucia-cloud-reject-401': [401,'invalid_api_key'],
        'lucia-cloud-limit-429': [429,'rate_limit_exceeded'],
        'lucia-cloud-unavailable-503': [503,'provider_unavailable']
      };
      if (Object.prototype.hasOwnProperty.call(failures, model)) {
        const [status, code] = failures[model];
        return send(res, status, {error:{type:code,code,message:'Simulated provider failure'}});
      }
      if (model === 'lucia-cloud-invalid-json') {
        return send(res,200,{id:'chatcmpl-invalid',choices:[
          {index:0,message:{role:'assistant',content:'not a browser action'},finish_reason:'stop'}
        ]});
      }
      if (model !== 'lucia-cloud-e2e') {
        return send(res, 404, {error:{code:'model_not_found'}});
      }
      const messages = Array.isArray(body.messages) ? body.messages : [];
      const phase = cloudPhase(messages);
      providerStats[phase]++;
      const user = String(messages.find(m => m.role === 'user')?.content || '');
      const answer = phase === 'actions'
        ? JSON.stringify(decideAction(user))
        : chat(body).message.content;
      return send(res, 200, {
        id:'chatcmpl-lucia-fixture',model,
        choices:[{index:0,message:{role:'assistant',content:answer},finish_reason:'stop'}]
      });
    }
    providerStats.ollama++;
    if (req.url === '/api/chat') return send(res, 200, chat(body));
    const output = decideAction(String(body.prompt || ''));
    return send(res, 200, {model:'lucia-local-e2e',response:JSON.stringify(output),done:true});
  }).catch(e => send(res, 400, {error:String(e.message||e)}));
}

function runFixture(req, res) {
  if (req.url === '/health') return send(res, 200, {ok:true,kind:'fixture'});
  const exact = {
    '/':'index.html',
    '/index.html':'index.html',
    '/multistep.html':'multistep.html',
    '/generic-search.html':'generic-search.html',
    '/delayed-docs.html':'delayed-docs.html',
    '/recovery.html':'recovery.html',
    '/generic-controls.html':'generic-controls.html',
    '/generic-document.html':'generic-document.html',
    '/form-controls.html':'form-controls.html',
    '/named-enter.html':'named-enter.html',
    '/nebula.html':'nebula.html'
  }[new URL(req.url || '/', 'http://127.0.0.1').pathname];
  if (!exact || req.method !== 'GET') return send(res, 404, 'Not found', 'text/plain');
  try {
    send(res, 200, fs.readFileSync(path.join(fixtureRoot,exact),'utf8'),
      'text/html; charset=utf-8');
  } catch (_) {
    send(res, 500, 'Fixture missing', 'text/plain');
  }
}

if (require.main === module) {
  const models = http.createServer(runLocalModel);
  const fixtures = http.createServer(runFixture);
  models.listen(modelPort, bind, () => {
    fixtures.listen(fixturePort, bind, () =>
      console.log('LUCIA_MOCK_READY model=' + modelPort + ' fixture=' + fixturePort));
  });
  process.on('SIGINT', () => { models.close(); fixtures.close(); });
  process.on('SIGTERM', () => { models.close(); fixtures.close(); });
}

module.exports = {observedNodes,decideAction,currentPageText,verifyEvidence,chat,cloudPhase,runLocalModel,providerStats};
