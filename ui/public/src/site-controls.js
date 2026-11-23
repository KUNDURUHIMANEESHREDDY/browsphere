/* Site controls for real WebView2 permissions and one-time popups.
   No site is ever granted permissions or a new window automatically. */
(function () {
  'use strict';
  const invoke = window.__TAURI__?.core?.invoke;
  const TYPES = Object.freeze([
    ['camera', 'Camera'],
    ['microphone', 'Microphone'],
    ['geolocation', 'Location'],
    ['notifications', 'Notifications']
  ]);
  let overlay = null;
  let popupRequest = null;
  let currentOrigin = null;
  let temporarilyHiddenTab = null;

  function activeTab() {
    return window.browsphereApp?.activeTab?.() || null;
  }
  function safeOrigin(raw) {
    try {
      const url = new URL(raw);
      return url.protocol === 'https:' && url.hostname && !url.username && !url.password
        ? url.origin : null;
    } catch (_) { return null; }
  }
  function safePopupUrl(raw) {
    try {
      const url = new URL(raw);
      if (!['https:', 'http:'].includes(url.protocol)
          || !url.hostname || url.username || url.password
          || url.href.length > 2048) return null;
      return url.href;
    } catch (_) { return null; }
  }
  function el(tag, className, textContent) {
    const element = document.createElement(tag);
    if (className) element.className = className;
    if (textContent !== undefined) element.textContent = textContent;
    return element;
  }
  const primary = 'rounded-lg bg-[#007AFF] text-white text-xs px-3 py-2 font-semibold';
  const secondary = 'rounded-lg bg-black/5 dark:bg-white/10 text-xs px-3 py-2';
  function close() {
    if (overlay) overlay.classList.add('hidden');
    popupRequest = null;
    const previous = temporarilyHiddenTab;
    temporarilyHiddenTab = null;
    const active = activeTab();
    if (previous && active && active.id === previous.id) {
      // Native child WebViews otherwise paint over CSS modals in the chrome.
      window.aetherBridge?.onSwitchTab?.(active.id, active.url);
    }
  }
  function shell(title, subtitle) {
    if (!overlay) {
      overlay = el('div', 'fixed inset-0 z-[100] flex items-center justify-center p-3 sm:p-6 bg-black/50');
      overlay.id = 'siteControlsOverlay';
      overlay.setAttribute('role', 'dialog');
      overlay.setAttribute('aria-modal', 'true');
      overlay.addEventListener('click', event => { if (event.target === overlay) close(); });
      document.body.appendChild(overlay);
    }
    overlay.replaceChildren();
    overlay.classList.remove('hidden');
    overlay.style.zIndex = '10000';
    const active = activeTab();
    if (active && /^https?:\/\//i.test(active.url) && window.aetherBridge?.isNative) {
      temporarilyHiddenTab = {id: active.id};
      window.aetherBridge.onSwitchTab(active.id, 'browsphere://start');
    }
    const body = el('div', 'w-full rounded-xl shadow-xl border border-black/10 dark:border-white/10 p-5 space-y-3');
    // Fixed dialog dimensions/colors do not depend on a regenerated Tailwind bundle.
    body.style.maxWidth = '460px';
    const dark = document.documentElement.getAttribute('data-theme') === 'dark';
    body.style.backgroundColor = dark ? '#24242A' : '#FFFFFF';
    body.style.color = dark ? '#FFFFFF' : '#1D1D1F';
    const heading = el('div', 'flex items-start justify-between gap-4');
    const info = el('div');
    info.appendChild(el('h2', 'text-[17px] font-semibold', title));
    info.appendChild(el('p', 'text-xs opacity-70 mt-1 break-all', subtitle));
    heading.appendChild(info);
    const dismiss = el('button', secondary, 'Close');
    dismiss.type = 'button';
    dismiss.setAttribute('aria-label', 'Close site dialog');
    dismiss.addEventListener('click', close);
    heading.appendChild(dismiss);
    body.appendChild(heading);
    overlay.appendChild(body);
    return body;
  }
  async function openCurrentSite() {
    const tab = activeTab();
    const origin = tab ? safeOrigin(tab.url) : null;
    const body = shell('Website permissions', origin || 'Open an HTTPS website to manage its permissions');
    if (!origin || !invoke) {
      body.appendChild(el('p', 'text-xs opacity-65', 'Permissions can only be managed for an HTTPS page in the native Windows application.'));
      return;
    }
    if (tab.private) {
      body.appendChild(el('p', 'text-xs opacity-70', 'Private tabs deny sensitive site permissions. They do not inherit or save ordinary browsing grants.'));
      return;
    }
    currentOrigin = origin;
    const explanation = el('p', 'text-xs opacity-70', 'All permissions are blocked by default. Enabling one grants this HTTPS site access on subsequent requests. Reload the site after changing permissions.');
    body.appendChild(explanation);
    const list = el('div', 'divide-y divide-black/10 dark:divide-white/10 rounded-xl border border-black/10 dark:border-white/10');
    body.appendChild(list);
    const status = el('p', 'text-xs opacity-65', 'Loading saved permissions…');
    body.appendChild(status);
    try {
      const saved = await invoke('get_site_permissions', {origin});
      if (currentOrigin !== origin || !overlay || overlay.classList.contains('hidden')) return;
      status.textContent = 'Only these four permission types can be enabled. Other sensitive requests remain blocked.';
      for (const [key, title] of TYPES) {
        const row = el('div', 'flex justify-between items-center gap-3 px-3 py-2.5');
        row.appendChild(el('span', 'text-xs font-medium', title));
        const button = el('button', secondary, saved.granted.includes(key) ? 'Allowed' : 'Blocked');
        button.type = 'button';
        button.dataset.sitePermission = key;
        button.setAttribute('aria-pressed', String(saved.granted.includes(key)));
        button.addEventListener('click', async () => {
          if (!activeTab() || safeOrigin(activeTab().url) !== origin || activeTab().private) {
            status.textContent = 'Site changed. Reopen this panel to change permissions.';
            return;
          }
          const allow = button.getAttribute('aria-pressed') !== 'true';
          button.disabled = true;
          try {
            const next = await invoke('set_site_permission', {origin, permission:key, allowed:allow});
            const enabled = next.granted.includes(key);
            button.textContent = enabled ? 'Allowed' : 'Blocked';
            button.setAttribute('aria-pressed', String(enabled));
            status.textContent = 'Saved. Reload the site for the change to take effect.';
          } catch (error) {
            status.textContent = 'Could not save permission: ' + String(error);
          } finally { button.disabled = false; }
        });
        row.appendChild(button);
        list.appendChild(row);
      }
      const reload = el('button', primary, 'Reload this site');
      reload.type = 'button';
      reload.addEventListener('click', () => {
        const latest = activeTab();
        if (latest && safeOrigin(latest.url) === origin && window.aetherBridge?.reloadTab) {
          window.aetherBridge.reloadTab(latest.id).catch(() => {});
        }
        close();
      });
      body.appendChild(reload);
    } catch (error) {
      status.textContent = 'Site permissions unavailable: ' + String(error);
    }
  }
  function showPopupRequest(detail) {
    const requestUrl = safePopupUrl(detail?.url);
    const tab = activeTab();
    if (!requestUrl || !tab || tab.id !== detail.tabId) return;
    popupRequest = {tabId: detail.tabId, url: requestUrl};
    const body = shell('Website requested a new tab', 'Browsphere blocked the popup until you approve it.');
    body.appendChild(el('p', 'text-xs opacity-70', 'Requested destination:'));
    body.appendChild(el('p', 'text-xs font-mono break-all select-text', requestUrl));
    const actions = el('div', 'flex justify-end gap-2');
    const block = el('button', secondary, 'Block');
    block.type = 'button';
    block.addEventListener('click', close);
    const open = el('button', primary, 'Open once in new tab');
    open.type = 'button';
    open.addEventListener('click', () => {
      const selected = popupRequest;
      const current = activeTab();
      if (selected && current && current.id === selected.tabId && safePopupUrl(selected.url)) {
        window.browsphereApp.newTab(selected.url);
      }
      close();
    });
    actions.append(block, open);
    body.appendChild(actions);
  }
  function init() {
    document.getElementById('sitePermissionsButton')?.addEventListener('click', openCurrentSite);
    window.addEventListener('aether:popup-blocked', event => showPopupRequest(event.detail));
    document.addEventListener('keydown', event => {
      if (event.key === 'Escape' && overlay && !overlay.classList.contains('hidden')) close();
    });
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
  window.browsphereSiteControls = {openCurrentSite};
})();
