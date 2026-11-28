'use strict';
const fs=require('node:fs'),crypto=require('node:crypto');
// Unfamiliar-site acceptance: the REAL home chatbox dispatches the REAL native
// task. Neither the browser action nor the independent verifier is mocked.
// Requires an isolated Browsphere instance with remote debugging enabled.
// LUCIA_TEST_GOAL and LUCIA_EXPECTED_HOSTS configure the task under test.
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const goal=process.env.LUCIA_TEST_GOAL;
if(!goal || goal.length>1300)throw Error('Supply a bounded LUCIA_TEST_GOAL');
function selectedModel(key,fallback){
  const value=process.env[key]||fallback;
  if(!/^[a-z0-9][a-z0-9._:/-]{0,119}$/i.test(value))throw Error('Invalid '+key);
  return value;
}
const actionModel=selectedModel('LUCIA_TEST_ACTION_MODEL','qwen2.5:3b-instruct');
const supervisorModel=selectedModel('LUCIA_TEST_SUPERVISOR_MODEL','lfm3:1.7b');
const expected=(process.env.LUCIA_EXPECTED_HOSTS||'').split(',').map(s=>s.trim().toLowerCase()).filter(Boolean);
const expectedSource=process.env.LUCIA_EXPECTED_SOURCE_URL||'';
const requireRecovery=process.env.LUCIA_REQUIRE_RECOVERY==='1';
const port=Number(process.env.LUCIA_CDP_PORT||9696);
const now=()=>new Date().toISOString();
// Bound each genuine model attempt below the whole-job deadline. This changes
// only test observability, never Lucia's native action or evidence policies.
const attemptBudgetMs=Number(process.env.LUCIA_TEST_ATTEMPT_TIMEOUT_MS||175000);
if(!Number.isInteger(attemptBudgetMs)||attemptBudgetMs<60000||attemptBudgetMs>280000)
  throw Error('Invalid unfamiliar-site attempt budget');
