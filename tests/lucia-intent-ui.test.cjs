'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');

const source = fs.readFileSync(
  path.join(__dirname, '..', 'ui/public/src/lucia-shell.js'), 'utf8'
);

function startShell(invokeResult) {
  const elements = new Map();
  const calls = [];
  function element() {
    return {
      innerHTML: '', value: '', children: [], dataset: {},
      classList: { add() {}, remove() {}, toggle() {} },
      addEventListener(name, handler) {
        (this.handlers ??= {})[name] = handler;
      },
      appendChild(child) { this.children.push(child); },
      remove() {},
    };
  }
  const document = {
    getElementById(id) {
      if (!elements.has(id)) elements.set(id, element());
      return elements.get(id);
    },
    createElement() { return element(); },
    addEventListener() {},
    documentElement: { setAttribute() {}, classList: { toggle() {} },
      getAttribute() { return 'false'; } },
  };
  const window = {
    __TAURI__: { core: { invoke: async (command, args) => {
      calls.push(command);
      return invokeResult(command, args);
    } } },
    dispatchEvent() {},
  };
  vm.runInNewContext(source, { document, window, URL, console, CustomEvent: class {} },
    { filename: 'lucia-shell.js' });

  return {
    calls,
    async send(prompt) {
      document.getElementById('chatPromptInput').value = prompt;
      document.getElementById('chatComposerForm')
        .handlers.submit({ preventDefault() {} });
      await new Promise(resolve => setImmediate(resolve));
    },
    get bubbles() {
      return document.getElementById('chatMessagesStream')
        .children.map(row => row.innerHTML).join('\n');
    }
  };
}

test('routing failure never silently turns an action into ordinary chat', async () => {
  const shell = startShell(command => {
    if (command === 'route_lucia_intent') throw new Error('endpoint timeout');
    throw new Error('must not call ' + command);
  });
  await shell.send('Play some music');
  assert.deepEqual(shell.calls, ['route_lucia_intent']);
  assert.match(shell.bubbles, /Lucia · Request failed/);
  assert.match(shell.bubbles, /No task was started/);
  assert.match(shell.bubbles, /endpoint timeout/);
});

test('invalid classifier result never starts a chat completion', async () => {
  const shell = startShell(command => {
    if (command === 'route_lucia_intent') return { mode: 'unknown' };
    throw new Error('must not call ' + command);
  });
  await shell.send('Play some music');
  assert.deepEqual(shell.calls, ['route_lucia_intent']);
  assert.match(shell.bubbles, /invalid task classification/);
});

test('valid chat intent remains a normal chat without browser actions', async () => {
  const shell = startShell(command => {
    if (command === 'route_lucia_intent') return { mode: 'chat' };
    if (command === 'ask_main_tab_ai') return { answer: 'Hi!', model: 'test' };
    throw new Error('unexpected: ' + command);
  });
  await shell.send('How are you?');
  assert.deepEqual(shell.calls, ['route_lucia_intent', 'ask_main_tab_ai']);
  assert.match(shell.bubbles, /Hi!/);
});

test('valid browser intent launches actual agent runner', async () => {
  const shell = startShell(command => {
    if (command === 'route_lucia_intent') return { mode: 'browser_task' };
    if (command === 'capture_live_snapshot') return {
      origin: 'about:blank', title: '', text_excerpt: '', interactive_nodes: [],
    };
    if (command === 'start_agent_run') return 'a-test-run';
    throw new Error('unexpected: ' + command);
  });
  await shell.send('Play music');
  assert.deepEqual(shell.calls, [
    'route_lucia_intent', 'capture_live_snapshot', 'start_agent_run',
  ]);
});
