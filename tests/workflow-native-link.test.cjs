const test=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../src-tauri/src/agent/action_executor.rs'),'utf8');
const fn=source.slice(source.indexOf('fn workflow_link_script'),source.indexOf('pub async fn execute_workflow_link'));
const template=fn.split('r#"')[1].split('"#)')[0];
const expected={id:'node',source:'https://example.org/',dest:'https://example.org/guide',label:'Read guide'};
function simulate(options={}){
  const clicks=[];
  const el={tagName:options.tag||'A',isConnected:!options.detached,href:options.href||expected.dest,
    target:options.target||'',innerText:options.label||expected.label,title:'',
    hasAttribute:key=>(options.attributes||[]).includes(key),getAttribute:()=>null,
    closest:()=>options.hidden?{}:null,matches:()=>Boolean(options.disabled),
    getBoundingClientRect:()=>options.offscreen?{left:900,right:1000,top:20,bottom:50,width:100,height:30}:{left:20,right:120,top:20,bottom:50,width:100,height:30},
    contains:()=>!options.occluded,click:()=>clicks.push('click')};
  const hit=options.occluded?{}:options.nested?{closest:()=>({})}:el;
  const context={document:{querySelectorAll:()=>options.duplicate?[el,el]:[el],elementFromPoint:()=>hit},
    CSS:{escape:x=>x},location:{href:options.source||expected.source},
    innerWidth:800,innerHeight:500,getComputedStyle:()=>({visibility:options.invisible?'hidden':'visible',display:'block',pointerEvents:'auto'})};
  const script=template.replaceAll('{{','{').replaceAll('}}','}').replaceAll('{data}',JSON.stringify(expected));
  return {result:vm.runInNewContext(script,context),clicks};
}
test('dispatches exactly one freshly verified native link click',()=>{
  const {result,clicks}=simulate();assert.equal(result.success,true);assert.deepEqual(clicks,['click']);
});
test('rejects stale, changed, hidden, occluded, duplicate or unsafe link without dispatch',()=>{
  for(const options of [{source:'https://example.org/else'},{href:'https://example.org/else'},{label:'Other'},
      {tag:'BUTTON'},{detached:true},{target:'_blank'},{attributes:['download']},{attributes:['onclick']},
      {hidden:true},{disabled:true},{offscreen:true},{invisible:true},{occluded:true},{nested:true},{duplicate:true}]){
    const {result,clicks}=simulate(options);
    assert.equal(result.success,false,JSON.stringify(options));assert.equal(clicks.length,0,JSON.stringify(options));
  }
});
