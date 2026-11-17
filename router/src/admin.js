'use strict';
// Metadata stays text. No innerHTML, credential access, local storage, or external requests.
(() => {
  const byId = id => document.getElementById(id);
  const rows = [];
  let windowEnd;
  let missing = 0, malformed = 0, pending = 0, undispatchedMalformed = 0;
  const seen = new Set();
  const workflowHealth = new Map();
  const unknown = value => value === null || value === undefined ? 'unknown' : String(value);
  function table(parent, headings, records) {
    const table = document.createElement('table');
    const head = document.createElement('thead');
    const title = document.createElement('tr');
    for (const heading of headings) { const cell = document.createElement('th'); cell.scope = 'col'; cell.textContent = heading; title.append(cell); }
    head.append(title); table.append(head);
    const body = document.createElement('tbody');
    for (const record of records) {
      const row = document.createElement('tr');
      for (const value of record) {
        const cell = document.createElement('td');
        if (value && typeof value === 'object' && value.log) {
          // Defense in depth: the API constructs and validates the same fixed-origin URL.
          if (/^https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv-ops\/actions\/runs\/[1-9][0-9]*$/.test(value.log)) {
            const link = document.createElement('a'); link.href = value.log; link.textContent = 'Log'; cell.append(link);
          } else cell.textContent = 'unknown';
        } else cell.textContent = unknown(value);
        row.append(cell);
      }
      body.append(row);
    }
    table.append(body); parent.append(table);
  }
  async function get(path) {
    const response = await fetch(path, {credentials:'same-origin', cache:'no-store', redirect:'error'});
    if (!response.ok) throw new Error(response.status === 403 ? 'Organization membership required. Sign in again.' : 'Metadata unavailable. Reload to try again.');
    return response.json();
  }
  function rollup() {
    byId('runs').replaceChildren();
    table(byId('runs'), ['Repository','Golem','Trigger','Seat','Execution','Observed model','Outcome','Duration (s)','Log'], rows.map(r => [r.repo,r.golem,r.trigger,r.seat,r.run_id === null ? 'not dispatched' : 'Actions',r.model,r.outcome,r.duration_s,r.log ? {log:r.log} : null]));
    const groups = new Map();
    let blocked = 0, known = 0, stopped = 0, incomplete = 0, unknownOutcome = 0;
    for (const row of rows) {
      if (row.run_id !== null) {
        const group = groups.get(row.seat) ?? {runs:0,seconds:0,unknown:0};
        group.runs++;
        if (row.duration_s === null) group.unknown++; else group.seconds += row.duration_s;
        groups.set(row.seat,group);
      }
      if (row.outcome === null) unknownOutcome++; else known++;
      if (row.outcome === 'blocked') blocked++;
      if (row.outcome === 'stopped') stopped++;
      if (row.outcome === 'incomplete') incomplete++;
    }
    byId('usage').replaceChildren();
    table(byId('usage'), ['Person / provider','Runs','Observed time (s)','Unknown durations'], [...groups].sort(([a],[b]) => a.localeCompare(b)).map(([seat,g]) => [seat,g.runs,Math.round(g.seconds * 1000) / 1000,g.unknown]));
    const conclusions = [...workflowHealth.values()].filter(c => c !== null);
    const failures = conclusions.filter(c => ['failure','timed_out','startup_failure','action_required','stale'].includes(c)).length;
    byId('health').textContent = `Loaded workflows only: ${failures} failed / ${conclusions.length} completed; failure rate ${conclusions.length ? (100 * failures / conclusions.length).toFixed(1) + '%' : 'unknown'}. Loaded verified records: ${blocked} blocked / ${known} known outcomes; ${incomplete} incomplete; ${stopped} stopped; ${unknownOutcome} unknown outcomes. Missing audits: ${missing}; malformed audits: ${malformed + undispatchedMalformed}; pending workflows: ${pending}. Refreshing seat expiry: unknown.`;
  }
  async function runs(page = 1) {
    const button = byId('more-runs'); button.disabled = true;
    try {
      const data = await get(`/v1/admin/runs?page=${page}${windowEnd ? `&as_of=${windowEnd}` : ''}`);
      windowEnd = data.as_of;
      for (const run of data.workflow_health) workflowHealth.set(run.run_id,run.conclusion);
      for (const record of data.runs) { if (!seen.has(`run:${record.run_id}`)) { seen.add(`run:${record.run_id}`); rows.push(record); } }
      missing += data.missing_audits; malformed += data.malformed_audits; pending += data.pending;
      rollup();
      byId('runs-status').textContent = `${data.from} to ${data.to}. Loaded ${rows.filter(r => r.run_id !== null).length} verified Actions records of ${data.total_workflow_runs} workflow runs. ${data.listing_complete ? 'Listing traversed' : 'Partial listing; more records may remain'}. ${missing || malformed || pending ? 'History incomplete.' : 'Coverage applies only to loaded records.'} ${data.coverage_note}`;
      button.hidden = data.next_page === null;
      button.onclick = () => runs(data.next_page);
    } catch { byId('runs-status').textContent = 'Metadata unavailable or membership no longer active. Reload or sign in again.'; }
    finally { button.disabled = false; }
  }
  async function undispatched(page = 1, after, asOf) {
    const button = byId('more-undispatched'); button.disabled = true;
    try {
      const data = await get(`/v1/admin/undispatched?page=${page}${after ? `&after=${encodeURIComponent(after)}` : ''}${asOf ? `&as_of=${asOf}` : ''}`);
      for (const record of data.runs) { const key = `request:${record.request_id}`; if (!seen.has(key)) { seen.add(key); rows.push(record); } }
      undispatchedMalformed += data.malformed_audits;
      rollup();
      byId('undispatched-status').textContent = `${data.listing_complete ? 'Undispatched audit listing traversed.' : 'Partial undispatched audit listing.'} Malformed undispatched audits loaded: ${undispatchedMalformed}. ${data.coverage_note}`;
      button.hidden = data.next_page === null; button.onclick = () => undispatched(data.next_page,data.after,data.as_of);
    } catch { byId('undispatched-status').textContent = 'Metadata unavailable or membership no longer active. Reload or sign in again.'; }
    finally { button.disabled = false; }
  }
  async function seats() {
    try {
      const data = await get('/v1/admin/seats');
      byId('seats-status').textContent = `${data.complete ? 'Membership listing complete.' : 'Partial membership listing; departed status cannot be confirmed.'} ${data.expiry_note}`;
      table(byId('seats'), ['Person / identifier','Provider','Membership','Identity','Refreshing','Expiry'], data.seats.map(s => [s.person,s.provider,s.membership,s.identity,s.refreshing ? 'yes' : 'no',s.expires_at]));
    } catch { byId('seats-status').textContent = 'Metadata unavailable or membership no longer active. Reload or sign in again.'; }
  }
  async function repositories(page = 1) {
    const button = byId('more-repositories'); button.disabled = true;
    try {
      const data = await get(`/v1/admin/repositories?page=${page}`);
      for (const repository of data.repositories) {
        const heading = document.createElement('h3'); heading.textContent = repository.repo; byId('repositories').append(heading);
        const state = document.createElement('p'); state.textContent = `${repository.metadata_status === 'unavailable' ? 'Metadata unavailable; enabled state unknown' : repository.enabled ? 'Enabled' : 'Not enabled'}; dispatch ${repository.dispatch_allowlisted ? 'allowlisted' : 'not allowlisted'}.`; byId('repositories').append(state);
        table(byId('repositories'), ['Golem','State','Reason'], repository.golems.map(g => [g.name,g.enabled ? 'enabled' : 'disabled',g.reason ?? '']));
      }
      byId('repositories-status').textContent = `${data.complete ? 'Listing traversed.' : 'Partial listing.'} ${data.total_repositories} installed repositories. ${data.coverage_note}`;
      button.hidden = data.next_page === null; button.onclick = () => repositories(data.next_page);
    } catch { byId('repositories-status').textContent = 'Metadata unavailable or membership no longer active. Reload or sign in again.'; }
    finally { button.disabled = false; }
  }
  void runs(); void undispatched(); void seats(); void repositories();
})();
