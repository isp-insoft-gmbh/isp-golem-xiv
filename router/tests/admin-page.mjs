// DOM consumer for the delivered dashboard script. A text-only double detects HTML sinks;
// real layout/browser verification remains the rendered-page gate.
import assert from 'node:assert/strict';
import {runInNewContext} from 'node:vm';

export async function verifyAdminPage(script) {
  class Element {
    constructor(tag) { this.tag = tag; this.children = []; this.value = ''; this.hidden = false; }
    append(child) { this.children.push(child); }
    replaceChildren() { this.children = []; this.value = ''; }
    set textContent(value) { this.value = String(value); this.children = []; }
    get textContent() { return this.value + this.children.map(c => c.textContent).join(' '); }
    set innerHTML(_) { assert.fail('Untrusted metadata must never reach an HTML sink'); }
  }
  const ids = new Map(['runs-status','runs','more-runs','undispatched-status','more-undispatched','usage','health','seats-status','seats','repositories-status','repositories','more-repositories'].map(id => [id,new Element('div')]));
  const document = {getElementById:id => { assert.ok(ids.has(id)); return ids.get(id); }, createElement:tag => new Element(tag)};
  const log = id => `https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${id}`;
  const first = {run_id:1,repo:'<img src=x onerror=attack()>',golem:'hello',trigger:'mention',seat:'member/anthropic',model:'claude-sonnet-5-5',outcome:'blocked',duration_s:42,log:log(1)};
  const second = {...first,run_id:2,model:null,duration_s:null,outcome:'replied',log:log(2)};
  const third = {...first,run_id:3,seat:'other/openai-codex',outcome:'no_change',duration_s:1,log:'https://attacker.invalid/'};
  let denied = false;
  const requests = [];
  async function fetch(path, options) {
    requests.push(path);
    assert.equal(options.credentials,'same-origin'); assert.equal(options.cache,'no-store'); assert.equal(options.redirect,'error');
    if (denied) return {ok:false,status:403,json:async () => assert.fail('must not consume denied response')};
    let body;
    if (path.startsWith('/v1/admin/runs')) {
      const page = new URL(path,'https://router.invalid').searchParams.get('page');
      body = {as_of:1794902400,from:'2026-10-18T00:00:00Z',to:'2026-11-17T00:00:00Z',runs:page === '1' ? [first,second] : [first,third],workflow_health:page === '1' ? [{run_id:1,conclusion:'failure'},{run_id:2,conclusion:'success'}] : [{run_id:1,conclusion:'failure'},{run_id:3,conclusion:'success'}],missing_audits:page === '1' ? 0 : 1,malformed_audits:0,pending:0,total_workflow_runs:4,listing_complete:page !== '1',next_page:page === '1' ? 2 : null,coverage_note:'Verified backfill required for historical gaps.'};
    } else if (path.startsWith('/v1/admin/undispatched')) {
      const page = new URL(path,'https://router.invalid').searchParams.get('page');
      body = {as_of:1794902400,runs:page === '1' ? [{...first,request_id:'11111111-1111-4111-8111-111111111111',run_id:null,outcome:'stopped',model:null,log:null,duration_s:null}] : [],listing_complete:page !== '1',next_page:page === '1' ? 2 : null,after:page === '1' ? 'next-audit-page' : null,malformed_audits:page === '1' ? 2 : 0,coverage_note:'Undispatched audits.'};
    } else if (path === '/v1/admin/seats') {
      body = {complete:true,expiry_note:'Expiry unknown.',seats:[{person:'GONE_PERSON',provider:'openai-codex',membership:'left organization',identity:'normalized identifier',refreshing:true,expires_at:null}]};
    } else if (path.startsWith('/v1/admin/repositories')) {
      const page = new URL(path,'https://router.invalid').searchParams.get('page');
      body = {complete:page !== '1',total_repositories:2,next_page:page === '1' ? 2 : null,coverage_note:'Installed repositories.',repositories:[page === '1' ? {repo:'isp-insoft-gmbh/unreadable',repository_id:1,enabled:null,metadata_status:'unavailable',dispatch_allowlisted:false,golems:[]} : {repo:'isp-insoft-gmbh/healthy',repository_id:2,enabled:true,metadata_status:'available',dispatch_allowlisted:false,golems:[{name:'bad',enabled:false,reason:'invalid definition'}]}]};
    } else assert.fail(`Unexpected dashboard URL: ${path}`);
    return {ok:true,json:async () => structuredClone(body)};
  }
  runInNewContext(script,{document,fetch,URL,console});
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(requests.length,4);
  assert.equal(ids.get('runs').children[0].children[1].children.length,3);
  assert.match(ids.get('runs').textContent,/not dispatched/);
  assert.match(ids.get('runs').textContent,/<img src=x onerror=attack\(\)>/);
  assert.match(ids.get('runs').textContent,/unknown/);
  assert.match(ids.get('usage').textContent,/member\/anthropic 2 42 1/);
  assert.match(ids.get('health').textContent,/1 failed \/ 2 completed; failure rate 50.0%/);
  assert.match(ids.get('health').textContent,/1 blocked/);
  assert.match(ids.get('seats').textContent,/GONE_PERSON openai-codex left organization/);
  assert.match(ids.get('seats').textContent,/unknown/);
  assert.match(ids.get('repositories').textContent,/Metadata unavailable; enabled state unknown/);
  assert.doesNotMatch(ids.get('repositories').textContent,/Not enabled/);
  assert.equal(ids.get('more-repositories').hidden,false);
  assert.equal(ids.get('more-runs').hidden,false);
  await ids.get('more-runs').onclick();
  assert.ok(requests.some(p => p === '/v1/admin/runs?page=2&as_of=1794902400'));
  assert.equal(ids.get('runs').children[0].children[1].children.length,4,'pagination deduplicates immutable run IDs');
  assert.match(ids.get('health').textContent,/1 failed \/ 3 completed; failure rate 33.3%/);
  assert.match(ids.get('health').textContent,/Missing audits: 1/);
  assert.match(ids.get('runs-status').textContent,/History incomplete/);
  assert.equal(ids.get('more-runs').hidden,true);
  assert.ok(!ids.get('runs').textContent.includes('attacker.invalid'));
  await ids.get('more-undispatched').onclick();
  assert.ok(requests.includes('/v1/admin/undispatched?page=2&after=next-audit-page&as_of=1794902400'));
  assert.equal(ids.get('more-undispatched').hidden,true);
  assert.match(ids.get('undispatched-status').textContent,/Malformed undispatched audits loaded: 2/, 'clean final page must not erase earlier malformed records');
  assert.match(ids.get('health').textContent,/malformed audits: 2/, 'health includes malformed undispatched audits');
  await ids.get('more-repositories').onclick();
  assert.match(ids.get('repositories').textContent,/healthy Enabled/);
  assert.match(ids.get('repositories').textContent,/bad disabled invalid definition/);
  assert.equal(ids.get('more-repositories').hidden,true);
  denied = true;
  await ids.get('more-runs').onclick();
  assert.equal(ids.get('runs-status').textContent,'Metadata unavailable or membership no longer active. Reload or sign in again.');
}
