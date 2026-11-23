'use strict';
// Exercise the production HTML renderer directly, without a browser account,
// private page content, or an external model. This is a UI test, not autonomy.
const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const activity = require('../ui/public/src/lucia-activity.js');
const source = fs.readFileSync(path.join(__dirname, '../ui/public/src/browser-shell.js'), 'utf8');
const start = source.indexOf('      function renderNewTabConversation() {');
const end = source.indexOf('      function renderNewTabHistoryDrawer()', start);
if (start < 0 || end < 0) throw Error('Lucia main chat renderer not found');

function render(thread) {
  const tabs = [{ id: 'tab-lucia-safe', history: ['browsphere://start'] }];
  const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({
    '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;', "'":'&#39;'
  }[c]));
  const context = {
    getActiveNewTabThread: () => thread,
    PRESET_PAGES: {'browsphere://start': {renderHtml: () => 'START PAGE'}},
    esc, tabs
  };
  vm.runInNewContext(source.slice(start, end) + ';globalThis.renderTask=renderNewTabConversation;', context);
  return context.renderTask();
}

test('main chat displays live phases and an executed step with its safe explanation', () => {
  const task = {
    runId:'test-run',status:'Running',plan:{stages:[
      {specialist:'browser',objective:'Navigate to YouTube',status:'active'},
      {specialist:'verification',objective:'Check the page',status:'pending'}
    ]},actions:1,evidence:0,tabId:'tab-lucia-safe',tabClosed:false,activityLog:[]
  };
  activity.append(task, activity.activity({phase:'observed',step_index:0,controls:120}));
  activity.append(task, activity.trace({
    action:{type:'click',element_label:'not safe to print',node_id:'secret-node'},
    step_index:0,outcome:'confirmed',active_origin:'https://www.youtube.com',
    model_route:'Browser Agent / litellm:oc/hy4-flash-free',
    thought:'raw hidden model reasoning confidential'
  }));
  const html = render({messages:[{role:'user',content:'Play a song'}],pending:true,agentTask:task});
  assert.match(html,/Lucia · Specialist Task/);
  assert.match(html,/Live activity/);
  assert.match(html,/Step 1/);
  assert.match(html,/Click observed control/);
  assert.match(html,/Page change confirmed/);
  assert.match(html,/oc\/hy4-flash-free/);
  assert.match(html,/https:\/\/www\.youtube\.com/);
  assert.match(html,/View task tab/);
  assert.match(html,/Actions and decision summaries, not private internal reasoning/);
  for (const privateText of ['raw hidden model reasoning','not safe to print','secret-node']) {
    assert.doesNotMatch(html, new RegExp(privateText));
  }
});

test('main chat shows waiting state before the first browser observation', () => {
  const html = render({
    messages:[],pending:true,agentTask:{
      status:'Running',plan:{stages:[]},actions:0,evidence:0,
      liveActivity:'Starting the browser task…',activityLog:[]
    }
  });
  assert.match(html,/Starting the browser task/);
  assert.match(html,/Waiting for live page observations/);
  assert.match(html,/Stop task/);
});
