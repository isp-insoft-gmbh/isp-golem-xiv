// DOM consumer for the delivered dashboard script. Text-only nodes detect HTML sinks;
// rendered-page and browser verification remain separate gates.
import assert from 'node:assert/strict';
import {runInNewContext} from 'node:vm';

export async function verifyAdminPage(script) {
  class Element {
    constructor(tag) { this.tag = tag; this.children = []; this.value = ''; this.hidden = false; this.disabled = false; this.href = ''; this.scope = ''; }
    append(child) { this.children.push(child); }
    replaceChildren() { this.children = []; this.value = ''; }
    set textContent(value) { this.value = String(value); this.children = []; }
    get textContent() { return this.value + this.children.map(child => child.textContent).join(' '); }
    set innerHTML(_) { assert.fail('Untrusted metadata must never reach an HTML sink'); }
  }
  const ids = new Map(['runs-status','runs','more-runs','undispatched-status','more-undispatched','usage','health','seats-status','seats','repositories-status','repositories','more-repositories'].map(id => [id, new Element('div')]));
  const document = {getElementById: id => { assert.ok(ids.has(id)); return ids.get(id); }, createElement: tag => new Element(tag), createTextNode: text => ({textContent: String(text)})};
  const log = id => `https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${id}`;
  const first = {run_id:1,repo:'<img src=x onerror=attack()>',golem:'hello',trigger:'mention',seat:'member/anthropic',model:'claude-sonnet-5-5',outcome:'blocked',duration_s:42,log:log(1),completed_at:'2026-11-16T10:00:00Z'};
  const second = {...first,run_id:2,model:null,duration_s:null,outcome:'replied',log:log(2),completed_at:'2026-11-16T11:00:00Z'};
  const third = {...first,run_id:3,seat:'other/openai-codex',outcome:'no_change',duration_s:1,log:'https://attacker.invalid/',completed_at:'2026-11-16T12:00:00Z'};
  const requests = [];
  async function fetch(path, options) {
    requests.push(path);
    assert.equal(options.credentials, 'same-origin');
    assert.equal(options.cache, 'no-store');
    assert.equal(options.redirect, 'error');
    let body;
    if (path.startsWith('/v1/admin/runs')) {
      const page = new URL(path, 'https://router.invalid').searchParams.get('page');
      body = {as_of:1794902400,from:'2026-10-18T00:00:00Z',to:'2026-11-17T00:00:00Z',runs:page === '1' ? [first,second] : [first,third],workflow_health:page === '1' ? [{run_id:1,conclusion:'failure'},{run_id:2,conclusion:'success'}] : [{run_id:1,conclusion:'failure'},{run_id:3,conclusion:'success'}],missing_audits:page === '1' ? 0 : 1,before_audits:page === '1' ? 0 : 2,malformed_audits:0,pending:0,total_workflow_runs:4,listing_complete:page !== '1',next_page:page === '1' ? 2 : null};
    } else if (path.startsWith('/v1/admin/undispatched')) {
      const page = new URL(path, 'https://router.invalid').searchParams.get('page');
      body = {as_of:1794902400,runs:page === '1' ? [{...first,request_id:'11111111-1111-4111-8111-111111111111',run_id:null,outcome:'stopped',model:null,log:null,duration_s:null}] : [],listing_complete:page !== '1',next_page:page === '1' ? 2 : null,after:page === '1' ? 'next-audit-page' : null,malformed_audits:page === '1' ? 2 : 0};
    } else if (path === '/v1/admin/seats') {
      body = {complete:true,expiry_note:'Not recorded.',seats:[{person:'GONE_PERSON',provider:'openai-codex',membership:'left organization',identity:'normalized identifier',refreshing:true},{person:'Member',provider:'anthropic',membership:'active',identity:'GitHub login',refreshing:false}]};
    } else if (path.startsWith('/v1/admin/repositories')) {
      const page = new URL(path, 'https://router.invalid').searchParams.get('page');
      body = {complete:page !== '1',total_repositories:2,next_page:page === '1' ? 2 : null,repositories:[page === '1' ? {repo:'isp-insoft-gmbh/unreadable',repository_id:1,enabled:null,metadata_status:'unavailable',golems:[]} : {repo:'isp-insoft-gmbh/healthy',repository_id:2,enabled:true,metadata_status:'available',golems:[{name:'bad',enabled:false,reason:'invalid definition'}]}]};
    } else assert.fail(`Unexpected dashboard URL: ${path}`);
    return {ok:true,json:async () => structuredClone(body)};
  }
  runInNewContext(script, {document,fetch,URL,console});
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(requests.length, 4);
  assert.match(ids.get('runs').children[0].children[0].textContent, /Audited runs on loaded pages/);
  assert.equal(ids.get('runs').children[0].children[2].children.length, 3);
  assert.match(ids.get('runs').textContent, /not dispatched/);
  assert.match(ids.get('runs').textContent, /<img src=x onerror=attack\(\)>/);
  assert.match(ids.get('runs').textContent, /unknown/);
  assert.match(ids.get('usage').textContent, /member\/anthropic 2 42 1/);
  const metric = label => ids.get('health').children[0].children.find(group => group.children[0].textContent === label)?.children[1].textContent;
  assert.equal(ids.get('health').children[0].tag, 'dl', 'health uses semantic label/value pairs');
  assert.equal(metric('Workflow failures'), '1 / 2 (50.0%)');
  assert.equal(metric('Blocked'), '1');
  assert.equal(metric('Seat refresh state'), 'not recorded');
  assert.equal(metric('Runs before audits'), '0');
  assert.match(ids.get('seats').textContent, /GONE_PERSON openai-codex left organization/);
  assert.match(ids.get('seats').textContent, /unknown/);
  // Last runs per seat come from loaded audits, matched case-insensitively by login.
  assert.match(ids.get('seats').textContent, /Member anthropic active GitHub login no 2026-11-16T11:00:00Z 2026-11-16T10:00:00Z/);
  assert.match(ids.get('seats-status').textContent, /sign-in expiry and refresh state are not recorded/);
  assert.match(ids.get('repositories').textContent, /Metadata unavailable; enabled state unknown/);
  assert.doesNotMatch(ids.get('repositories').textContent, /Not enabled/);
  assert.equal(ids.get('repositories').children.filter(child => child.tag === 'table').length, 0, 'no empty golem table for unreadable repository');
  assert.equal(ids.get('more-repositories').hidden, false);
  assert.equal(ids.get('more-runs').hidden, false);
  await ids.get('more-runs').onclick();
  assert.ok(requests.includes('/v1/admin/runs?page=2&as_of=1794902400'));
  assert.equal(ids.get('runs').children[0].children[2].children.length, 4, 'pagination deduplicates immutable run IDs');
  const pagedRows = ids.get('runs').children[0].children[2].children;
  assert.deepEqual(pagedRows.flatMap(row => row.children[8].children.map(link => link.href)).sort(), [log(1), log(2)], 'existing audited runs remain exactly once after an overlapping page');
  assert.equal(pagedRows.filter(row => row.children[3].textContent === third.seat && row.children[6].textContent === third.outcome).length, 1, 'new audited run appends exactly once');
  assert.equal(metric('Workflow failures'), '1 / 3 (33.3%)');
  assert.equal(metric('Missing audits'), '1');
  assert.equal(metric('Runs before audits'), '2');
  assert.match(ids.get('runs-status').textContent, /History incomplete: 1 missing audit, 2 malformed audits\. 2 earlier/);
  assert.match(ids.get('runs-status').textContent, /2 earlier runs predate audits \(before 2026-11-13\)\./);
  assert.equal(ids.get('more-runs').hidden, true);
  assert.ok(!ids.get('runs').textContent.includes('attacker.invalid'));
  await ids.get('more-undispatched').onclick();
  assert.ok(requests.includes('/v1/admin/undispatched?page=2&after=next-audit-page&as_of=1794902400'));
  assert.equal(ids.get('more-undispatched').hidden, true);
  assert.match(ids.get('undispatched-status').textContent, /Malformed undispatched audits: 2/, 'clean final page must not erase earlier malformed records');
  assert.equal(metric('Malformed audits'), '2', 'health includes malformed undispatched audits');
  await ids.get('more-repositories').onclick();
  assert.match(ids.get('repositories').textContent, /healthy Enabled/);
  assert.match(ids.get('repositories').textContent, /bad disabled invalid definition/);
  assert.equal(ids.get('more-repositories').hidden, true);
  assert.equal(await verifyEmptyLoad(script), true);
  assert.equal(await verifyInitialRetry(script), true);
  assert.equal(await verifyMembershipFailure(script), true);
  await verifyMixedSourceRetry(script);
  await verifyRevokedPagination(script);
  await verifyExpiredPagination(script);
  await verifyOnboarding(script);
}

