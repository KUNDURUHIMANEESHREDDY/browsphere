// Independent acceptance oracle: parse captured visible schedule text, not
// runtime classification or the Rust verifier's own report.
const norm=s=>String(s||'').replace(/\s+/g,' ').trim();
const esc=s=>s.replace(/[.*+?^\${}()|[\]\\]/g,'\\$&');
function evaluateTrainResults(excerpt,report,date,from,to,minMinutes){
 const text=norm(excerpt), journey=new Date(date+'T00:00:00Z');
 if(!/^\d{4}-\d{2}-\d{2}$/.test(date)||Number.isNaN(+journey)||journey.toISOString().slice(0,10)!==date)throw Error('Invalid journey date');
 const header=text.match(/\b(\d+) Results for ([^|]+)\| ([^|]+?) For Quota\b/i);
 const count=header?Number(header[1]):0;
 const routeMatches=!!header&&new RegExp('\\b'+esc(from)+'\\b.*\\b'+esc(to)+'\\b','i').test(header[2]);
 const [year,month,day]=date.split('-');
 const weekday=journey.toLocaleString('en-US',{weekday:'short',timeZone:'UTC'});
 const monthName=journey.toLocaleString('en-US',{month:'short',timeZone:'UTC'});
 const dateMatches=!!header&&new RegExp('^'+weekday+', 0?'+Number(day)+' '+monthName+' '+year+'$','i').test(header[3].trim());
 const start=text.indexOf('Previous DayNext Day');
 const body=start<0?'':text.slice(start+'Previous DayNext Day'.length);
 const cards=body.split(/Book NowOTHER DATES|OTHER DATES/).slice(0,-1);
 const rows=cards.map(card=>norm(card).match(/^([\w .&/-]+?) \((\d{5})\) Runs On: \S+ Train Schedule ((?:[01]\d|2[0-3]):[0-5]\d) \| ([^|]+?) \| (\w{3}, \d{1,2} \w{3}) (\d{1,3}:[0-5]\d) ((?:[01]\d|2[0-3]):[0-5]\d) \| ([^|]+?) \| (\w{3}, \d{1,2} \w{3})(?: |$)/));
 const minutes=s=>{const [h,m]=s.split(':').map(Number);return h*60+m;};
 const fromList=text.match(/FROM STATIONS Select All (.*?) TO STATIONS Select All (.*?) \d+ Results for/i);
 const stationMatches=!!fromList&&rows.every(r=>r&&new RegExp('\\b'+esc(r[4].replace(/\.$/,''))+'\\b','i').test(fromList[1])&&new RegExp('\\b'+esc(r[8].replace(/\.$/,''))+'\\b','i').test(fromList[2]));
 const scheduleTimesMatch=rows.every(r=>{
   if(!r)return false;
   const departureDay=weekday+', '+Number(day)+' '+monthName;
   const elapsed=minutes(r[6]);
   if(elapsed<=0||elapsed>240*60||r[5].replace(/, 0/ ,', ')!==departureDay)return false;
   const expected=new Date(+journey+(minutes(r[3])+elapsed)*60000);
   const arrDate=expected.toLocaleString('en-US',{weekday:'short',timeZone:'UTC'})+', '+expected.getUTCDate()+' '+expected.toLocaleString('en-US',{month:'short',timeZone:'UTC'});
   return expected.toISOString().slice(11,16)===r[7]&&r[9].replace(/, 0/,', ')===arrDate;
 });
 const hasTrainList=count>0&&rows.length===count&&rows.every(Boolean)&&new Set(rows.filter(Boolean).map(r=>r[2])).size===count&&stationMatches&&scheduleTimesMatch;
 const eligible=rows.filter(r=>r&&minutes(r[3])>=minMinutes);
 const findings=Array.isArray(report?.findings)?report.findings:[];
 const rowFindings=findings.filter(f=>/\(\d{5}\)/.test(f.quote||''));
 const exactQuotes=findings.length>0&&findings.every(f=>norm(f.quote).length>=12&&text.includes(norm(f.quote)));
 const departureWindow=hasTrainList&&rowFindings.length===eligible.length&&eligible.every(r=>rowFindings.some(f=>{
   const q=norm(f.quote),claim=norm(f.claim);
   return q===norm(r[0])&&[r[1],r[2],r[3],r[4],r[7],r[8],date].every(s=>claim.includes(s));
 }));
 const correctSource=findings.length>0&&findings.every(f=>{try{const u=new URL(f.url);return u.protocol==='https:'&&u.hostname==='www.irctc.co.in'&&u.pathname==='/nget/booking/train-list';}catch{return false;}});
 const availabilitySeparated=/seat availability was not independently verified/i.test(report?.limitations||'')&&!/\b(?:available seats|seats available|confirmed seats)\b/i.test(rowFindings.map(f=>f.claim).join(' '));
 return {hasTrainList,routeMatches,dateMatches,stationMatches,scheduleTimesMatch,departureWindow,exactQuotes,correctSource,availabilitySeparated};
}
module.exports={evaluateTrainResults};
