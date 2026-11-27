// Optional local browser check: node router/tests/admin-browser.mjs <Chromium binary> [screenshot directory]
// The production page and assets run under CSP against an isolated loopback API fixture.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {readFile, mkdtemp, rm, writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {setTimeout as sleep} from 'node:timers/promises';

const [chromium, shots] = process.argv.slice(2);
assert.ok(chromium, 'Pass a local Chromium executable');
const assets = await Promise.all(['html','css','js'].map(ext => readFile(new URL(`../src/admin.${ext}`, import.meta.url), 'utf8')));
const repo = 'isp-insoft-gmbh/example-repo';
const row = (n, golem, extra = {}) => ({id:`00000000-0000-4000-8000-00000000000${n}`, resource:`${repo}/${golem}`, seat:'example-alex/anthropic', state:'bound', run_id:77, attempt:1, dispatched:true, request_verified:true, check_id:21, trigger:'mention', harness:'claude', blockers:[], ...extra});
const ready = row(1, 'review-notes');
const held = row(2, 'historical-hold', {seat:'example-sam/anthropic', state:'dispatching', run_id:null, attempt:null});
const pending = row(3, 'hello', {state:'pending', run_id:null, attempt:null, dispatched:false, blockers:[{...ready, on:'seat'}], waiting_since:'2030-01-01T09:05:00Z'});
const waiting = row(4, 'audit-notes', {state:'pending', run_id:null, attempt:null, dispatched:false, blockers:[{...ready, on:'seat'}, {...pending, on:'seat'}], waiting_since:'2030-01-01T08:00:00Z'});
const crossSeat = row(5, 'review-notes', {seat:'example-jo/openai-codex', state:'pending', run_id:null, attempt:null, dispatched:false, blockers:[{...ready, on:'repository/golem'}], request_verified:false, check_id:null, trigger:null, harness:null});
const retired = row(6, 'completed', {state:'terminal', run_id:31});
const repositories = [{repo, enabled:false, metadata_status:'available', golems:[]},
  {repo:'isp-insoft-gmbh/mixed', enabled:true, metadata_status:'available', golems:[{name:'hello',enabled:true},{name:'broken',enabled:false}]},
  {repo:'isp-insoft-gmbh/unknown', enabled:null, metadata_status:'unavailable', golems:[]}];