async function verifyMixedSourceRetry(script) {
  let fail = true;
  const run = {run_id:9,repo:'isp-insoft-gmbh/fixture',golem:'hello',trigger:'mention',seat:'member/anthropic',model:null,outcome:'replied',duration_s:42,log:null};
  const dashboard = harness(script, async path => {
    if (path.startsWith('/v1/admin/runs')) return {as_of:1794902400,from:'start',to:'end',runs:[run],workflow_health:[{run_id:9,conclusion:'success'}],missing_audits:0,before_audits:0,malformed_audits:0,pending:0,total_workflow_runs:1,listing_complete:true,next_page:null};
    if (path.startsWith('/v1/admin/undispatched')) {
      if (fail) return {errorStatus:503};
      return {as_of:1794902400,runs:[{...run,request_id:'22222222-2222-4222-8222-222222222222',run_id:null,outcome:'stopped',duration_s:null}],listing_complete:true,next_page:null,malformed_audits:0};
    }
    if (path === '/v1/admin/seats') return {complete:true,seats:[]};
    return {complete:true,total_repositories:0,next_page:null,repositories:[]};
  });
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  const status = dashboard.ids.get('undispatched-status');
  assert.match(status.textContent, /Metadata unavailable/);
  assert.equal(dashboard.ids.get('runs').children[0].children[2].children.length, 1, 'successful workflow rows survive another source failing');
  const retry = status.children.find(child => child.tag === 'button');
  assert.ok(retry, 'failed undispatched source has its own recovery action');
  fail = false;
  await retry.onclick();
  assert.equal(dashboard.ids.get('runs').children[0].children[2].children.length, 2);
  assert.match(dashboard.ids.get('usage').textContent, /member\/anthropic 1 42 0/, 'undispatched recovery does not add dispatched usage');
  assert.match(dashboard.ids.get('health').textContent, /Stopped 1/);
}

