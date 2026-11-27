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
    // Stagehand v4 implements this using Chrome's own request-policy engine.
    // Enforce it BEFORE navigation: checking the final URL alone is too late
    // to prevent a cross-origin redirect or third-party subresource request.
    // Exact hostname, never wildcard/subdomains and never user-provided rules.
    await browser.context.setDomainPolicy({
      allowedDomains:[new URL(target).hostname]
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
      if (await page.url() !== observedUrl) throw Error('Source changed during research observation');
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
    if (await page.url() !== observedUrl) throw Error('Source changed during research extraction');
    // Model-supplied facts are still untrusted and not independent evidence.
    // Reject invented or off-source URLs before even displaying them as leads.
    const facts=out.data.facts;
    if (!Array.isArray(facts) || facts.some(f=>f.source_url!==observedUrl))
      throw Error('Extracted facts claim an unobserved source');
    return {url:observedUrl,facts,verified:false,
      provenance:'Untrusted Stagehand extraction; Lucia must independently verify quotes'};
  } finally {
    // Stagehand manages its own isolated browser. Never close a user profile.
    try{await stagehand?.close()}catch{}
    try{await browser?.close()}catch{}
  }
}
