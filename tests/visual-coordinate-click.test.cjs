const test=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../src-tauri/src/agent/action_executor.rs'),'utf8');
const fn=source.slice(source.indexOf('fn build_click_script_at'),source.indexOf('fn build_type_script'));
const template=fn.split('r#"')[1].split('"#,')[0];
const point={point:{x:100,y:150},viewport:{width:800,height:500,scroll_x:0,scroll_y:0,scale:1,origin:'https://example.org/catalog'},bounds:{x:50,y:60,width:100,height:40,in_viewport:true}};
function simulate({visual=point,occluded=false,nested=false,stale=false,form=false}={}){
  const events=[];
  const el={tagName:'A',isConnected:true,matches:()=>false,getAttribute:()=>null,closest:s=>form&&s==='form'?{}:null,getBoundingClientRect:()=>({left:50,top:60,right:150,bottom:100,width:100,height:40}),scrollIntoView:()=>events.push({type:'scroll'}),dispatchEvent:e=>events.push(e),click:()=>events.push({type:'legacy-click'})};
  const child={matches:()=>nested,parentElement:el,dispatchEvent:e=>events.push(e)};
  const hit=occluded?{matches:()=>false,parentElement:null,getRootNode:()=>({})}:child;
  const doc={querySelector:()=>el,elementFromPoint:()=>hit};
  class Event {constructor(type,props){Object.assign(this,props,{type})}}
  const ctx={document:doc,CSS:{escape:x=>x},getComputedStyle:()=>({display:'block',visibility:'visible'}),location:{href:stale?'https://example.org/changed':point.viewport.origin},innerWidth:800,innerHeight:500,scrollX:0,scrollY:0,devicePixelRatio:1,PointerEvent:Event,MouseEvent:Event};
  // JSON substituted after brace unescaping to preserve nested objects.
  const safeScript=template.replaceAll('{{','{').replaceAll('}}','}').replaceAll('{request_id}','fixture').replaceAll('{id_json}',JSON.stringify('node')).replaceAll('{point_json}',JSON.stringify(visual));
  const result=vm.runInNewContext(safeScript,ctx);
  return {result,events};
}
test('visual point dispatch uses screenshot coordinates, not center or scrolling',()=>{
 const {result,events}=simulate();assert.equal(result.success,true);
 assert.equal(events.length,5);assert.equal(events.at(-1).type,'click');
 assert.equal(events.at(-1).clientX,80);assert.equal(events.at(-1).clientY,75);
});
test('rejects occluded, nested, stale or form targets with zero events',()=>{
 for(const options of [{occluded:true},{nested:true},{stale:true},{form:true},{visual:{...point,point:{x:900,y:160}}}]){
   const {result,events}=simulate(options);assert.equal(result.success,false,JSON.stringify(options));assert.equal(events.length,0);
 }
});
test('legacy DOM click still works without visual point',()=>{
 const {result,events}=simulate({visual:null});assert.equal(result.success,true);assert.equal(events[0].type,'scroll');assert.equal(events.at(-1).type,'legacy-click');
});