async function verifyRevokedPagination(script) {
  const run = {run_id:9,repo:'isp-insoft-gmbh/fixture',golem:'hello',trigger:'mention',seat:'member/anthropic',model:null,outcome:'replied',duration_s:42,log:null};
  const dashboard = harness(script, async path => {
    if (path.startsWith('/v1/admin/runs?page=2')) return {errorStatus:403};
    if (path.startsWith('/v1/admin/runs')) return {as_of:1794902400,from:'start',to:'end',runs:[run],workflow_health:[{run_id:9,conclusion:'success'}],missing_audits:0,before_audits:0,malformed_audits:0,pending:0,total_workflow_runs:2,listing_complete:false,next_page:2};
    if (path.startsWith('/v1/admin/undispatched')) return {as_of:1794902400,runs:[],listing_complete:true,next_page:null,malformed_audits:0};
    if (path === '/v1/admin/seats') return {complete:true,seats:[]};
    return {complete:true,total_repositories:0,next_page:null,repositories:[]};
  });
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  await dashboard.ids.get('more-runs').onclick();
  assert.match(dashboard.ids.get('runs-status').textContent, /Organization membership required/);
  assert.equal(dashboard.ids.get('more-runs').hidden, true);
  assert.equal(dashboard.ids.get('runs').children[0].children[2].children.length, 1, 'membership loss preserves existing rows without adding denied data');
  assert.match(dashboard.ids.get('usage').textContent, /member\/anthropic 1 42 0/);
}

async function verifyExpiredPagination(script) {
  const dashboard = harness(script, async path => {
    if (path.startsWith('/v1/admin/runs')) return {errorStatus:400};
    if (path.startsWith('/v1/admin/undispatched')) return {as_of:1794902400,runs:[],listing_complete:true,next_page:null,malformed_audits:0};
    if (path === '/v1/admin/seats') return {complete:true,seats:[]};
    return {complete:true,total_repositories:0,next_page:null,repositories:[]};
  });
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  const status = dashboard.ids.get('runs-status');
  assert.match(status.textContent, /Pagination expired/);
  assert.ok(status.children.some(child => child.tag === 'a' && child.href === '/admin' && child.textContent === 'Reload dashboard'));
  assert.equal(status.children.some(child => child.tag === 'button'), false, 'expired cursors require reload, not another identical request');
}

