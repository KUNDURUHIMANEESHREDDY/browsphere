'use strict';
const {approvedNativeFixtureSearch} = require('./cloud-native-shortcut.cjs');
// Real Windows Tauri/WebView2 end-to-end test with a LOOPBACK model simulator.
// No live public sites, credentials, paid API endpoints or consent bypasses.
const assert = require('node:assert/strict');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const port = Number(process.env.LUCIA_CDP_PORT || 9272);
const base = 'http://127.0.0.1:' + Number(process.env.LUCIA_FIXTURE_PORT || 18764);
const realModel = process.env.LUCIA_E2E_REAL_MODEL?.trim() || null;
const modelUrl = realModel ? 'http://127.0.0.1:11434' : 'http://127.0.0.1:' + Number(process.env.LUCIA_MODEL_PORT || 18766);
const chosenModel = realModel || 'lucia-local-e2e';
const cloudCompat = process.env.LUCIA_E2E_CLOUD_COMPAT === '1';

async function connect() {
  let target;
  for (let attempt = 0; attempt < 40; attempt++) {
    const targets = await (await fetch('http://127.0.0.1:' + port + '/json')).json();
    target = targets.find(t => t.type === 'page' && t.url?.includes('tauri.localhost'));
    if (target) break;
    await sleep(350);
  }
  assert.ok(target, 'Native Browsphere main WebView2 target missing');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok,bad) => { ws.onopen = ok; ws.onerror = bad; });
  let seq = 0;
  const pending = new Map();
  ws.onmessage = event => {
    const result = JSON.parse(event.data);
    const task = pending.get(result.id);
    if (!task) return;
    pending.delete(result.id);
    result.error ? task.reject(Error(JSON.stringify(result.error)))
      : task.resolve(result.result || {});
  };
  async function command(method, params = {}) {
    const id = ++seq;
    return Promise.race([
      new Promise((resolve,reject) => {
        pending.set(id,{resolve,reject});
        ws.send(JSON.stringify({id,method,params}));
      }),
      sleep(18000).then(() => {
        if (pending.has(id)) {
          pending.delete(id);
          throw Error('CDP command timed out: ' + method);
        }
      })
    ]);
  }
  async function evaluate(expression) {
    const response = await command('Runtime.evaluate',{
      expression, returnByValue:true, awaitPromise:true, userGesture:true
    });
    if (response.exceptionDetails) {
      throw Error('Evaluation failed: ' + JSON.stringify(response.exceptionDetails).slice(0,600));
    }
    return response.result?.value;
  }
  return {ws,command,evaluate};
}

