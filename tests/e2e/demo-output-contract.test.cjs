'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('fs'),os=require('os'),path=require('path'),crypto=require('crypto');
const {evaluate}=require('./demo-output-contract.cjs');
const sha=s=>crypto.createHash('sha256').update(s).digest('hex');
function base(){return {runId:'native-run-12345',status:'completed',verified:true,
 sources:[{url:'https://docs.python.org/3/',quote:'Official Python documentation introduces the Python programming language.'},
 {url:'https://developer.mozilla.org/en-US/',quote:'MDN publishes current web development documentation for developers.'}]};}
test('R1 requires actual file, citations, independently identical readback',t=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'lucia-report-contract-'));
 t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
 const text='Report\n'+('Research comparison and implementation context. '.repeat(8))
 +'\nhttps://docs.python.org/3/\nhttps://developer.mozilla.org/en-US/';
 const file=path.join(dir,'report.md');fs.writeFileSync(file,text);
 const r={...base(),artifact:{path:file,sha256:sha(text),readback:text}};
 assert.equal(evaluate('R1',r).pass,true);
 r.artifact.readback='edited copy';
 assert.equal(evaluate('R1',r).pass,false);
 r.artifact.readback=text;r.sources[0].url='https://different.example/';
 assert.equal(evaluate('R1',r).pass,false);
});
test('C1 requires actual create and get-event receipts with matching event',()=>{
 const b=base(),event={eventId:'evt-01',title:'Work review',start:'2026-12-01T10:00:00+05:30',end:'2026-12-01T11:00:00+05:30',timezone:'Asia/Kolkata'};
 const r={...b,calendar:{write:{...event,acknowledged:true},readback:{...event,provider:'fixture-calendar'},audit:[{action:'create_event',eventId:'evt-01'},{action:'get_event',eventId:'evt-01'}]}};
 assert.equal(evaluate('C1',r).pass,true);
 r.calendar.readback.title='Wrong title';
 assert.equal(evaluate('C1',r).pass,false);
 r.calendar.readback.title='Work review';r.calendar.audit.pop();
 assert.equal(evaluate('C1',r).pass,false);
});