async function verifyEmptyLoad(script) {
  const dashboard = harness(script, async path => path.startsWith('/v1/admin/runs')
    ? {as_of:1794902400,from:'start',to:'end',runs:[],workflow_health:[],missing_audits:0,before_audits:0,malformed_audits:0,pending:0,total_workflow_runs:0,listing_complete:true,next_page:null}
    : path.startsWith('/v1/admin/undispatched')
      ? {as_of:1794902400,runs:[],listing_complete:true,next_page:null,malformed_audits:0}
      : path === '/v1/admin/seats' ? {complete:true,expiry_note:'Expiry unknown.',seats:[]}
        : {complete:true,total_repositories:0,next_page:null,repositories:[]});
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  assert.match(dashboard.ids.get('runs').textContent, /Audited runs on loaded pages/);
  assert.match(dashboard.ids.get('runs-status').textContent, /No audited runs on loaded pages; 0 workflows reported/);
  assert.doesNotMatch(dashboard.ids.get('runs-status').textContent, /No runs/);
  assert.match(dashboard.ids.get('usage').textContent, /No records on loaded pages/);
  assert.match(dashboard.ids.get('seats').textContent, /No records on loaded pages/);
  return true;
}

async function verifyMembershipFailure(script) {
  const dashboard = harness(script, async path => {
    if (path.startsWith('/v1/admin/runs')) return {errorStatus:403};
    if (path.startsWith('/v1/admin/undispatched')) return {as_of:1794902400,runs:[],listing_complete:true,next_page:null,malformed_audits:0};
    if (path === '/v1/admin/seats') return {complete:true,expiry_note:'Expiry unknown.',seats:[]};
    return {complete:true,total_repositories:0,next_page:null,repositories:[]};
  });
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  const status = dashboard.ids.get('runs-status');
  assert.match(status.textContent, /Organization membership required/);
  assert.ok(status.children.some(child => child.tag === 'button' && child.textContent === 'Retry'));
  assert.ok(status.children.some(child => child.tag === 'a' && child.href === '/admin'));
  return true;
}

async function verifyInitialRetry(script) {
  let fail = true;
  const dashboard = harness(script, async path => {
    if (path.startsWith('/v1/admin/runs') && fail) { fail = false; return null; }
    if (path.startsWith('/v1/admin/runs')) return {as_of:1794902400,from:'start',to:'end',runs:[],workflow_health:[],missing_audits:0,before_audits:0,malformed_audits:0,pending:0,total_workflow_runs:0,listing_complete:true,next_page:null};
    if (path.startsWith('/v1/admin/undispatched')) return {as_of:1794902400,runs:[],listing_complete:true,next_page:null,malformed_audits:0};
    if (path === '/v1/admin/seats') return {complete:true,expiry_note:'Expiry unknown.',seats:[]};
    return {complete:true,total_repositories:0,next_page:null,repositories:[]};
  });
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  const status = dashboard.ids.get('runs-status');
  assert.match(status.textContent, /Metadata unavailable/);
  const retry = status.children.find(child => child.tag === 'button');
  assert.ok(retry, 'initial failure exposes retry before any pagination success');
  assert.equal(dashboard.ids.get('more-runs').hidden, true, 'failed initial load does not offer load more');
  const retrying = retry.onclick();
  assert.equal(retry.disabled, true, 'retry is disabled during its request');
  await retrying;
  assert.equal(retry.disabled, false);
  assert.match(status.textContent, /No audited runs on loaded pages/);
  assert.equal(dashboard.requests.filter(path => path.startsWith('/v1/admin/runs')).length, 2);
  return true;
}

