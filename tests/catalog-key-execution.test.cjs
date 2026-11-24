const test=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const vm=require("node:vm");
const src=fs.readFileSync("src-tauri/src/agent/action_executor.rs","utf8");
const a=src.indexOf("fn build_key_script_mode("),b=src.indexOf("fn build_select_script(",a);
const raw=src.slice(a,b).match(/format!\(r#"([\s\S]*?)"#,/)[1];
const origin="https://www.croma.com/",query="apple macbook air m4";
function script(scope){return raw.replace("{id_json}",JSON.stringify("search")).replace("{key_json}",JSON.stringify("Enter")).replace("{scope}",JSON.stringify(scope)).replaceAll("{{","{").replaceAll("}}","}");}
function fixture({value=query,href=origin,blocked=false,form=null}={}){
 let events=[],focused=false;
 const rect={width:100,height:30,left:10,top:20};
 const el={tagName:"INPUT",value,isConnected:true,readOnly:false,getBoundingClientRect:()=>rect,
 matches:()=>false,getAttribute:k=>({role:"searchbox",type:"search"}[k]||null),closest:()=>form,
 focus(){focused=true},getRootNode:()=>({activeElement:focused?el:null}),dispatchEvent:e=>{events.push(e.type);return true;}};
 const document={querySelector:()=>el,elementFromPoint:()=>blocked?{}:el};
 const ctx={document,CSS:{escape:x=>x},location:{href,origin:new URL(href).origin},URL,innerWidth:1000,innerHeight:600,getComputedStyle:()=>({display:"block",visibility:"visible"}),KeyboardEvent:class{constructor(type){this.type=type}},Event:class{}};
 return {ctx,events};
}
test("generated native search script parses and validates exact query without synthetic dispatch",()=>{
 const f=fixture();const r=vm.runInNewContext(script([origin,query]),f.ctx);
 assert.equal(r.success,true);assert.deepEqual(f.events,[]);
});
test("native search preparation rejects wrong query, wrong URL and occlusion",()=>{
 for(const options of [{value:"wrong"},{href:"https://www.croma.com/other"},{blocked:true}]){
  const f=fixture(options),r=vm.runInNewContext(script([origin,query]),f.ctx);
  assert.equal(r.success,false);assert.deepEqual(f.events,[]);
 }
});
test("legacy Enter still parses and dispatches only on a nonempty search",()=>{
 const f=fixture();assert.equal(vm.runInNewContext(script(null),f.ctx).success,true);assert.deepEqual(f.events,["keydown","keyup"]);
 const blank=fixture({value:""});assert.equal(vm.runInNewContext(script(null),blank.ctx).success,false);assert.deepEqual(blank.events,[]);
});
