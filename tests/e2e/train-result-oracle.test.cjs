const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {evaluateTrainResults}=require('./train-result-oracle.cjs');
const text=fs.readFileSync(path.join(__dirname,'fixtures/public-train-results-v13.txt'),'utf8').trim();
const url='https://www.irctc.co.in/nget/booking/train-list';
const report={limitations:'Seat availability was not independently verified.',findings:[
 {url,quote:'4 Results for KSR BENGALURU NEW DELHI | Wed, 25 Nov 2026',claim:'4 scheduled trains; 2 match.'},
 {url,quote:'KARNATAKA EXP (12627) Runs On: MTWTFSS Train Schedule 19:20 | KSR BENGALURU | Wed, 25 Nov 37:40 09:00 | NEW DELHI | Fri, 27 Nov',claim:'KARNATAKA EXP (12627) departs KSR BENGALURU at 19:20 on 2026-11-25 and arrives NEW DELHI at 09:00 on 2026-11-27'},
 {url,quote:'RAJDHANI EXP (22691) Runs On: MTWTFSS Train Schedule 20:00 | KSR BENGALURU | Wed, 25 Nov 33:30 05:30 | HAZRAT NIZAMUDDIN JN | Fri, 27 Nov',claim:'RAJDHANI EXP (22691) departs KSR BENGALURU at 20:00 on 2026-11-25 and arrives HAZRAT NIZAMUDDIN JN at 05:30 on 2026-11-27'}]};
const run=(t=text,r=report)=>evaluateTrainResults(t,r,'2026-11-25','Bengaluru','New Delhi',1080);
test('actual capture satisfies independent schedule checks',()=>{assert.deepEqual(Object.values(run()),Array(9).fill(true));});
test('missing train, wrong date, fabricated time and seat claim fail',()=>{
 assert.equal(run(text,{...report,findings:report.findings.slice(0,2)}).departureWindow,false);
 assert.equal(run(text.replace('Wed, 25 Nov 2026','Thu, 26 Nov 2026')).dateMatches,false);
 assert.equal(run(text.replace('05:30 | HAZRAT','05:31 | HAZRAT')).scheduleTimesMatch,false);
 assert.equal(run(text,{...report,limitations:'Confirmed seats available'}).availabilitySeparated,false);
 assert.equal(run(text.replace('4 Results for','5 Results for')).hasTrainList,false);
});
