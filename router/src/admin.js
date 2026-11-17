'use strict';
// Metadata stays text. No innerHTML, credential access, local storage, or external requests.
(() => {
  const byId = id => document.getElementById(id);
  const rows = [];
  const seen = new Set();
  const workflowHealth = new Map();
  let windowEnd;
  let missing = 0, malformed = 0, pending = 0, undispatchedMalformed = 0;
  const unknown = value => value === null || value === undefined ? 'unknown' : String(value);

  function table(parent, caption, headings, records) {
    const result = document.createElement('table');
    const label = document.createElement('caption');
    label.textContent = caption;
    result.append(label);
    const head = document.createElement('thead');
    const title = document.createElement('tr');
    for (const heading of headings) {
      const cell = document.createElement('th');
      cell.scope = 'col';
      cell.textContent = heading;
      title.append(cell);
    }
    head.append(title);
    result.append(head);
    const body = document.createElement('tbody');
    for (const record of records) {
      const row = document.createElement('tr');
      for (const value of record) {
        const cell = document.createElement('td');
        if (value && typeof value === 'object' && value.log) {
          if (/^https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv-ops\/actions\/runs\/[1-9][0-9]*$/.test(value.log)) {
            const link = document.createElement('a');
            link.href = value.log;
            link.textContent = 'Log';
            cell.append(link);
          } else cell.textContent = 'unknown';
        } else cell.textContent = unknown(value);
        row.append(cell);
      }
      body.append(row);
    }
    result.append(body);
    parent.append(result);
    if (!records.length) {
      const empty = document.createElement('p');
      empty.textContent = 'No records on loaded pages.';
      parent.append(empty);
    }
  }

  async function get(path) {
    const response = await fetch(path, {credentials: 'same-origin', cache: 'no-store', redirect: 'error'});
    if (!response.ok) throw new Error(response.status === 403 ? 'membership' : response.status === 400 ? 'expired' : 'unavailable');
    return response.json();
  }

  function status(id, message, retry, reload = false) {
    const target = byId(id);
    target.replaceChildren();
    target.append(document.createTextNode(message));
    if (retry) {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = 'Retry';
      button.onclick = async () => {
        button.disabled = true;
        try { await retry(); }
        finally { button.disabled = false; }
      };
      target.append(button);
    }
    if (retry || reload) {
      const link = document.createElement('a');
      link.href = '/admin';
      link.textContent = reload ? 'Reload dashboard' : 'Reload / sign in';
      target.append(link);
    }
  }

  function failed(id, error, retry) {
    const expired = error.message === 'expired';
    status(id, expired ? 'Pagination expired. Reload to start a new scan.'
      : error.message === 'membership' ? 'Organization membership required. Check GitHub sign-in.'
      : 'Metadata unavailable. Try again.', expired ? null : retry, expired);
  }

  function rollup() {
    const runRows = rows.filter(row => row.run_id !== null);
    byId('runs').replaceChildren();
    table(byId('runs'), 'Audited runs on loaded pages', ['Repository', 'Golem', 'Trigger', 'Seat', 'Execution', 'Observed model', 'Outcome', 'Duration (s)', 'Log'], rows.map(row => [row.repo, row.golem, row.trigger, row.seat, row.run_id === null ? 'not dispatched' : 'Actions', row.model, row.outcome, row.duration_s, row.log ? {log: row.log} : null]));

    const groups = new Map();
    let blocked = 0, known = 0, stopped = 0, incomplete = 0, unknownOutcome = 0;
    for (const row of rows) {
      if (row.run_id !== null) {
        const group = groups.get(row.seat) ?? {runs: 0, seconds: 0, unknown: 0};
        group.runs++;
        if (row.duration_s === null) group.unknown++;
        else group.seconds += row.duration_s;
        groups.set(row.seat, group);
      }
      if (row.outcome === null) unknownOutcome++;
      else known++;
      if (row.outcome === 'blocked') blocked++;
      if (row.outcome === 'stopped') stopped++;
      if (row.outcome === 'incomplete') incomplete++;
    }
    byId('usage').replaceChildren();
    table(byId('usage'), 'Usage from audited, dispatched runs on loaded pages', ['Person / provider', 'Runs', 'Observed time (s)', 'Unknown durations'], [...groups].sort(([a], [b]) => a.localeCompare(b)).map(([seat, group]) => [seat, group.runs, Math.round(group.seconds * 1000) / 1000, group.unknown]));

    const conclusions = [...workflowHealth.values()].filter(value => value !== null);
    const failures = conclusions.filter(value => ['failure', 'timed_out', 'startup_failure', 'action_required', 'stale'].includes(value)).length;
    const failureRate = conclusions.length ? `${(100 * failures / conclusions.length).toFixed(1)}%` : 'unknown';
    const health = document.createElement('dl');
    health.className = 'metrics';
    for (const [label, value] of [
      ['Workflow failures', conclusions.length ? `${failures} / ${conclusions.length} (${failureRate})` : 'unknown'],
      ['Audited runs loaded', runRows.length], ['Known outcomes', known],
      ['Blocked', blocked], ['Incomplete', incomplete], ['Stopped', stopped],
      ['Unknown outcomes', unknownOutcome], ['Missing audits', missing],
      ['Malformed audits', malformed + undispatchedMalformed], ['Pending workflows', pending],
      ['Refreshing seat expiry', 'unknown'],
    ]) {
      const group = document.createElement('div');
      const term = document.createElement('dt'); term.textContent = label;
      const detail = document.createElement('dd'); detail.textContent = String(value);
      group.append(term); group.append(detail); health.append(group);
    }
    byId('health').replaceChildren(); byId('health').append(health);
  }

  async function runs(page = 1) {
    const button = byId('more-runs');
    const retry = () => runs(page);
    button.disabled = true;
    button.hidden = false;
    button.onclick = retry;
    try {
      const data = await get(`/v1/admin/runs?page=${page}${windowEnd ? `&as_of=${windowEnd}` : ''}`);
      windowEnd = data.as_of;
      for (const run of data.workflow_health) workflowHealth.set(run.run_id, run.conclusion);
      for (const record of data.runs) {
        const key = `run:${record.run_id}`;
        if (!seen.has(key)) { seen.add(key); rows.push(record); }
      }
      missing += data.missing_audits;
      malformed += data.malformed_audits;
      pending += data.pending;
      rollup();
      const loadedRuns = rows.filter(row => row.run_id !== null).length;
      const coverage = data.listing_complete ? 'Workflow listing complete.' : data.next_page === null ? 'Scan limit reached; history incomplete.' : 'More workflows available.';
      const gaps = [[missing, 'missing audit'], [malformed + undispatchedMalformed, 'malformed audit'], [pending, 'pending workflow']]
        .filter(([count]) => count > 0).map(([count, label]) => `${count} ${label}${count === 1 ? '' : 's'}`).join(', ');
      status('runs-status', `${data.from.slice(0, 10)} to ${data.to.slice(0, 10)} UTC. ${loadedRuns ? `${loadedRuns} audited ${loadedRuns === 1 ? 'run' : 'runs'} loaded` : 'No audited runs on loaded pages'}; ${data.total_workflow_runs} workflows reported. ${coverage}${gaps ? ` History incomplete: ${gaps}.` : ''}`, null);
      button.hidden = data.next_page === null;
      button.onclick = data.next_page === null ? null : () => runs(data.next_page);
    } catch (error) { button.hidden = true; failed('runs-status', error, retry); }
    finally { button.disabled = false; }
  }

  async function undispatched(page = 1, after, asOf) {
    const button = byId('more-undispatched');
    const retry = () => undispatched(page, after, asOf);
    button.disabled = true;
    button.hidden = false;
    button.onclick = retry;
    try {
      const data = await get(`/v1/admin/undispatched?page=${page}${after ? `&after=${encodeURIComponent(after)}` : ''}${asOf ? `&as_of=${asOf}` : ''}`);
      for (const record of data.runs) {
        const key = `request:${record.request_id}`;
        if (!seen.has(key)) { seen.add(key); rows.push(record); }
      }
      undispatchedMalformed += data.malformed_audits;
      rollup();
      const coverage = data.listing_complete ? 'scan complete' : data.next_page === null ? 'scan limit reached; listing incomplete' : 'more records available';
      status('undispatched-status', `Requests before execution: ${coverage}.${undispatchedMalformed ? ` Malformed undispatched audits: ${undispatchedMalformed}.` : ''}`, null);
      button.hidden = data.next_page === null;
      button.onclick = data.next_page === null ? null : () => undispatched(data.next_page, data.after, data.as_of);
    } catch (error) { button.hidden = true; failed('undispatched-status', error, retry); }
    finally { button.disabled = false; }
  }

  async function seats() {
    try {
      const data = await get('/v1/admin/seats');
      status('seats-status', `${data.complete ? 'Membership listing complete.' : 'Membership listing partial; departed status may be unknown.'} Seat expiry is unavailable.`, null);
      byId('seats').replaceChildren();
      table(byId('seats'), 'Registered seats', ['Person / identifier', 'Provider', 'Membership', 'Identity', 'Refreshing', 'Expiry'], data.seats.map(seat => [seat.person, seat.provider, seat.membership, seat.identity, seat.refreshing ? 'yes' : 'no', seat.expires_at]));
    } catch (error) { failed('seats-status', error, seats); }
  }

  async function repositories(page = 1) {
    const button = byId('more-repositories');
    const retry = () => repositories(page);
    button.disabled = true;
    button.hidden = false;
    button.onclick = retry;
    try {
      const data = await get(`/v1/admin/repositories?page=${page}`);
      const parent = byId('repositories');
      for (const repository of data.repositories) {
        const heading = document.createElement('h3');
        if (/^isp-insoft-gmbh\/[A-Za-z0-9_.-]+$/.test(repository.repo) && !['.', '..'].includes(repository.repo.split('/')[1])) {
          const link = document.createElement('a');
          link.href = `https://github.com/${repository.repo}`; link.textContent = repository.repo;
          heading.append(link);
        } else heading.textContent = repository.repo;
        parent.append(heading);
        const state = document.createElement('p');
        state.textContent = `${repository.metadata_status === 'unavailable' ? 'Metadata unavailable; enabled state unknown' : repository.enabled ? 'Enabled' : 'Not enabled'}; dispatch ${repository.dispatch_allowlisted ? 'allowlisted' : 'not allowlisted'}.`;
        parent.append(state);
        if (repository.golems.length) table(parent, `Golems in ${repository.repo}`, ['Golem', 'State', 'Reason'], repository.golems.map(golem => [golem.name, golem.enabled ? 'enabled' : 'disabled', golem.reason ?? '']));
        else if (repository.enabled === true) {
          const next = document.createElement('p');
          next.textContent = 'Add a .golems/ definition to enable golems.';
          parent.append(next);
        }
      }
      status('repositories-status', `${data.total_repositories} installed repositories. ${data.complete ? 'Listing complete.' : data.next_page === null ? 'Scan limit reached; listing incomplete.' : 'More repositories available.'}`, null);
      button.hidden = data.next_page === null;
      button.onclick = data.next_page === null ? null : () => repositories(data.next_page);
    } catch (error) { button.hidden = true; failed('repositories-status', error, retry); }
    finally { button.disabled = false; }
  }

  void runs();
  void undispatched();
  void seats();
  void repositories();
})();