const snapshot = '2030-01-01T12:00:00Z';
const run = {id:77, attempt:1, conclusion:'failure', updated_at:'2030-01-01T11:00:00Z'};
const recorded = {actor_login:'fixture-owner', at:snapshot, evidence_sha:'3'.repeat(40)};
const prs = {pull:{title:'Enable golems', head:'golems/enable', base:'trunk', body:'Review before merging.'},files:[['.golems/hello.md','A synthetic definition.']]};
let denied = false, stale = true, posts = 0, previews = 0, starts = 0;
const failures = [];
const server = createServer(async (req, res) => {
  const reply = (status, body, type='application/json') => {
    res.writeHead(status, {'Content-Type':type, 'Cache-Control':'no-store', 'Content-Security-Policy':"default-src 'none'; script-src 'self'; connect-src 'self'; style-src 'self'"});
    res.end(type === 'application/json' ? JSON.stringify(body) : body);
  };
  try {
    const url = new URL(req.url, 'http://127.0.0.1');
    switch (url.pathname) {
      case '/admin': return reply(200, assets[0].replace('{login}', 'fixture-owner').replace('{csrf}', 'fixture-csrf'), 'text/html');
      case '/admin.css': return reply(200, assets[1], 'text/css');
      case '/admin.js': return reply(200, assets[2], 'text/javascript');
      case '/v1/admin/queue': return denied ? reply(403, {}) : reply(200, {snapshot_at:snapshot, rows:[retired, ready, held, pending, waiting, crossSeat], next:[]});
      case '/v1/admin/runs': return reply(200, {as_of:1, from:snapshot, to:snapshot, runs:[], workflow_health:[], total_workflow_runs:0, listing_complete:true, next_page:null, missing_audits:0, before_audits:0, malformed_audits:0, pending:0});
      case '/v1/admin/undispatched': return reply(200, {runs:[], listing_complete:true, next_page:null, malformed_audits:0});
      case '/v1/admin/seats': return reply(200, {seats:[], complete:true});
      case '/v1/admin/repositories': return reply(200, {repositories, total_repositories:repositories.length, complete:true, next_page:null});
      case '/v1/admin/onboarding/preview':
        assert.equal(req.method, 'GET'); assert.equal(url.searchParams.get('repository'), repo); return reply(200, prs);
      case '/v1/admin/onboarding':
        assert.equal(req.method, 'POST'); assert.equal(req.headers['x-golem-csrf'], 'fixture-csrf'); starts++;
        return reply(200, {url:`https://github.com/${repo}/pull/8`, created:false});
      case '/v1/admin/queue/preview':
      case '/v1/admin/queue/confirm': {
        assert.equal(req.method, 'POST'); assert.equal(req.headers['x-golem-csrf'], 'fixture-csrf');
        let text = ''; for await (const chunk of req) text += chunk;
        const body = JSON.parse(text);
        if (url.pathname.endsWith('/preview')) {
          previews++;
          if (body.request_id === held.id) return reply(200, {eligible:false, reason:'unbound', request:held});
          if (body.request_id === pending.id) {
            assert.equal(body.operation, 'cancel');
            return reply(200, {eligible:true, request:pending, unblocked:[], expires:Date.now()+60000, token:'cancel-token'});
          }
          assert.deepEqual(body, {request_id:ready.id, operation:'release'});
          return reply(200, {eligible:true, request:ready, run, outcome:'blocked', unblocked:[pending], expires:Date.now()+60000, token:'proof-token'});
        }
        posts++;
        if (body.request_id === pending.id) {
          assert.equal(body.operation, 'cancel'); assert.equal(body.token, 'cancel-token');
          return reply(200, {request:{...pending,state:'stop_requested'}, audit:recorded, journal_sha:'2'.repeat(40)});
        }
        assert.equal(body.request_id, ready.id); assert.equal(body.operation, 'release'); assert.equal(body.token, 'proof-token');
        if (stale) { stale = false; return reply(409, {}); }
        return reply(200, {request:{...ready,state:'terminal'}, audit:recorded, journal_sha:'1'.repeat(40)});
      }
      case '/favicon.ico': return reply(404, {});
      default: throw new Error(`unexpected fixture path ${url.pathname}`);
    }
  } catch (error) { failures.push(error); reply(500, {}); }
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const local = `http://127.0.0.1:${server.address().port}`;
const profile = await mkdtemp(join(tmpdir(), 'golem-browser-'));
const child = spawn(chromium, ['--headless', '--disable-background-networking', '--no-first-run', '--no-default-browser-check', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], {stdio:'ignore'});
let ws;
try {
  let port;
  for (let i = 0; i < 80 && !port; i++) {
    try { port = Number((await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]); } catch { await sleep(100); }
  }
  assert.ok(port, 'Chromium debugging port');
  const [page] = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await once(ws, 'open');
  let sequence = 0;
  const replies = new Map(), exceptions = [];
  ws.addEventListener('message', ({data}) => {
    const message = JSON.parse(data);
    if (message.method === 'Runtime.exceptionThrown') exceptions.push(message.params.exceptionDetails.text);
    const waiter = replies.get(message.id);
    if (waiter) { replies.delete(message.id); if (message.error) waiter.reject(new Error(JSON.stringify(message.error))); else waiter.resolve(message.result); }
  });
  const call = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence;
    replies.set(id, {resolve, reject}); ws.send(JSON.stringify({id, method, params}));
  });
  const js = async expression => {
    const result = await call('Runtime.evaluate', {expression, returnByValue:true});
    assert.ok(!result.exceptionDetails, `Browser exception for: ${expression}`);
    return result.result.value;
  };
  async function until(expression) {
    for (let i = 0; i < 80; i++) { if (await js(expression)) return; await sleep(75); }
    assert.fail(`Browser condition not met: ${expression}`);
  }
  const text = value => `document.body.innerText.includes(${JSON.stringify(value)})`;
  const click = async (value, selector = 'button') => assert.equal(await js(`(() => { const target = [...document.querySelectorAll(${JSON.stringify(selector)})].find(e => e.offsetWidth && e.textContent.includes(${JSON.stringify(value)})); if (!target) return false; target.click(); return true; })()`), true, value);
  async function press(key, code, windowsVirtualKeyCode) {
    await call('Input.dispatchKeyEvent', {type:'keyDown', key, code, windowsVirtualKeyCode});
    await call('Input.dispatchKeyEvent', {type:'keyUp', key, code, windowsVirtualKeyCode});
  }
  async function size(width, height, theme) {
    await call('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
    await call('Emulation.setEmulatedMedia',{features:[{name:'prefers-color-scheme',value:theme}]});
  }
  async function shot(name) {
    if (!shots) return;
    const {data} = await call('Page.captureScreenshot',{format:'png'});
    await writeFile(join(shots, `${name}.png`), Buffer.from(data, 'base64'));
  }
  const noOverflow = async () => assert.equal(await js('document.documentElement.scrollWidth <= innerWidth'), true);
  await call('Runtime.enable');
  await size(390, 1100, 'dark');
  await call('Page.navigate',{url:`${local}/admin`});
  await until('document.getElementById("queue-tab")?.offsetWidth > 0');
  await noOverflow();
  await press('Tab','Tab',9);
  assert.equal(await js('document.activeElement.textContent'), 'Skip to content');
  await press('Tab','Tab',9);
  assert.equal(await js('document.activeElement.textContent'), 'Manage seats');
  await press('Tab','Tab',9);
  assert.equal(await js('document.activeElement.textContent'), 'Operations');
  await js('document.querySelector(".skip").focus()');
  await press('Enter','Enter',13);
  assert.equal(await js('document.activeElement.id'), 'main');
  await click('Queue');
  assert.deepEqual(await js(`[...document.querySelectorAll('#queue-list .record-group:first-child .record strong')].map(row => row.textContent.split(' / ').at(-1))`), ['review-notes','hello','audit-notes','completed'], 'reservations first, then pending journal order even when request timestamps disagree');
  await noOverflow();
  await shot('mobile-dark-queue');
  await press('End', 'End', 35);
  await until('scrollY > 0');
  await noOverflow();
  await shot('mobile-dark-cross-seat');
  await click('review-notes', '#queue-list .reference');
  assert.equal(await js(text('77 / 1')), true, 'cross-seat reference opens the actual resource blocker');
  await click('Back to queue');
  await js(`(() => { const search = document.querySelector('#queue-list input'); search.value = 'review-notes'; search.dispatchEvent(new Event('input')); })()`);
  await click('review-notes');
  await click('Back to queue');
  assert.equal(await js(`document.querySelector('#queue-list input').value`), 'review-notes');
  assert.equal(await js(`document.querySelectorAll('#queue-list .record').length`), 2);
  assert.equal(await js('document.activeElement.className'), 'record');
  await click('review-notes');
  await click('Recheck release evidence');
  await until(text('Confirm verified release'));
  await noOverflow();
  assert.equal(posts, 0, 'preview is not confirm');
  await shot('mobile-dark-eligible');
  await click('Confirm verified release');
  await until(text('Preview stale or expired'));
  assert.equal(await js(text('Confirm verified release')), false);
  await click('Recheck release evidence');
  await until(text('Confirm verified release'));
  await click('Confirm verified release');
  await until(text('Verified release recorded'));
  assert.equal(await js(text('@fixture-owner')), true);
  await click('Back to queue');
  await js(`(() => { const search = document.querySelector('#queue-list input'); search.value = ''; search.dispatchEvent(new Event('input')); })()`);
  await click('historical-hold');
  assert.equal(await js(text('No trusted bound run')), true, 'missing snapshot fact is adjacent to release');
  await click('Recheck release evidence');
  await until(text('No trusted bound run'));
  assert.equal(await js(text('Confirm verified release')), false);
  await until(`!Array.from(document.querySelectorAll('#queue-list button')).find(button => button.textContent === 'Recheck release evidence').disabled`);
  await js(`document.querySelector('#queue-list .action-block details:not([hidden]) summary').focus()`);
  assert.equal(await js('document.activeElement.tagName'), 'SUMMARY');
  await press(' ','Space',32);
  await until(`Array.from(document.querySelectorAll('#queue-list .action-block details[open]')).some(element => element.offsetWidth && element.textContent.includes('does not establish identity'))`);
  await size(1120, 900, 'light');
  await js('scrollTo(0, 0)');
  await shot('desktop-light-denied-proof');
  await click('Back to queue');
  await click('/ hello');
  await click('Review pending cancellation');
  await until(text('Confirm pending cancellation'));
  await click('Confirm pending cancellation');
  await until(text('Cancellation recorded'));
  await click('Repositories', 'nav a');
  await until(text('Not configured'));
  assert.deepEqual(await js(`[...document.querySelectorAll('#repositories .record-group h3')].map(heading => heading.textContent)`), ['Configured','Not configured','Unknown']);
  assert.equal(await js(text('Mixed')), true);
  assert.equal(await js(text('Review starter PR')), false);
  await shot('desktop-light-repositories');
  await size(360, 720, 'light');
  await noOverflow();
  await shot('mobile-light-repositories');
  await size(1120, 900, 'dark');
  await noOverflow();
  await shot('desktop-dark-repositories');
  await size(1120, 900, 'light');
  await click(repo);
  await click('Review starter PR');
  await until(text('Review before merging.'));
  assert.equal(starts, 0);
  await shot('desktop-light-starter-preview');
  await click('Create starter PR');
  await until(text('Existing starter PR, which may be closed'));
  denied = true;
  await call('Page.navigate',{url:`${local}/admin?member`});
  await until(text('Queue is restricted'));
  assert.equal(await js('document.getElementById("queue-tab").hidden'), true);
  assert.equal(await js('document.querySelectorAll("#queue-list .record").length'), 0);
  assert.deepEqual(exceptions, []);
  assert.deepEqual(failures, []);
  assert.deepEqual([previews, posts, starts], [4,3,1]);
  console.log('Browser: seat grouping and journal order, cross-seat blocker, repository groups, narrow keyboard/scrolling, dark/light reflow, filters, release eligible/stale/confirmed/unbound, exact cancel, starter preview/existing and member denial passed.');
} finally {
  ws?.close(); server.close();
  child.kill();
  if (child.exitCode === null && child.signalCode === null) await once(child, 'exit');
  await rm(profile, {recursive:true, force:true, maxRetries:5, retryDelay:100});
}
