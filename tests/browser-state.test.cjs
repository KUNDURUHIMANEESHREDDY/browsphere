const assert = require('node:assert/strict');
const browser = require('../ui/public/src/browser-state.js');

function storage() {
  const data = new Map();
  return {
    getItem: key => data.has(key) ? data.get(key) : null,
    setItem: (key, value) => data.set(key, value),
    data
  };
}
const { validAddress, addressOrSearch, canPersistAddress,
  saveSession, saveLibrary, load, SESSION_KEY } = browser;

assert.equal(validAddress('example.com/a?b=c'), 'https://example.com/a?b=c');
assert.equal(validAddress('http://localhost:3000/'), 'http://localhost:3000/');
assert.equal(validAddress('browsphere://start'), 'browsphere://start');
assert.equal(validAddress('javascript:alert(1)'), null);
assert.equal(validAddress('https://user:pass@example.com'), null);
assert.equal(validAddress('https://a.example/ bad'), null);
assert.equal(addressOrSearch('what is a browser?', 'DuckDuckGo'),
  'https://duckduckgo.com/?q=what%20is%20a%20browser%3F');
assert.equal(addressOrSearch('https://example.org/post', 'Google'), 'https://example.org/post');
assert.equal(addressOrSearch('file:///etc/passwd', 'Google'), null);
assert.equal(canPersistAddress('https://example.com/callback?access_token=secret'), false);
assert.equal(canPersistAddress('https://example.com/#access_token=secret'), false);
assert.equal(canPersistAddress('https://example.com/path?category=docs'), true);

const store = storage();
const initial = [
  { id: 'tab-1', group: 'Open Tabs', history: ['browsphere://start', 'https://example.com/article'], historyIndex: 1 },
  { id: 'tab-private', group: 'Open Tabs', history: ['https://private.example/'], historyIndex: 0, privateContext: true },
  { id: 'tab-3', group: 'Open Tabs', history: ['https://site.example/?token=confidential'], historyIndex: 0 }
];
assert.equal(saveSession(store, initial, 'tab-1', false), true);
let restored = load(store);
assert.deepEqual(restored.tabs.map(t => t.id), ['tab-1']);
assert.equal(restored.tabs[0].history[1], 'https://example.com/article');
assert.equal(restored.activeTabId, 'tab-1');

const frozen = store.getItem(SESSION_KEY);
assert.equal(saveSession(store, [{ id: 'tab-private', history: ['https://sensitive.example'], historyIndex: 0 }], 'tab-private', true), false);
assert.equal(store.getItem(SESSION_KEY), frozen, 'private mode must not overwrite normal session');

assert.equal(saveLibrary(store, [
  { url: 'https://example.com', title: 'Example', folder: 'Favorites' },
  { url: 'https://example.com', title: 'Duplicate' },
  { url: 'javascript:alert(1)', title: 'Unsafe' }
], [{ url: 'https://docs.example/page', title: 'Docs', unread: true }]), true);
restored = load(store);
assert.equal(restored.bookmarks.length, 1);
assert.equal(restored.readingList.length, 1);
assert.equal(restored.readingList[0].unread, true);
store.setItem(SESSION_KEY, '{broken');
restored = load(store);
assert.equal(restored.tabs.length, 1);
assert.equal(restored.tabs[0].history[0], 'browsphere://start');

console.log('Browser persistence, URL normalization and private-session regression checks passed.');
