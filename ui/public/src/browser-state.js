/* Browser-local session and library storage. No cookies, passwords, or private tabs. */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  if (root) root.BrowsphereState = api;
})(typeof window !== 'undefined' ? window : null, function () {
  'use strict';
  const SESSION_KEY = 'browsphere:session:v1';
  const LIBRARY_KEY = 'browsphere:library:v1';
  const START = 'browsphere://start';
  const SEARCH_ENGINES = Object.freeze({
    Google: 'https://www.google.com/search?q=',
    DuckDuckGo: 'https://duckduckgo.com/?q=',
    Perplexity: 'https://www.perplexity.ai/search?q=',
    Kagi: 'https://kagi.com/search?q='
  });
  function validAddress(raw) {
    if (typeof raw !== 'string' || raw.length > 2048) return null;
    const value = raw.trim();
    if (value === START || value === 'about:blank') return value;
    if (!value || /[\x00-\x20\x7f]/.test(value)) return null;
    try {
      const parsed = new URL(/^https?:\/\//i.test(value) ? value : 'https://' + value);
      if (!['http:', 'https:'].includes(parsed.protocol) || !parsed.hostname ||
          parsed.username || parsed.password) return null;
      return parsed.href;
    } catch (_) { return null; }
  }
  function addressOrSearch(raw, engine) {
    const value = String(raw || '').trim();
    if (!value) return null;
    if (value === START || value === 'about:blank') return value;
    if (/^browsphere:\/\/(?:settings|preferences)$/i.test(value)) return value.toLowerCase();
    const likelyAddress = /^(https?:\/\/|localhost(?::\d+)?(?:\/|$)|(?:[^.\s/:]+\.)+[^.\s/:]+(?::\d+)?(?:[/?#]|$))/i.test(value);
    if (likelyAddress) return validAddress(value);
    if (/^[a-z][\w+.-]*:/i.test(value)) return null;
    return (SEARCH_ENGINES[engine] || SEARCH_ENGINES.Google) + encodeURIComponent(value);
  }
  function canPersistAddress(address) {
    const url = validAddress(address);
    if (!url) return false;
    if (url === START || url === 'about:blank') return true;
    try {
      const parsed = new URL(url);
      const params = [parsed.searchParams, new URLSearchParams(parsed.hash.replace(/^#/, ''))];
      for (const group of params) {
        for (const key of group.keys()) {
          if (/^(?:access_token|id_token|refresh_token|code|token|secret|password|api_?key|client_secret|session|auth)$/i.test(key)) return false;
        }
      }
      return true;
    } catch (_) { return false; }
  }
  function cleanedTabs(tabs) {
    const result = [];
    const ids = new Set();
    for (const tab of Array.isArray(tabs) ? tabs.slice(0, 40) : []) {
      if (!tab || tab.privateContext || typeof tab.id !== 'string' ||
          !/^tab-[a-z0-9_-]{1,60}$/i.test(tab.id) || ids.has(tab.id)) continue;
      const original = (Array.isArray(tab.history) ? tab.history : []).slice(-40);
      const currentIndex = Math.min(original.length - 1,
        Math.max(0, Number.isInteger(tab.historyIndex) ? tab.historyIndex : original.length - 1));
      const selectedAddress = original[currentIndex];
      const history = original.filter(canPersistAddress).map(validAddress);
      if (!history.length) continue;
      const matchingIndex = history.indexOf(validAddress(selectedAddress));
      ids.add(tab.id);
      result.push({
        id: tab.id, group: String(tab.group || 'Open Tabs').slice(0, 64),
        history, historyIndex: matchingIndex >= 0 ? matchingIndex : history.length - 1
      });
      if (result.length === 30) break;
    }
    return result.length ? result : [{ id: 'tab-1', group: 'Open Tabs', history: [START], historyIndex: 0 }];
  }
  function cleanedCollection(entries, kind) {
    if (!Array.isArray(entries)) return [];
    const seen = new Set();
    const out = [];
    for (const item of entries.slice(0, 600)) {
      if (!item || !canPersistAddress(item.url)) continue;
      const url = validAddress(item.url);
      if (seen.has(url)) continue;
      seen.add(url);
      out.push(kind === 'bookmarks'
        ? { folder: String(item.folder || 'Favorites').slice(0, 64), title: String(item.title || url).slice(0, 256), url }
        : { title: String(item.title || url).slice(0, 256), url, readTime: String(item.readTime || '5 min').slice(0, 30), unread: !!item.unread });
      if (out.length >= 500) break;
    }
    return out;
  }
  function parse(storage, key) {
    try {
      const text = storage.getItem(key);
      return text && text.length < 1000000 ? JSON.parse(text) : null;
    } catch (_) { return null; }
  }
  function load(storage) {
    const snapshot = parse(storage, SESSION_KEY);
    const library = parse(storage, LIBRARY_KEY);
    const tabs = cleanedTabs(snapshot && snapshot.tabs);
    return {
      tabs,
      activeTabId: snapshot && tabs.some(t => t.id === snapshot.activeTabId) ? snapshot.activeTabId : tabs[0].id,
      bookmarks: cleanedCollection(library && library.bookmarks, 'bookmarks'),
      readingList: cleanedCollection(library && library.readingList, 'reading')
    };
  }
  function saveSession(storage, tabs, activeTabId, isPrivateMode) {
    if (isPrivateMode) return false;
    try {
      const safeTabs = cleanedTabs(tabs);
      storage.setItem(SESSION_KEY, JSON.stringify({
        version: 1, tabs: safeTabs,
        activeTabId: safeTabs.some(t => t.id === activeTabId) ? activeTabId : safeTabs[0].id
      }));
      return true;
    } catch (_) { return false; }
  }
  function saveLibrary(storage, bookmarks, readingList) {
    try {
      storage.setItem(LIBRARY_KEY, JSON.stringify({
        version: 1,
        bookmarks: cleanedCollection(bookmarks, 'bookmarks'),
        readingList: cleanedCollection(readingList, 'reading')
      }));
      return true;
    } catch (_) { return false; }
  }
  return { validAddress, addressOrSearch, canPersistAddress, cleanedTabs,
           load, saveSession, saveLibrary, SESSION_KEY, LIBRARY_KEY };
});
