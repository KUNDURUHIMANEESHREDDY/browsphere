// Actual Stagehand capability reuse, not a replica browser/agent implementation.
// Isolated ephemeral Chrome profile; never attaches to a user's private Tauri
// session or exposes WebView2 CDP. Only read-only methods are exported.
import {localBrowser,Stagehand} from '@browserbasehq/stagehand';
import {z} from 'zod/v4';
import {allowedResearchUrl} from './url-policy.mjs';
import {createOllamaModel} from './model-provider.mjs';

export async function stagehandRead({url,instruction,mode='inspect',model}, {ci=false,fixturePort=''}={}) {
  const target=allowedResearchUrl(url,{ci,fixturePort});
  if (!['inspect','observe','extract'].includes(mode)) throw Error('Read-only modes only');
  if (typeof instruction !== 'string' || instruction.length > 240) throw Error('Invalid research instruction');
  let browser,stagehand;
  try {
    browser=await localBrowser.launch({headless:true,ignoreHTTPSErrors:false,acceptDownloads:false});
    stagehand=await Stagehand.create({
      browser,
      model:createOllamaModel({model: model || 'qwen2.5:3b-instruct'})
    });
    const page=await browser.context.activePage();
    if (!page) throw Error('Stagehand has no browser page');
    await page.goto(target);
    const observedUrl=await page.url();
    if (new URL(observedUrl).origin !== new URL(target).origin) {
      throw Error('Untrusted cross-origin navigation refused');
    }
    if (mode==='inspect') {
      const title=await page.title();
      return {url:observedUrl,title:String(title).slice(0,200),verified:false,
        provenance:'Stagehand browser observation; no native Lucia evidence review'};
    }
    if (mode==='observe') {
      const out=await stagehand.observe(instruction,{timeout:40000});
      return {url:observedUrl,actions:out.data.slice(0,12).map(a=>({
        description:String(a.description||'').slice(0,180),
        method:String(a.method||'').slice(0,30),
        selector:String(a.selector||'').slice(0,500)
      })),verified:false,
        provenance:'Stagehand proposed actions only; no browser mutation or native approval'};
    }
    const out=await stagehand.extract(instruction,z.object({
      facts:z.array(z.object({quote:z.string(),source_url:z.string()})).max(8)
    }),{timeout:40000});
    return {url:observedUrl,facts:out.data.facts,verified:false,
      provenance:'Untrusted Stagehand extraction; Lucia must independently verify quotes'};
  } finally {
    // Stagehand manages its own isolated browser. Never close a user profile.
    try{await stagehand?.close()}catch{}
    try{await browser?.close()}catch{}
  }
}