async function main(){
 let target;
 for(let i=0;i<100;i++){
   try{
     const pages=await(await fetch('http://127.0.0.1:'+port+'/json',{signal:AbortSignal.timeout(3500)})).json();
     target=pages.find(p=>p.type==='page'&&p.url?.includes('tauri.localhost'));
     if(target)break;
   }catch{}
   await sleep(300);
 }
 if(!target)throw Error('No native app UI exposed over CDP');
 const ws=new WebSocket(target.webSocketDebuggerUrl);
 await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
 let seq=0;const pending=new Map();
 ws.onmessage=e=>{
   const d=JSON.parse(e.data),p=pending.get(d.id);
   if(p){pending.delete(d.id);d.error?p.reject(Error(JSON.stringify(d.error))):p.resolve(d.result);}
 };
 async function evaluate(expression){
   const id=++seq;
   const value=await Promise.race([
     new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});
       ws.send(JSON.stringify({id,method:'Runtime.evaluate',
         params:{expression,awaitPromise:true,returnByValue:true,userGesture:true}}));}),
     sleep(17000).then(()=>{if(pending.has(id)){pending.delete(id);throw Error('CDP evaluation timeout');}})
   ]);
   if(value.exceptionDetails)throw Error(JSON.stringify(value.exceptionDetails));
   return value.result?.value;
 }
 let taskTab;
 try{
   let ready=false;
   for(let i=0;i<55;i++){
     ready=await evaluate("Boolean(window.aetherBridge?.startLuciaSpecialistTask&&window.__TAURI__?.core?.invoke&&document.getElementById('newTabChatInput'))").catch(()=>false);
     if(ready)break;await sleep(300);
   }
   if(!ready)throw Error('Native chat input is not ready');
   await evaluate(`(async()=>{
     window.__orchestratorProbe={launches:[],events:[]};
     const original=window.aetherBridge.startLuciaSpecialistTask;
     window.aetherBridge.startLuciaSpecialistTask=function(...args){
       const entry={goal:args[0],tab:args[3],runId:null};window.__orchestratorProbe.launches.push(entry);
       return Promise.resolve(original.apply(this,args)).then(runId=>{entry.runId=runId;return runId},error=>{entry.error=String(error);throw error});
     };
     for(const kind of ['agent:trace','agent:run_error','agent:run_status','agent:verification_report','agent:goal_progress','agent:objective_replan','agent:verification_recovery','agent:fact_dependencies','agent:document_artifact','agent:supervisor','agent:stage','agent:orchestration','agent:model_performance','agent:research_gate','agent:model_role','agent:research_evidence_proof'])
       await window.__TAURI__.event.listen(kind,event=>
         window.__orchestratorProbe.events.push({kind,payload:event.payload}));
     const config=await window.__TAURI__.core.invoke('get_router_config');
     config.prefer_local_for_simple=true;config.lucia_cloud_only=false;
     config.ollama_url='http://127.0.0.1:11434';
     config.ollama_model=${JSON.stringify(actionModel)};
     config.planner_model=${JSON.stringify(actionModel)};
     // The bundled/default writer tag may not be installed on the isolated
     // runner. The evidence passage selector must use this genuinely
     // installed, explicitly tested model, not a phantom alternate role.
     config.writer_model=${JSON.stringify(actionModel)};
     config.supervisor_model=${JSON.stringify(supervisorModel)};
     await window.__TAURI__.core.invoke('update_router_config',{config});
     return true;
   })()`);
   await evaluate(`(()=>{
     const input=document.getElementById('newTabChatInput');
     input.value=${JSON.stringify(goal)};
     input.dispatchEvent(new Event('input',{bubbles:true}));
     input.dispatchEvent(new KeyboardEvent('keydown',
       {key:'Enter',code:'Enter',bubbles:true,cancelable:true}));
     return true;
   })()`);
   const started=Date.now();
   let runId=null,finalStatus=null,lastCount=0,lastHeartbeat=0,lastGateCount=0;
   while(Date.now()-started<attemptBudgetMs){
     await sleep(1100);
     const d=await evaluate('({launches:window.__orchestratorProbe.launches,events:window.__orchestratorProbe.events.slice(-160)})');
     if(d.launches.length){runId=d.launches[0].runId;taskTab=d.launches[0].tab;}
     const evs=d.events.filter(e=>e.payload?.run_id===runId);
     const traces=evs.filter(e=>e.kind==='agent:trace');
     if(Date.now()-started-lastHeartbeat>=15000){
       lastHeartbeat=Date.now()-started;
       const modelEvents=evs.filter(e=>e.kind==='agent:model_performance');
       const latest=modelEvents.at(-1)?.payload||{};
       const stage=evs.filter(e=>e.kind==='agent:stage').at(-1)?.payload||{};
       console.log('LIVE_HEARTBEAT',JSON.stringify({
         elapsed_s:Math.round(lastHeartbeat/1000),runIdAssigned:Boolean(runId),
         traces:traces.length,stage:stage.specialist||null,
         model_calls:modelEvents.length,last_model_wall_ms:Number(latest.wall_ms)||null,
         last_model_timed_out:latest.timed_out===true,
         run_errors:evs.filter(e=>e.kind==='agent:run_error').length
       }));
     }
     if(traces.length>lastCount){
       for(const item of traces.slice(lastCount)){
         const e=item.payload;
         console.log('LIVE_ACTION',JSON.stringify({at:now(),action:e.action?.type,outcome:e.outcome,decision:e.decision,route:e.model_route}).slice(0,730));
       }
       lastCount=traces.length;
     }
     const gates=evs.filter(e=>e.kind==='agent:research_gate');
     for(const item of gates.slice(lastGateCount)){
       const e=item.payload;
       console.log('RESEARCH_GATE',JSON.stringify({step:e.step_index,stage:e.specialist,action:e.action,reason:e.reason_code,verifier_failure:e.verifier_failure_code,exact:e.exact_source_observed,sources:e.captured_sources,fresh:e.freshly_observed,previous_failed:e.previous_action_failed,recoveries:e.verification_recoveries}));
     }
     lastGateCount=gates.length;
     // Match statuses actually returned by the Rust loop_runner/main.rs.
     // In particular "unverified", "halted" and "max_steps" are FINISHED
     // failures, not live tasks; previously every such trial idled until its
     // 280-second deadline and the final sixth-run CI report was lost.
     const terminalStatuses=new Set([
       'completed','failed','cancelled','stopped','timed_out',
       'unverified','halted','max_steps','tab_closed','tab_missing'
     ]);
     const ended=evs.find(e=>e.kind==='agent:run_status'&&
       terminalStatuses.has(e.payload?.status));
     if(ended){
       finalStatus=ended.payload.status;
       console.log('LIVE_TERMINAL',JSON.stringify({
         elapsed_s:Math.round((Date.now()-started)/1000),
         status:finalStatus,observed_actions:traces.length
       }));
       break;
     }
     if(!runId&&d.launches.some(l=>l.error)){
       const error=String(d.launches.find(l=>l.error).error).slice(0,350);
       console.log('ADMISSION_DIAGNOSTIC',JSON.stringify({error}));
       throw Error('Native task admission rejected: '+error);
     }
     if(!runId&&Date.now()-started>40000){
       console.log('ADMISSION_DIAGNOSTIC',JSON.stringify({launches:d.launches.length,runId}));
       throw Error('No native task ID after 40 seconds');
     }
   }
   // Give the independent report channel a bounded chance to arrive after status.
   await sleep(800);
   const final=await evaluate('window.__orchestratorProbe');
   const evs=final.events.filter(e=>e.payload?.run_id===runId);
   const report=evs.filter(e=>e.kind==='agent:verification_report'&&e.payload.report?.status==='evidence_checked').at(-1)?.payload?.report;
   const errors=evs.filter(e=>e.kind==='agent:run_error').map(e=>e.payload.message);
   const attemptTimedOut=!finalStatus;
   if(attemptTimedOut)errors.push('No terminal status within bounded genuine-model attempt deadline');
   const replanCount=evs.filter(e=>e.kind==='agent:objective_replan').length;
   const supervisorReviews=evs.filter(e=>e.kind==='agent:supervisor'&&e.payload?.status==='reviewing').length;
   const stalledObservations=evs.filter(e=>e.kind==='agent:orchestration'&&e.payload?.stalled===true).length;
   const specialistStages=evs.filter(e=>e.kind==='agent:stage').map(e=>({name:e.payload?.specialist,status:e.payload?.status}));
   const failedActions=evs.filter(e=>e.kind==='agent:trace'&&e.payload?.outcome==='failed').length;
   // Numeric-only action timing from the real Rust runtime. Ollama's complete
   // response counters are logged by its native adapter; timeouts have no
   // response body and are reported by the caller's deadline event.
   const modelPerformance=evs.filter(e=>e.kind==='agent:model_performance')
     .map(e=>e.payload).slice(-12).map(p=>({
       wall_ms:Number(p.wall_ms)||0,budget_ms:Number(p.budget_ms)||0,
       prompt_chars:Number(p.prompt_chars)||0,timed_out:p.timed_out===true
     }));
   const verifierModels=[...new Set(evs.filter(e=>e.kind==='agent:model_role'&&e.payload?.role==='verifier').map(e=>e.payload.model).filter(Boolean))];
   const recoveryEvents=evs.filter(e=>e.kind==='agent:verification_recovery');
   const recoveryStages=recoveryEvents.map(e=>e.payload.status);
   const progress=evs.filter(e=>e.kind==='agent:goal_progress').map(e=>e.payload.progress).at(-1);
   const lastFactDependencies=evs.filter(e=>e.kind==='agent:fact_dependencies').map(e=>e.payload).at(-1);
   const factDependencyHistory=evs.filter(e=>e.kind==='agent:fact_dependencies').map(e=>({step:e.payload.step_index,missing:e.payload.missing,ready:e.payload.eligible_for_synthesis}));
   const hosts=[...new Set((report?.findings||[]).map(f=>{try{return new URL(f.url).hostname.toLowerCase().replace(/^www[.]/,'')}catch{return null}}).filter(Boolean))];
   const missingHosts=expected.filter(h=>!hosts.some(observed=>observed===h||observed.endsWith('.'+h)));
   const findings=report?.findings||[];
   // Routing a task to a configured local model is not evidence that the model
   // actually reasoned or acted. Require a confirmed, non-completion browser
   // action from the explicitly selected REAL Ollama model.
   const genuineModelActions=evs.filter(e=>e.kind==='agent:trace' &&
     e.payload?.action?.type!=='complete' &&
     ['ok','confirmed'].includes(e.payload?.outcome) &&
     String(e.payload?.model_route||'').includes('ollama:'+actionModel));
   // Unlike an action or model routing announcement, this proof is emitted
   // only after Qwen's passage selection and the independent native quote,
   // source and semantic checks all succeeded.
   const genuineResearchSelections=evs.filter(e=>
     e.kind==='agent:research_evidence_proof' &&
     e.payload?.model===actionModel &&
     e.payload?.status==='independently_checked_native_passages' &&
     Number(e.payload?.findings)>=1);
   const genuineModelUsed=genuineModelActions.length>0||genuineResearchSelections.length>0;
   // A recovery counts only when intervention follows a problem and the
   // independent evidence report arrives after that intervention.
   const problemIndex=evs.findIndex(e=>(e.kind==='agent:trace'&&e.payload?.outcome==='failed')||
     (e.kind==='agent:orchestration'&&e.payload?.stalled===true));
   const interventionIndex=problemIndex<0?-1:evs.findIndex((e,i)=>i>problemIndex&&
     (e.kind==='agent:objective_replan'||(e.kind==='agent:supervisor'&&e.payload?.status==='reviewing')));
   const evidenceIndex=evs.findLastIndex(e=>e.kind==='agent:verification_report'&&
     e.payload.report?.status==='evidence_checked');
   const recoveryVerified=problemIndex>=0&&interventionIndex>problemIndex&&
     evidenceIndex>interventionIndex&&finalStatus==='completed';
   // A guide's homepage is not proof that the requested deep source was read.
   // Accept the exact user-supplied path or a descendant page of that guide.
   const sourcePathVerified=!expectedSource||findings.some(f=>{
     try{
       const want=new URL(expectedSource),got=new URL(f.url);
       const path=pathname=>pathname.split('/').filter(Boolean).join('/');
       const prefix=path(want.pathname),actual=path(got.pathname);
       return got.origin===want.origin&&(actual===prefix||actual.startsWith(prefix+'/'));
     }catch{return false}
   });
   const requiresFree=goal.split(/[^A-Za-z0-9]+/).some(w=>w.toLowerCase()==='free');
   const paidUpgradeQuote=requiresFree&&findings.some(f=>
     /everything (?:in|from) free(?: forever)?,\s*(?:plus|and)|per user\s*(?:\/|per)\s*month|billed annually|billed yearly/i.test(f.quote||''));
   const paidSummary=requiresFree&&(/everything (?:in|from) free(?: forever)?,\s*(?:plus|and)|per user\s*(?:\/|per)\s*month|paid (?:plan|tier|upgrade)|premium (?:plan|tier)|[$€£₹]\s*[1-9]\d*/i.test(report?.summary||''));
   const missingFreeProof=requiresFree?expected.filter(host=>!findings.some(f=>{
     try{const actual=new URL(f.url).hostname.toLowerCase();return (actual===host||actual.endsWith('.'+host))&&/\bfree\b/i.test(f.quote)}catch{return false}
   })):[];
   const teamLimitNeeded=requiresFree&&/\b(team|members?)\b/i.test(goal);
   const capacityQuote=quote=>{
     const tokens=String(quote||'').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
     if(!tokens.includes('free')||/everything (?:in|from) free.*(?:plus|and)|per user\s*(?:\/|per)\s*month/i.test(quote))return false;
     return tokens.some((word,i)=>/^(member|collaborator|user|seat|guest|people|person)/.test(word)&&
       tokens.slice(Math.max(0,i-5),i+6).some(token=>(/^\d+$/.test(token)&&Number(token)>0)||
         ['unlimited','one','two','three','four','five','six','seven','eight','nine','ten','twenty'].includes(token)));
   };
   const missingCapacityProof=teamLimitNeeded?expected.filter(host=>!findings.some(f=>{
     try{const actual=new URL(f.url).hostname.toLowerCase().replace(/^www\./,'');return actual===host&&capacityQuote(f.quote)}catch{return false}
   })):[];
   const incompleteSummary=/need to compare|still need to compare|cannot determine|insufficient evidence|unclear whether|based on the free plan details/i.test(report?.summary||'');
   const requiresReport=/\b(?:write|create|prepare) a (?:research )?report\b/i.test(goal)&&/\b(?:research|sources|citations)\b/i.test(goal);
   const artifact=evs.filter(e=>e.kind==='agent:document_artifact'&&e.payload.status==='readback_verified').at(-1)?.payload?.artifact;
   let artifactReadbackVerified=false;
   if(requiresReport&&artifact){
     try{
       const raw=fs.readFileSync(artifact.path);
       const sha=crypto.createHash('sha256').update(raw).digest('hex');
       const text=raw.toString('utf8');
       artifactReadbackVerified=artifact.readback_verified===true&&artifact.source_count>=2
         &&sha===artifact.sha256&&raw.length===artifact.bytes
         &&findings.every(f=>text.includes(f.url)&&text.includes(
           String(f.quote).replaceAll('<','&lt;').replaceAll('>','&gt;').replace(/\r?\n/g,'\n> ')));
     }catch{};
   }
   const passed=final.launches.length===1&&final.launches[0].goal===goal&&Boolean(runId)
     &&finalStatus==='completed'&&Boolean(report)&&!missingHosts.length&&sourcePathVerified
     &&!paidUpgradeQuote&&!paidSummary&&!missingFreeProof.length&&!missingCapacityProof.length&&!incompleteSummary
     &&(!requiresReport||artifactReadbackVerified)
     &&findings.length>0&&findings.every(f=>f.url&&f.quote&&f.claim)
     &&(expectedSource?genuineResearchSelections.length>0:genuineModelUsed)
     &&(problemIndex<0||recoveryVerified)&&(!requireRecovery||recoveryVerified);
   console.log('ORCHESTRATOR_RESULT',JSON.stringify({passed,status:finalStatus||'timed_out',attemptTimedOut,attemptBudgetMs,runId,actions:evs.filter(e=>e.kind==='agent:trace').length,errors,replans:replanCount,supervisorReviews,stalledObservations,specialistStages,failedActions,modelPerformance,verifierModels,recoveryStages,requireRecovery,recoveryVerified,genuineModelUsed,genuineModelActions:genuineModelActions.length,genuineResearchSelections:genuineResearchSelections.length,progress,lastFactDependencies,factDependencyHistory:factDependencyHistory.slice(-8),findings:findings.length,hosts,missingHosts,expectedSource,sourcePathVerified,paidUpgradeQuote,paidSummary,missingFreeProof,missingCapacityProof,incompleteSummary,requiresReport,artifactReadbackVerified,summary:report?.summary?.slice(0,500)}));
   if(!passed)throw Error('Native task failed one or more end-to-end criteria');
 }finally{
   try{await evaluate("window.__TAURI__.core.invoke('cancel_agent_run')")}catch{}
   if(taskTab)try{await evaluate('window.browsphereApp.closeTab('+JSON.stringify(taskTab)+')')}catch{}
   ws.close();
 }
}
main().catch(e=>{console.error('ORCHESTRATOR_LIVE_FAIL',e.stack||String(e));process.exitCode=1});
