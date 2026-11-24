const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const vm=require('node:vm');
const source=fs.readFileSync('src-tauri/src/agent/dom_extractor.rs','utf8');
const start=source.indexOf('  function publicSearchProduct(el) {{');
const end=source.indexOf('  function productLinkLabel(el) {{',start);
assert.ok(start>=0&&end>start);
const fn=source.slice(start,end).replaceAll('{{','{').replaceAll('}}','}');
const target='https://www.croma.com/apple-macbook-air-m4/p/123';
function resolver(origin='https://www.google.com'){
 const ctx={URL,location:{origin,pathname:'/search'},isVisible:e=>!e.hidden};
 vm.createContext(ctx);vm.runInContext(fn,ctx);
 return (href,hidden=false)=>ctx.publicSearchProduct({href,hidden,closest:()=>null});
}
test('public search destination is recovered from direct or single explicit wrapper',()=>{
 const r=resolver();assert.equal(r(target),target);
 assert.equal(r('https://www.google.com/url?q='+encodeURIComponent(target)),target);
 assert.equal(r('https://www.google.com/goto?url='+encodeURIComponent(target)+'&gsessionid=redacted'),target);
 assert.equal(resolver('https://search.brave.com')(target),target);
 assert.equal(resolver('https://search.brave.com')('https://search.brave.com/go/v1?url='+encodeURIComponent(target)),target);
 assert.equal(resolver('https://search.brave.com')('https://search.brave.com/go/v1?url=opaque'),null);
 assert.equal(resolver('https://search.brave.com')('https://search.brave.com/go/v1?url='+encodeURIComponent(target)+'&url='+encodeURIComponent(target)),null);
});
test('opaque, ambiguous, secret-bearing, external, hidden and nonproduct destinations fail closed',()=>{
 const r=resolver();
 for(const u of ['https://www.google.com/goto?url=opaqueToken','https://www.google.com/url?q='+encodeURIComponent(target)+'&url='+encodeURIComponent(target),target+'?token=private',target+'#private','https://www.croma.com.evil.example/a/p/1','https://www.croma.com/category/macbook','https://user:pass@www.croma.com/a/p/1','http://www.croma.com/a/p/1'])assert.equal(r(u),null,u);
 assert.equal(r(target,true),null);assert.equal(resolver('https://example.org')(target),null);
});