async function main() {
  if(realModel){
    const listing=await (await fetch(modelUrl + '/api/tags')).json();
    assert.ok((listing.models||[]).some(m=>m.name===realModel || m.model===realModel),
      'The configured real Qwen model is not installed in Ollama');
  } else {
    assert.equal((await (await fetch(modelUrl + '/health')).json()).ok,true,
      'Loopback simulated model is not running');
  }
  assert.equal((await (await fetch(base + '/health')).json()).ok,true,
    'Local webpage fixture is not running');
  const {ws,command,evaluate} = await connect();
  const snapshotExpression = "window.__TAURI__.core.invoke('capture_live_snapshot')";
  const eventsExpression = "window.__luciaNativeEvents.slice(-300)";
  try {
    await command('Runtime.enable');
    // The CDP endpoint can appear before the chrome's JavaScript is initialized.
    let shellReady = false;
    for (let i=0; i<60; i++) {
      shellReady = await evaluate('Boolean(window.browsphereApp?.newTab && window.aetherBridge?.isNative)').catch(() => false);
      if (shellReady) break;
      await sleep(300);
    }
    assert.ok(shellReady, 'Native Browsphere UI never became ready');
    // Cloud CI deliberately points Ollama at a closed port. Every specialist
    // must use the OpenAI-compatible API; local fallback would fail the run.
    const localEndpoint = cloudCompat ? 'http://127.0.0.1:1' : modelUrl;
    const cloudEndpoint = modelUrl + '/v1/chat/completions';
    await evaluate("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');c.ollama_url=" +
      JSON.stringify(localEndpoint) + ";c.ollama_model=" + JSON.stringify(chosenModel) +
      ";c.litellm_url=" + JSON.stringify(cloudEndpoint) +
      ";c.litellm_model='lucia-cloud-e2e';c.lucia_cloud_only=" + String(cloudCompat) +
      ";c.prefer_local_for_simple=" + String(!cloudCompat) +
      ";c.domain_overrides={};await window.__TAURI__.core.invoke('update_router_config',{config:c});window.__luciaNativeEvents=[];for(const name of ['agent:plan','agent:stage','agent:trace','agent:run_status','agent:run_error','agent:verification_report','agent:approval_required','agent:observation','agent:activity'])await window.__TAURI__.event.listen(name,e=>window.__luciaNativeEvents.push({event:name,payload:e.payload}));return true})()");
    async function openLocal(path) {
      const url = base + path;
      await evaluate('window.browsphereApp.newTab(' + JSON.stringify(url) + ')');
      for (let i=0;i<40;i++) {
        await sleep(350);
        const observed = await evaluate(snapshotExpression).catch(() => null);
        if (observed?.origin === url &&
            observed.interactive_nodes?.length > 0) {
          return observed;
        }
        // A child WebView can expose the destination URL before DOM
        // hydration finishes. Retry its fresh observation until controls load.
      }
      throw Error('Fixture navigation not observed in native WebView: ' + url);
    }

    async function run(goal, initial, limit=realModel?720:60) {
      // Exercise the real UI-to-Rust bridge, including run-to-tab registration.
      const id = await evaluate('window.aetherBridge.startLuciaSpecialistTask(' +
        JSON.stringify(goal) + ',' + JSON.stringify(initial.origin) + ',' +
        JSON.stringify(initial.title) + ')');
      assert.ok(id,'Native agent run failed to start');
      let terminal, events = [];
      let reported = 0;
      for(let i=0;i<limit;i++){
        await sleep(750);
        events = await evaluate(eventsExpression) || [];
        if(realModel){
          const progress=events.filter(x=>x.payload?.run_id===id &&
            ['agent:trace','agent:stage','agent:run_error','agent:run_status'].includes(x.event));
          for(const entry of progress.slice(reported)){
            // Log only sanitized action classifications, never page text,
            // model prompts, typed values, personal information or secrets.
            console.log('QWEN_PROGRESS',JSON.stringify({
              event:entry.event,action:entry.payload?.action?.type,
              outcome:entry.payload?.outcome,status:entry.payload?.status,
              specialist:entry.payload?.specialist
            }));
          }
          reported=progress.length;
        }
        const approval = events.find(x=>x.event==='agent:approval_required'
          && x.payload?.run_id===id);
        if(approval){
          await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
          throw Error('Unexpected approval required on isolated harmless test fixture');
        }
        terminal=events.find(x=>x.event==='agent:run_status'
          && x.payload?.run_id===id);
        if(terminal)break;
      }
      if(!terminal){
        await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')");
        throw Error('Lucia task timed out');
      }
      const relevant=events.filter(x=>x.payload?.run_id===id);
      const actions=relevant.filter(x=>x.event==='agent:trace').map(x=>({
        type:x.payload?.action?.type,
        outcome:x.payload?.outcome,
        route:x.payload?.model_route
      }));
      const report=relevant.find(x=>x.event==='agent:verification_report')?.payload?.report;
      const errors=relevant.filter(x=>x.event==='agent:run_error')
        .map(x=>x.payload?.message);
      const phases=relevant.filter(x=>x.event==='agent:activity')
        .map(x=>x.payload?.phase);
      assert.ok(phases.includes('observing') && phases.includes('observed') &&
        phases.includes('deciding') && phases.includes('acting') &&
        phases.includes('checking'), 'Native Lucia did not publish each live task phase');
      assert.ok(relevant.filter(x=>x.event==='agent:activity').every(x=>
        !Object.keys(x.payload||{}).some(k=>['thought','text','prompt','url','token'].includes(k))),
        'Live task phases must not expose page content, prompts, or model reasoning');
      console.log('AGENT_RUN',JSON.stringify({
        status:terminal.payload?.status,actions,reportStatus:report?.status,errors
      }));
      assert.equal(terminal.payload?.status,'completed','Agent run did not finish');
      assert.equal(report?.status,'evidence_checked','Missing independent evidence');
      if (cloudCompat) {
        const modelActions=actions.filter(a=>a.route?.includes('Browser Agent') &&
          a.type !== 'complete' && a.outcome !== 'stopped_or_unknown');
        const narrowTitleLookup = /search for .+ and open .+report.*title/i.test(goal);
        const trustedNamedTitleSequence = narrowTitleLookup &&
          modelActions.some(a=>a.type==='type_text' &&
            a.route?.includes('search_input:trusted_user_query')) &&
          modelActions.some(a=>a.type==='press_key' &&
            a.route?.includes('search_enter:after_observed_query_type'));
        // Only the exact, successfully evidenced four-action local fixture
        // permits zero cloud inference. The production browser still enforces
        // live-DOM bindings and independent review; no Ollama fallback is used.
        const trustedNativeFixtureSearch=approvedNativeFixtureSearch(goal,modelActions,report);
        assert.ok(modelActions.some(a=>a.route?.includes('litellm:lucia-cloud-e2e')) ||
          trustedNamedTitleSequence || trustedNativeFixtureSearch,
          'No cloud-model action or rigorously verified native fixture route was observed');
        assert.ok(modelActions.every(a=>a.route?.includes('litellm:lucia-cloud-e2e') ||
          (a.type==='click' && a.route?.includes('first_result:unique_live_dom')) ||
          (a.type==='type_text' && a.route?.includes('search_input:trusted_user_query')) ||
          (a.type==='press_key' && a.route?.includes('search_enter:after_observed_query_type')) ||
          (trustedNativeFixtureSearch && a.type==='click' &&
            a.route?.includes('recovery:ranked_live_alternative'))),
          'Browser action used an unexpected provider or ungrounded route');
        assert.ok(actions.every(a=>!a.route?.includes('ollama:')),
          'Cloud-only task unexpectedly fell back to Ollama');
      }
      const observations = relevant.filter(x=>x.event==='agent:observation')
        .map(x=>x.payload);
      return {actions,report,observations};
    }

    // 0. Non-YouTube website: 160 navigation links precede the actual content.
    // Test real native DOM prioritization, image-only links, unique control IDs,
    // Shadow DOM labels and article text—not model-supplied page descriptions.
    const generic = await openLocal('/generic-controls.html');
    const controls = generic.interactive_nodes || [];
    const findLabel = label => controls.find(n => n.label === label);
    console.log('GENERIC_SITE_CONTROLS', JSON.stringify({
      firstControls: controls.slice(0,12).map(n=>({tag:n.tag,role:n.role,label:n.label})),
      controlsObserved:controls.length,
      mainTextPrefix:generic.text_excerpt.slice(0,125)
    }));
    assert.ok(generic.text_excerpt.startsWith('INDEPENDENT_SITE_CONTENT:'),
      'Main article content should precede unrelated global navigation');
    assert.ok(findLabel('Robotics reference article'),
      'Image-only link must expose an accessible label');
    assert.ok(findLabel('Search robotics documentation'),
      'Shadow-root aria-labelledby input must be observed');
    assert.ok(findLabel('Search docs'),
      'Open Shadow DOM button must be observed');
    const duplicates = controls.filter(n => n.label === 'Show first section' ||
      n.label === 'Show second section');
    assert.equal(duplicates.length,2, 'Both initially duplicate-ID buttons must be observed');
    assert.notEqual(duplicates[0].node_id, duplicates[1].node_id,
      'Different controls may never share a live DOM node ID');
    assert.ok(controls.slice(0,8).some(n => n.label === 'Robotics reference article'),
      'Semantically relevant content should precede 160 unrelated site-menu links');
    assert.ok(controls.length<=120, 'Native DOM observation must respect its node budget');
    const documentPage = await openLocal('/generic-document.html');
    assert.ok(documentPage.text_excerpt.includes('GENERAL_LINK_DESTINATION_CONFIRMED'),
      'Generic linked article must expose readable success evidence');
    console.log('PASS generic sites: main-content prioritization, image links, Shadow DOM, duplicate IDs, linked article');

    // 1. Search text -> submit local search -> click result -> SPA URL -> verified.
    const initial=await openLocal('/multistep.html');
    const search=await run(
      realModel
        ? 'On the current local website, type robotics video into the Search box, click Search, then open the first robotics video result. Report the opened video details page.'
        : 'On the current local test page, search for robotics video and open the first result.',
      initial
    );
    assert.ok(search.actions.some(x=>x.type==='type_text'
      && ['ok','confirmed'].includes(x.outcome)), 'Search typing not executed');
    assert.ok(search.actions.filter(x=>x.type==='click'
      && ['ok','confirmed'].includes(x.outcome)).length>=2,
      'Search control + result were not clicked');
    const after=await evaluate(snapshotExpression);
    assert.ok(after.origin.includes('/multistep.html?video=robotics-demo'),
      'No real observed SPA video-result URL');
    assert.ok(after.text_excerpt.includes('VIDEO_PAGE_CONFIRMED'),
      'No visible result confirmation');
    // The latest completed DOM observation must update the actual UI address bar.
    let synced=false;
    for(let i=0;i<15;i++){
      const tab=await evaluate('window.browsphereApp.activeTab()');
      if(tab?.url===after.origin){synced=true;break;}
      await sleep(150);
    }
    assert.ok(synced,'Visible browser URL was not synchronized with SPA navigation');
    console.log('PASS native multi-step typing, search, result click, SPA URL sync and evidence review');

    if (!realModel) {
    // A named-article search can navigate straight to its result on Enter.
    // The production agent must still use the chosen model for real actions
    // and a fresh WebView2 title/URL for final verification; its specialist
    // plan is fixed so one unnecessary hosted planning request is avoided.
    const providerBefore = cloudCompat ? await (await fetch(modelUrl+'/stats')).json() : null;
    const namedInitial = await openLocal('/named-enter.html');
    const named = await run(
      'On this local dictionary page, search for nebula and open the corresponding article. Then report its page title.',
      namedInitial,100
    );
    assert.ok(named.actions.some(a=>a.type==='type_text'&&a.outcome==='ok'),
      'Named article query was not typed into the observed field');
    assert.ok(named.actions.some(a=>a.type==='press_key'&&a.outcome==='ok'),
      'Named article search was not submitted through its observed Enter key');
    const namedAfter = await evaluate(snapshotExpression);
    assert.equal(new URL(namedAfter.origin).pathname,'/nebula.html',
      'Enter search did not open the requested article in native WebView2');
    assert.equal(namedAfter.title,'nebula - Fixture Dictionary',
      'The native article title is missing or mismatched');
    assert.ok(namedAfter.text_excerpt.includes('Fixture Dictionary article: nebula'),
      'Named article has no readable source evidence');
    if (cloudCompat) {
      const providerAfter = await (await fetch(modelUrl+'/stats')).json();
      assert.equal(providerAfter.planner-providerBefore.planner,0,
        'Simple title lookup unexpectedly spent a cloud model request on planning');
      // A narrow read-only named-title lookup now types the exact supplied
      // query and submits once through a native, observed searchbox. No
      // cloud action inference is needed for these two deterministic steps.
      assert.equal(providerAfter.actions-providerBefore.actions,0,
        'Named-title lookup unexpectedly used cloud action inference');
    }
    console.log('PASS native grounded search Enter -> named article, independent evidence, no unnecessary cloud calls');

    // 2. Generic website: heavy navigation chrome hides real controls unless
    // the DOM reader prioritizes main content; search works only on Enter.
    const docsInitial = await openLocal('/generic-search.html');
    const docs = await run(
      'Search this documentation website for systems, press Enter, and open the API reference.',
      docsInitial
    );
    assert.ok(docs.actions.some(a=>a.type==='type_text' &&
      ['ok','confirmed'].includes(a.outcome)),'Generic docs query not typed');
    assert.ok(docs.actions.some(a=>a.type==='press_key' &&
      ['ok','confirmed'].includes(a.outcome)),'Generic docs Enter action not executed');
    assert.ok(docs.actions.some(a=>a.type==='click' &&
      ['ok','confirmed'].includes(a.outcome)),'Generic docs result link not clicked');
    const docsAfter = await evaluate(snapshotExpression);
    assert.ok(docsAfter?.origin?.includes('/generic-search.html?doc=api'),
      'Generic docs result URL not observed');
    assert.ok(docsAfter?.text_excerpt?.includes('GENERIC_DOCS_CONFIRMED'),
      'Generic docs confirmation not observed');
    console.log('PASS native generic website with 180 navigation controls and Enter-only search');

    const delayedInitial = await openLocal('/delayed-docs.html');
    const delayed = await run(
      'Load delayed documentation, then open the documentation result after it appears.',
      delayedInitial,100
    );
    const delayedClicks = delayed.actions.filter(a=>a.type==='click' &&
      ['ok','confirmed'].includes(a.outcome));
    assert.equal(delayedClicks.length,2,
      'Slow SPA task clicked a control more than once or failed to open the result');
    assert.ok(delayed.observations.some(o=>o?.phase==='post_action' && o.changed===true),
      'No observed page transition from the delayed action');
    const delayedAfter = await evaluate(snapshotExpression);
    assert.ok(delayedAfter.origin.includes('/delayed-docs.html?doc=ready'),
      'Delayed result URL was not observed');
    assert.ok(delayedAfter.text_excerpt.includes('DELAYED_DOCS_CONFIRMED'),
      'Delayed result confirmation missing from live DOM');
    console.log('PASS native delayed SPA updates: settled observation, two grounded clicks and evidence review');

    // 3. A cross-control form sequence using the same cloud action schema.
    const controlsInitial = await openLocal('/form-controls.html');
    const category = controlsInitial.interactive_nodes.find(n => n.label === 'Category');
    assert.ok(category?.options?.includes('Engineering'),
      'DOM does not expose safe observed options for a native dropdown');
    const checkbox = controlsInitial.interactive_nodes.find(n => n.label === 'Include examples');
    assert.equal(checkbox?.checked,false,'Checkbox state was not observed');
    const formRun = await run(
      'On this test page select Engineering, enable examples, choose Intermediate, enable Show advanced, open details and close the dialog.',
      controlsInitial,100
    );
    assert.ok(formRun.actions.some(a=>a.type==='select_option' &&
      ['ok','confirmed'].includes(a.outcome)), 'Dropdown option was not selected');
    assert.ok(formRun.actions.some(a=>a.type==='type_text' &&
      ['ok','confirmed'].includes(a.outcome)), 'Rich-text editor was not filled');
    assert.ok(formRun.actions.filter(a=>a.type==='set_checked' &&
      ['ok','confirmed'].includes(a.outcome)).length >= 3,
      'Checkbox, radio and switch actions did not all succeed');
    const controlClicks = formRun.actions.filter(a=>a.type==='click' &&
      ['ok','confirmed'].includes(a.outcome));
    assert.ok(controlClicks.length >= 2,'Dialog open and close clicks did not complete');
    const controlsAfter = await evaluate(snapshotExpression);
    assert.ok(controlsAfter.text_excerpt.includes('CONTROLS_FLOW_CONFIRMED'),
      'Full controls workflow not confirmed in actual DOM');
    if (process.env.LUCIA_E2E_CLOUD_COMPAT === '1') {
      assert.ok(formRun.actions.some(a=>a.route?.includes('litellm:lucia-cloud-e2e')),
        'Cloud compatible model route was not used by real native agent');
    }
    console.log('PASS native dropdown, checkbox, radio, switch, rich editor, dialog and cloud-compatible actions');

    // A dialog without a close button should dismiss via Escape.
    const keyboardInitial = await openLocal('/form-controls.html');
    const keyboard = await run(
      'Open keyboard dialog, then close it using Escape.',keyboardInitial,65
    );
    assert.ok(keyboard.actions.some(a=>a.type==='press_key' &&
      ['ok','confirmed'].includes(a.outcome)), 'Dialog Escape action never succeeded');
    const keyboardAfter = await evaluate(snapshotExpression);
    assert.ok(keyboardAfter.text_excerpt.includes('KEYBOARD_DIALOG_CLOSED'),
      'Native dialog did not visibly close after Escape');
    console.log('PASS native Escape closes a verified open dialog');

    }

    // 4. Genuine failed primary click -> model recovers with fallback -> verified.
    const recoveryInitial=await openLocal('/recovery.html');
    const recovery=await run(
      'On the local recovery test page, click Start recovery test. After it fails, use the recovery fallback.',
      recoveryInitial
    );
    const failedAt=recovery.actions.findIndex(x=>x.type==='click' && x.outcome==='failed');
    assert.ok(failedAt>=0,'Test did not observe a genuine failed click');
    assert.ok(recovery.actions.slice(failedAt+1).some(x=>x.type==='click'
      && ['ok','confirmed'].includes(x.outcome)),'No successful fallback click');
    const finalVerification=recovery.actions.filter(x=>x.type==='complete'
      && x.route?.includes('Verification Agent')).at(-1);
    assert.equal(finalVerification?.outcome,'ok',
      'Verifier audit trace must report success even when its summary mentions a previous failed click');
    const afterRecovery=await evaluate(snapshotExpression);
    assert.ok(afterRecovery.text_excerpt.includes('Recovery complete: fallback button succeeded'),
      'Recovery confirmation not visible');
    console.log('PASS native failed action, autonomous fallback, and independent verification');
    if (realModel) {
      // Keep the CPU-only hosted-runner real-LLM experiment scoped to two
      // distinct task patterns; mock-model CI continues testing background tabs.
      console.log('PASS REAL MODEL: '+realModel+
        ' completed search/click/SPA navigation and failed-action recovery');
      return;
    }

    // 3. A fresh task-owned background WebView must finish while the user
    // switches to another unrelated tab. Its verified result must not
    // navigate or mutate the foreground tab.
    const backgroundInitial = await openLocal('/multistep.html');
    const taskId = await evaluate('window.browsphereApp.createTaskTab(' +
      JSON.stringify(backgroundInitial.origin) + ')');
    assert.match(taskId, /^tab-lucia-/);
    const backgroundGoal = 'On the current local test page, search for robotics video and open the first result.';
    const backgroundRun = await evaluate(
      'window.aetherBridge.startLuciaSpecialistTask(' +
      JSON.stringify(backgroundGoal) + ',' +
      JSON.stringify(backgroundInitial.origin) + ',' +
      JSON.stringify(backgroundInitial.title) + ',' + JSON.stringify(taskId) + ')');
    assert.ok(backgroundRun, 'Background Lucia run did not start');
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    const browsingTab = await evaluate('window.browsphereApp.activeTab()');
    assert.notEqual(browsingTab.id, taskId, 'Browsing tab should differ from task tab');
    let backgroundTerminal;
    for (let i = 0; i < 80; i++) {
      await sleep(700);
      const events = await evaluate(eventsExpression) || [];
      backgroundTerminal = events.find(e => e.event === 'agent:run_status' &&
        e.payload?.run_id === backgroundRun);
      if (backgroundTerminal) break;
    }
    assert.ok(backgroundTerminal, 'Background task never reached a terminal state');
    if (backgroundTerminal.payload?.status !== 'completed') {
      const backgroundEvents = await evaluate(eventsExpression) || [];
      const debugPage = await evaluate(
      "window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:" + JSON.stringify(taskId) + "})").catch(()=>null);
    console.log('BACKGROUND_DEBUG',JSON.stringify({
      status:backgroundTerminal.payload?.status,
      origin:debugPage?.origin,
      interactiveCount:debugPage?.interactive_nodes?.length,
      textFound:debugPage?.text_excerpt?.includes('VIDEO_PAGE_CONFIRMED'),
      traces:backgroundEvents.filter(e=>e.event==='agent:trace'&&e.payload?.run_id===backgroundRun)
        .map(e=>({action:e.payload?.action?.type,outcome:e.payload?.outcome,route:e.payload?.model_route})),
      errors:backgroundEvents.filter(e=>e.event==='agent:run_error'&&e.payload?.run_id===backgroundRun)
        .map(e=>e.payload?.message)
      }));
    }
    assert.equal(backgroundTerminal.payload?.status, 'completed',
      'The background multi-step task failed');
    const backgroundSnapshot = await evaluate(
      "window.__TAURI__.core.invoke('capture_tab_snapshot',{tabId:" +
      JSON.stringify(taskId) + "})");
    assert.ok(backgroundSnapshot?.origin?.includes('/multistep.html?video=robotics-demo'),
      'Background task did not open the first video result');
    assert.ok(backgroundSnapshot.text_excerpt.includes('VIDEO_PAGE_CONFIRMED'),
      'Background target page lacked verification evidence');
    assert.equal((await evaluate('window.browsphereApp.activeTab()')).id, browsingTab.id,
      'Task unexpectedly changed the foreground tab');
    console.log('PASS native task-owned tab continues during unrelated browsing');

    // 4. Native close must cancel the matching task, including when paused.
    await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:true})");
    try {
      const closingTab = await evaluate(
        "window.browsphereApp.createTaskTab('about:blank')");
      const closingRun = await evaluate(
        "window.aetherBridge.startLuciaSpecialistTask(" +
          JSON.stringify('On this local test page, find the recovery control.') +
          ",'browsphere://start','New Tab'," + JSON.stringify(closingTab) + ")");
      assert.ok(closingRun, 'Closing test run did not start');
      // The user is still browsing elsewhere; close the task target by ID.
      await evaluate('window.browsphereApp.closeTab(' + JSON.stringify(closingTab) + ')');
      let closedTerminal, closedError;
      for (let i=0; i<45; i++) {
        await sleep(350);
        const events = await evaluate(eventsExpression) || [];
        closedError = events.find(e => e.event === 'agent:run_error' &&
          e.payload?.run_id === closingRun &&
          /assigned browser tab was closed/.test(e.payload?.message || ''));
        closedTerminal = events.find(e => e.event === 'agent:run_status' &&
          e.payload?.run_id === closingRun);
        if (closedTerminal && closedError) break;
      }
      assert.ok(closedError, 'Closing the assigned tab did not report the cause');
      assert.equal(closedTerminal?.payload?.status, 'tab_closed',
        'Closing a task tab must stop the run with the correct cause');
      console.log('PASS closing the background task tab cancels with a clear message');
    } finally {
      await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:false})");
    }

    if (cloudCompat) {
      const response = await fetch(modelUrl + '/stats');
      assert.equal(response.status,200,'Cloud-model diagnostic counters unavailable');
      const counts = await response.json();
      assert.ok(counts.planner >= 4,'Cloud model was not used for task planning');
      assert.ok(counts.actions >= 10,'Cloud model did not propose enough browser actions');
      assert.ok(counts.verifier >= 4,'Cloud model was not used for source verification');
      assert.equal(counts.ollama,0,'Local Ollama requests occurred during cloud-only test');
      console.log('PASS cloud-only planner, browser actions, and verifier; no local fallback',
        JSON.stringify(counts));

      // Use the actual Rust cloud-agent connection probe against simulated
      // provider errors. Never infer a successful action from HTTP 200 alone.
      async function providerFailure(model) {
        return evaluate("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');" +
          "c.litellm_model=" + JSON.stringify(model) +
          ";await window.__TAURI__.core.invoke('update_router_config',{config:c});" +
          "try{await window.__TAURI__.core.invoke('test_model_connection',{backend:'cloud-agent'});" +
          "return 'UNEXPECTED_SUCCESS'}catch(e){return String(e)}})()");
      }
      try {
        assert.match(await providerFailure('lucia-cloud-reject-401'),/HTTP 401/,
          'Auth failure was not surfaced from the selected cloud provider');
        assert.match(await providerFailure('lucia-cloud-limit-429'),/HTTP 429/,
          'Cloud provider rate limit was not surfaced');
        assert.match(await providerFailure('lucia-cloud-unavailable-503'),/HTTP 503/,
          'Cloud provider outage was not surfaced');
        assert.match(await providerFailure('lucia-cloud-invalid-json'),
          /not a valid browser action/,
          'Invalid cloud browser-action JSON was not rejected');
      } finally {
        await evaluate("(async()=>{const c=await window.__TAURI__.core.invoke('get_router_config');" +
          "c.litellm_model='lucia-cloud-e2e';" +
          "await window.__TAURI__.core.invoke('update_router_config',{config:c});return true})()");
      }
      const afterFailures=await (await fetch(modelUrl+'/stats')).json();
      assert.equal(afterFailures.ollama,0,
        'Cloud provider failures unexpectedly triggered local Ollama fallback');
      console.log('PASS cloud provider HTTP 401/429/503 and invalid-action fail closed without Ollama');
    }

    // 5. The user-facing View task tab control must open the assigned tab.
    await evaluate("window.browsphereApp.newTab('browsphere://start')");
    await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:true})");
    try {
      const chatReady = await evaluate("Boolean(document.getElementById('newTabChatInput'))");
      assert.ok(chatReady, 'Main Lucia composer unavailable');
      await evaluate("(()=>{const f=document.getElementById('newTabChatInput');" +
        "f.value='On this blank local test page, inspect available controls.';" +
        `document.querySelector('[data-action="nt-run-agent"]').click();return true})()`);
      let viewTabId;
      for (let i=0;i<45;i++) {
        await sleep(250);
        viewTabId = await evaluate(
          `document.querySelector('[data-action="nt-view-task-tab"]')?.dataset.tabId || null`);
        if (viewTabId) break;
      }
      assert.match(viewTabId || '', /^tab-lucia-/,
        'View task tab button not shown in the Lucia conversation');
      await evaluate(`document.querySelector('[data-action="nt-view-task-tab"]').click()`);
      const opened = await evaluate('window.browsphereApp.activeTab()');
      assert.equal(opened.id, viewTabId, 'View task tab did not select its target');
      await evaluate('window.browsphereApp.closeTab(' + JSON.stringify(viewTabId) + ')');
      console.log('PASS task progress button opens assigned WebView2 tab');
    } finally {
      await evaluate("window.__TAURI__.core.invoke('set_agent_paused',{paused:false})");
    }
  } finally {
    ws.close();
  }
}

main().catch(err=>{console.error('FAIL',err.stack||err);process.exitCode=1});
