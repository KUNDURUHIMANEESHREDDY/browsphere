'use strict';
// Evidence contract for R1 (report) and C1 (calendar). A fixture unit test is
// NOT a live demo pass: only native task-run receipts with readback qualify.
const fs=require('fs'),crypto=require('crypto'),path=require('path');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const web=url=>{try{const u=new URL(url);return u.protocol==='https:'&&!!u.hostname&&!u.username&&!u.password;}catch{return false;}};
function evaluate(kind,r){
 const errors=[];
 const need=(ok,msg)=>{if(!ok)errors.push(msg);};
 need(typeof r?.runId==='string'&&r.runId.length>=8,'Missing native run ID');
 need(r?.status==='completed'&&r?.verified===true,'Native task not independently verified');
 need(Array.isArray(r?.sources)&&r.sources.length>=2,'Two observed research sources required');
 if(Array.isArray(r?.sources)){
  need(new Set(r.sources.map(x=>x.url)).size>=2,'Duplicate research sources');
  for(const s of r.sources)need(web(s.url)&&typeof s.quote==='string'&&s.quote.length>=20,'Invalid observed source passage');
 }
 if(kind==='R1'){
  const output=r?.artifact;
  const file=output?.path;
  let actual='';
  if(typeof file==='string'&&fs.existsSync(file)&&fs.statSync(file).isFile())actual=fs.readFileSync(file,'utf8');
  need(!!actual,'Report artifact not found/readable');
  need(actual.length>=200,'Report is too short');
  need(hash(actual)===output?.sha256,'Saved report and readback checksum disagree');
  need(typeof output?.readback==='string'&&hash(output.readback)===hash(actual),'No independent report readback');
  for(const s of r?.sources||[])need(actual.includes(s.url),'Report omits a cited research source');
 }else if(kind==='C1'){
  const write=r?.calendar?.write,read=r?.calendar?.readback;
  need(!!write?.eventId&&write?.acknowledged===true,'Calendar create-event receipt missing');
  need(!!read?.eventId&&read.eventId===write?.eventId,'Calendar event ID readback mismatch');
  for(const field of ['title','start','end','timezone']){
   need(typeof write?.[field]==='string'&&write[field].length>0,'Missing event '+field);
   need(read?.[field]===write?.[field],'Calendar readback mismatch: '+field);
  }
  need(typeof read?.provider==='string'&&read.provider.length>0,'Calendar provider not identified');
  need(Array.isArray(r?.calendar?.audit)&&r.calendar.audit.some(x=>x.action==='create_event'&&x.eventId===write?.eventId)&&r.calendar.audit.some(x=>x.action==='get_event'&&x.eventId===write?.eventId),'MCP create/get audit receipts missing');
 }else need(false,'Unknown demo scenario');
 return {pass:errors.length===0,errors};
}
if(require.main===module){
 const [kind,file]=process.argv.slice(2);
 if(!kind||!file){console.error('Usage: node demo-output-contract.cjs R1|C1 receipt.json');process.exit(2);}
 const r=JSON.parse(fs.readFileSync(file,'utf8')),out=evaluate(kind,r);
 console.log('DEMO_OUTPUT_GATE',JSON.stringify({case:kind,...out}));
 process.exit(out.pass?0:1);
}
module.exports={evaluate};
