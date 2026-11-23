// Browsphere live website compatibility smoke test.
// Launch a disposable Browsphere instance with WebView2 remote debugging enabled,
// then set BROWSER_CDP_PORT (default 9240). Never use a personal browser profile.
import {setTimeout as sleep} from 'node:timers/promises';
const PORT = Number(process.env.BROWSER_CDP_PORT || 9240);
const BASE = `http://127.0.0.1:${PORT}/json`;
async function targets() {
  const response = await fetch(BASE);
  if (!response.ok) throw new Error(`Debugging endpoint returned ${response.status}`);
  return response.json();
}
class CDP {
  constructor(url) { this.url = url; this.pending = new Map(); this.id = 0; }
  async open() {
    this.socket = new WebSocket(this.url);
    await new Promise((ok, bad) => {
      this.socket.addEventListener('open', ok, {once:true});
      this.socket.addEventListener('error', bad, {once:true});
    });
    this.socket.onmessage = event => {
      const msg = JSON.parse(event.data);
      const callback = this.pending.get(msg.id);
      if (!callback) return;
      this.pending.delete(msg.id);
      msg.error ? callback.reject(Error(JSON.stringify(msg.error))) : callback.resolve(msg.result || {});
    };
    await this.call('Runtime.enable');
  }
  call(method, params={}) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, {resolve, reject});
      this.socket.send(JSON.stringify({id, method, params}));
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(Error('Timed out: ' + method));
        }
      }, 12000);
    });
  }
  async evaluate(expression) {
    const result = await this.call('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true, userGesture:true});
    if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails));
    return result.result?.value;
  }
  close() { try { this.socket?.close(); } catch (_) {} }
}
const results = [];
function record(name, success, detail='') {
  results.push({name, success:!!success, detail});
  console.log((success ? 'PASS ' : 'FAIL ') + name + ' ' + JSON.stringify(detail).slice(0, 550));
}
let main, child;
try {
  const starting = await targets();
  const app = starting.find(tab => tab.url === 'http://tauri.localhost/');
  if (!app) throw Error('Browsphere main view not found; launch the native test instance');
  main = new CDP(app.webSocketDebuggerUrl);
  await main.open();
  let shellReady = false;
  for (let attempt = 0; attempt < 75; attempt++) {
    shellReady = await main.evaluate('Boolean(window.aetherBridge?.isNative && window.browsphereApp?.navigate && window.browsphereSiteControls?.openCurrentSite)').catch(() => false);
    if (shellReady) break;
    await sleep(300);
  }
  if (!shellReady) throw Error('Native Browsphere shell initialization timed out');
  const initial = await main.evaluate('({native:window.aetherBridge?.isNative,controls:!!window.browsphereSiteControls,lock:!!document.getElementById("sitePermissionsButton")})');
  record('Native UI and site controls load', initial.native && initial.controls && initial.lock, initial);

  const sites = [
    ['Example Domain', 'https://example.com/'],
    ['GitHub', 'https://github.com/'],
    ['Wikipedia', 'https://www.wikipedia.org/'],
    ['Hacker News', 'https://news.ycombinator.com/']
  ];
  for (const [label, url] of sites) {
    await main.evaluate(`window.browsphereApp.navigate(${JSON.stringify(url)})`);
    let match;
    for (let attempt=0; attempt<9; attempt++) {
      await sleep(750);
      const list = await targets();
      match = list.find(tab => tab.type === 'page' && tab.url.startsWith(url.slice(0,-1)));
      if (match) break;
    }
    if (!match) {
      record(label + ' real WebView2 navigation', false, 'No matching child WebView2 page');
      continue;
    }
    child?.close();
    child = new CDP(match.webSocketDebuggerUrl);
    await child.open();
    const observedPage = '({url:location.href,title:document.title,ready:document.readyState,headings:document.querySelectorAll("h1").length})';
    const expectedHost = new URL(url).host;
    let page = await child.evaluate(observedPage);
    for (let attempt = 0; attempt < 25; attempt++) {
      // The CDP target URL can update before the renderer's old document is replaced.
      // Do not treat a previous website's title/content as the new site's evidence.
      const arrived = new URL(page.url).host === expectedHost;
      if (arrived && page.title.trim().length > 0 &&
          (page.ready !== 'loading' || page.headings > 0)) break;
      await sleep(350);
      page = await child.evaluate(observedPage);
    }
    const valid = new URL(page.url).host === expectedHost && page.title.trim().length > 0 &&
      (page.ready !== 'loading' || page.headings > 0);
    record(label + ' real WebView2 navigation', valid, {url:match.url.slice(0,100),...page});
    let chrome, synchronized = false;
    for (let attempt = 0; attempt < 20; attempt++) {
      chrome = await main.evaluate('({url:window.browsphereApp.activeTab().url,bar:document.getElementById("omnibarInput").value})');
      synchronized = new URL(chrome.url).host === new URL(url).host &&
        chrome.bar.includes(new URL(url).host);
      if (synchronized) break;
      // The child document can be ready before the main-tab navigation event
      // updates the visible address bar. Require convergence, not instant sync.
      await sleep(300);
    }
    record(label + ' address bar sync', synchronized, chrome);
  }

  await main.evaluate('window.browsphereApp.navigate("https://example.com/")');
  await sleep(1200);
  const permissions = await main.evaluate('window.browsphereSiteControls.openCurrentSite();true');
  await sleep(350);
  const state = await main.evaluate('({dialog:document.querySelector("#siteControlsOverlay")?.textContent,buttons:document.querySelectorAll("[data-site-permission]").length})');
  record('HTTPS origin permission panel opens', !!permissions && state.buttons === 4 && state.dialog.includes('example.com'), state);

  const startingPermission = await main.evaluate('window.__TAURI__.core.invoke("get_site_permissions",{origin:"https://example.com"})');
  record('Default denied permission grants', !startingPermission.granted.includes('camera'), startingPermission);
  await main.evaluate('document.querySelector("[data-site-permission=camera]").click()');
  await sleep(500);
  const enabled = await main.evaluate('window.__TAURI__.core.invoke("get_site_permissions",{origin:"https://example.com"})');
  record('User can grant one HTTPS site permission', enabled.granted.includes('camera'), enabled);
  await main.evaluate('document.querySelector("[data-site-permission=camera]").click()');
  await sleep(500);
  const revoked = await main.evaluate('window.__TAURI__.core.invoke("get_site_permissions",{origin:"https://example.com"})');
  record('User can revoke site permission', !revoked.granted.includes('camera'), revoked);
  const insecure = await main.evaluate('window.__TAURI__.core.invoke("set_site_permission",{origin:"http://example.com",permission:"camera",allowed:true}).then(()=>false,()=>true)');
  record('Native policy rejects insecure origin', insecure);
  const unsupported = await main.evaluate('window.__TAURI__.core.invoke("set_site_permission",{origin:"https://example.com",permission:"file-system-access",allowed:true}).then(()=>false,()=>true)');
  record('Native policy rejects unsupported sensitive permission', unsupported);

  await main.evaluate('document.getElementById("siteControlsOverlay")?.querySelector("[aria-label=\\\"Close site dialog\\\"]")?.click()');
  let example = (await targets()).find(tab => tab.url.startsWith('https://example.com/'));
  child?.close();
  if (example) {
    child = new CDP(example.webSocketDebuggerUrl);
    await child.open();
    const openResult = await child.evaluate('window.open("https://example.org/","_blank")===null');
    await sleep(850);
    const blocked = !(await targets()).some(tab => tab.url.startsWith('https://example.org/'));
    const queued = await main.evaluate('({visible:!!document.getElementById("siteControlsOverlay")&&!document.getElementById("siteControlsOverlay").classList.contains("hidden"),message:document.getElementById("siteControlsOverlay")?.textContent})');
    record('Popup blocked pending user consent', blocked && queued.visible && queued.message.includes('example.org'), {blocked, queued});
    await main.evaluate('Array.from(document.querySelectorAll("#siteControlsOverlay button")).find(button=>button.textContent.includes("Open once"))?.click()');
    await sleep(1700);
    const afterClick = await targets();
    record('Explicit Open once creates real browser tab', afterClick.some(tab => tab.url.startsWith('https://example.org/')), afterClick.map(tab => tab.url.slice(0,75)));
  } else record('Popup blocked pending user consent',false,'No example.com WebView');
} catch (error) {
  console.error('BROWSER_COMPAT_ERROR', error.stack || String(error));
  process.exitCode = 2;
} finally {
  child?.close();
  main?.close();
  console.log('BROWSER_COMPAT_RESULTS ' + results.filter(row => row.success).length + '/' + results.length);
  if (results.some(row => !row.success)) process.exitCode = 1;
}
