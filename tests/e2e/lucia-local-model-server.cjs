'use strict';
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

function observedNodes(prompt) {
  const suffix = prompt.split('Interactive nodes JSON: ').at(-1);
  try {
    const value = JSON.parse(suffix);
    return Array.isArray(value) ? value : [];
  } catch (_) {
    return [];
  }
}

function decideAction(prompt) {
  const nodes = observedNodes(prompt);
  const has = label => nodes.find(n => n.label === label);
  const reply = action => ({
    thought: 'Select the next action from the freshly observed local test page.',
    action
  });
  if (prompt.includes('Recovery complete: fallback button succeeded')) {
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
  if (prompt.includes('VIDEO_PAGE_CONFIRMED')) {
    return reply({type:'complete', summary:'Observed robotics video success confirmation'});
  }
  if (has('Open robotics video')) {
    return reply({type:'click',node_id:has('Open robotics video').node_id,
      element_label:'Open robotics video',is_submit:false});
  }
  if (prompt.includes('Query: robotics video')) {
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
  } else {
    response = {mode:'browser_task'};
  }
  return {model:'lucia-local-e2e',message:{role:'assistant',content:JSON.stringify(response)},done:true};
}

function runLocalModel(req, res) {
  if (req.url === '/health') return send(res, 200, {ok:true,kind:'mock-ollama'});
  if (req.method === 'GET' && req.url === '/api/tags') {
    return send(res, 200, {models:[{name:'lucia-local-e2e',model:'lucia-local-e2e'}]});
  }
  if (req.method !== 'POST' || !['/api/chat','/api/generate'].includes(req.url)) {
    return send(res, 404, {error:'Not found'});
  }
  boundedBody(req).then(body => {
    if (req.url === '/api/chat') return send(res, 200, chat(body));
    const output = decideAction(String(body.prompt || ''));
    return send(res, 200, {model:'lucia-local-e2e',response:JSON.stringify(output),done:true});
  }).catch(e => send(res, 400, {error:String(e.message||e)}));
}

function runFixture(req, res) {
  if (req.url === '/health') return send(res, 200, {ok:true,kind:'fixture'});
  const exact = {
    '/multistep.html':'multistep.html',
    '/recovery.html':'recovery.html'
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

module.exports = {observedNodes,decideAction,verifyEvidence,chat};