async function verifyOnboarding(script) {
  const repo = 'isp-insoft-gmbh/fixture';
  let posts = 0, release;
  let mode = 'pending';
  const dashboard = harness(script, async (path, options) => {
    if (path === '/v1/admin/onboarding') {
      posts++;
      assert.equal(options.method, 'POST');
      assert.equal(options.headers['X-Golem-Csrf'], 'csrf-token');
      assert.equal(options.headers['Content-Type'], 'application/json');
      assert.equal(options.body, JSON.stringify({repository:repo}));
      if (mode === 'pending') return await new Promise(resolve => { release = resolve; });
      if (mode === 'network') throw new Error('offline');
      if (typeof mode === 'number') return {errorStatus:mode};
      return {responseStatus:200,created:false,url:mode};
    }
    if (path.startsWith('/v1/admin/runs')) return {as_of:1,from:'start',to:'end',runs:[],workflow_health:[],missing_audits:0,before_audits:0,malformed_audits:0,pending:0,total_workflow_runs:0,listing_complete:true,next_page:null};
    if (path.startsWith('/v1/admin/undispatched')) return {as_of:1,runs:[],listing_complete:true,next_page:null,malformed_audits:0};
    if (path === '/v1/admin/seats') return {complete:true,seats:[]};
    return {complete:true,total_repositories:4,next_page:null,repositories:[
      {repo,enabled:false,metadata_status:'available',golems:[]},
      {repo:'isp-insoft-gmbh/unknown',enabled:null,metadata_status:'unavailable',golems:[]},
      {repo:'isp-insoft-gmbh/enabled',enabled:true,metadata_status:'available',golems:[]},
      {repo:'evil/fixture',enabled:false,metadata_status:'available',golems:[]}
    ]};
  });
  dashboard.run();
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(posts, 0, 'loading never mutates');
  const parent = dashboard.ids.get('repositories');
  const buttons = parent.children.filter(child => child.tag === 'button');
  assert.equal(buttons.length, 1);
  const button = buttons[0];
  const status = parent.children.find(child => child.role === 'status');
  assert.equal(status['aria-live'], 'polite');
  assert.match(parent.textContent, /reviewable starter files/);
  const pending = button.onclick();
  assert.equal(button.disabled, true);
  await button.onclick();
  assert.equal(posts, 1);
  release({responseStatus:201,created:true,url:`https://github.com/${repo}/pull/42`});
  await pending;
  assert.equal(button.disabled, false);
  assert.equal(status.children[0].href, `https://github.com/${repo}/pull/42`);
  mode = 'network';
  await button.onclick();
  assert.match(status.textContent, /outcome may be unknown/);
  assert.equal(posts, 2);
  mode = `https://github.com/${repo}/pull/42`;
  await button.onclick();
  assert.match(status.textContent, /may be closed/);
  assert.equal(posts, 3, 'retry requires explicit action');
  for (const url of ['https://github.com/isp-insoft-gmbh/other/pull/42', `https://github.com/${repo}/pull/1?x=evil`, 'javascript:alert(1)']) {
    mode = url;
    await button.onclick();
    assert.match(status.textContent, /Unexpected response/);
    assert.equal(status.children.length, 0);
  }
  for (const [code, message] of [[401,/Sign in/],[403,/membership and repository write access/],[409,/configured.*archived.*incomplete/]]) {
    mode = code;
    await button.onclick();
    assert.match(status.textContent, message);
  }
}

function harness(script, respond) {
  class Element {
    constructor(tag) { this.tag = tag; this.children = []; this.value = ''; this.hidden = false; this.disabled = false; this.href = ''; }
    append(child) { this.children.push(child); }
    setAttribute(name, value) { this[name] = value; }
    replaceChildren() { this.children = []; this.value = ''; }
    set textContent(value) { this.value = String(value); this.children = []; }
    get textContent() { return this.value + this.children.map(child => child.textContent).join(' '); }
    set innerHTML(_) { assert.fail('Untrusted metadata must never reach an HTML sink'); }
  }
  const ids = new Map(['runs-status','runs','more-runs','undispatched-status','more-undispatched','usage','health','seats-status','seats','repositories-status','repositories','more-repositories','onboarding-csrf'].map(id => [id, new Element('div')]));
  ids.get('onboarding-csrf').value = 'csrf-token';
  const requests = [];
  const document = {getElementById: id => { assert.ok(ids.has(id)); return ids.get(id); }, createElement: tag => new Element(tag), createTextNode: text => ({textContent:String(text)})};
  async function fetch(path, options) {
    requests.push(path);
    assert.equal(options.credentials, 'same-origin'); assert.equal(options.cache, 'no-store'); assert.equal(options.redirect, 'error');
    const body = await respond(path, options);
    if (body === null) return {ok:false,status:503};
    if (body.errorStatus) return {ok:false,status:body.errorStatus};
    return {ok:true,status:body.responseStatus ?? 200,json:async () => structuredClone(body)};
  }
  return {ids,requests,run:() => runInNewContext(script, {document,fetch,URL,console})};
}
