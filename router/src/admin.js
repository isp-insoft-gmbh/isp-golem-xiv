'use strict';
// Metadata stays text. No innerHTML, credential access, local storage, or external requests.
(() => {
  const byId = id => document.getElementById(id);
  const rows = [];
  const seen = new Set();
  const workflowHealth = new Map();
  let windowEnd, seatData;
  let missing = 0, beforeAudits = 0, malformed = 0, pending = 0, undispatchedMalformed = 0;
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
      ['Unknown outcomes', unknownOutcome], ['Missing audits', missing], ['Runs before audits', beforeAudits],
      ['Malformed audits', malformed + undispatchedMalformed], ['Pending workflows', pending],
      ['Seat refresh state', 'not recorded'],
    ]) {
      const group = document.createElement('div');
      const term = document.createElement('dt'); term.textContent = label;
      const detail = document.createElement('dd'); detail.textContent = String(value);
      group.append(term); group.append(detail); health.append(group);
    }
    byId('health').replaceChildren(); byId('health').append(health);
    renderSeats();
  }

  // Each seat's latest audited runs on loaded pages; sign-in expiry and refresh state are not recorded.
  function renderSeats() {
    if (!seatData) return;
    const last = new Map();
    for (const row of rows) {
      if (row.run_id === null || typeof row.completed_at !== 'string') continue;
      const kind = ['changed', 'replied', 'no_change'].includes(row.outcome) ? 'success' : row.outcome === 'blocked' ? 'blocked' : null;
      if (!kind) continue;
      const entry = last.get(row.seat) ?? {};
      if (!entry[kind] || row.completed_at > entry[kind]) entry[kind] = row.completed_at;
      last.set(row.seat, entry);
    }
    byId('seats').replaceChildren();
    table(byId('seats'), 'Registered seats', ['Person / identifier', 'Provider', 'Membership', 'Identity', 'Refreshing', 'Last successful run', 'Last blocked run'], seatData.seats.map(seat => {
      const entry = last.get(`${seat.person.toLowerCase()}/${seat.provider}`) ?? {};
      return [seat.person, seat.provider, seat.membership, seat.identity, seat.refreshing ? 'yes' : 'no', entry.success ?? null, entry.blocked ?? null];
    }));
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
      beforeAudits += data.before_audits;
      malformed += data.malformed_audits;
      pending += data.pending;
      rollup();
      const loadedRuns = rows.filter(row => row.run_id !== null).length;
      const coverage = data.listing_complete ? 'Workflow listing complete.' : data.next_page === null ? 'Scan limit reached; history incomplete.' : 'More workflows available.';
      const gaps = [[missing, 'missing audit'], [malformed + undispatchedMalformed, 'malformed audit'], [pending, 'pending workflow']]
        .filter(([count]) => count > 0).map(([count, label]) => `${count} ${label}${count === 1 ? '' : 's'}`).join(', ');
      const earlier = beforeAudits ? ` ${beforeAudits} earlier ${beforeAudits === 1 ? 'run predates' : 'runs predate'} audits (before 2026-11-13).` : '';
      status('runs-status', `${data.from.slice(0, 10)} to ${data.to.slice(0, 10)} UTC. ${loadedRuns ? `${loadedRuns} audited ${loadedRuns === 1 ? 'run' : 'runs'} loaded` : 'No audited runs on loaded pages'}; ${data.total_workflow_runs} workflows reported. ${coverage}${gaps ? ` History incomplete: ${gaps}.` : ''}${earlier}`, null);
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
      seatData = await get('/v1/admin/seats');
      status('seats-status', `${seatData.complete ? 'Membership listing complete.' : 'Membership listing partial; departed status may be unknown.'} Last runs come from audited runs on loaded pages; sign-in expiry and refresh state are not recorded.`, null);
      renderSeats();
    } catch (error) { failed('seats-status', error, seats); }
  }

  function onboarding(parent, repo) {
    const explanation = document.createElement('p');
    explanation.textContent = 'Create a pull request with reviewable starter files. No agents or default-branch changes are made.';
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = 'Create starter PR';
    button.setAttribute('aria-label', `Create starter PR for ${repo}`);
    const result = document.createElement('p');
    result.setAttribute('role', 'status');
    result.setAttribute('aria-live', 'polite');
    button.onclick = async () => {
      if (button.disabled) return;
      button.disabled = true;
      result.textContent = 'Checking for or creating a starter PR…';
      try {
        const response = await fetch('/v1/admin/onboarding', {
          method: 'POST', credentials: 'same-origin', cache: 'no-store', redirect: 'error',
          headers: {'Content-Type': 'application/json', 'X-Golem-Csrf': byId('onboarding-csrf').value},
          body: JSON.stringify({repository: repo})
        });
        if (!response.ok) {
          result.textContent = response.status === 401 ? 'Sign in to continue.' : response.status === 403 ? 'Active organization membership and repository write access required.' : response.status === 409 ? 'Starter PR unavailable: repository may be configured, archived, or have an incomplete request. Check GitHub before retrying.' : 'Request failed. Retry explicitly to check for an existing request.';
          return;
        }
        const data = await response.json();
        const prefix = `https://github.com/${repo}/pull/`;
        if (typeof data.url !== 'string' || !data.url.startsWith(prefix) || !/^[1-9][0-9]*$/.test(data.url.slice(prefix.length)) || typeof data.created !== 'boolean' || !((response.status === 201 && data.created) || (response.status === 200 && !data.created))) {
          result.textContent = 'Unexpected response. Retry explicitly to check for an existing request.';
          return;
        }
        result.textContent = data.created ? 'Starter PR created: ' : 'Existing starter PR, which may be closed: ';
        const link = document.createElement('a');
        link.href = data.url;
        link.textContent = 'View starter PR';
        result.append(link);
      } catch (_) {
        result.textContent = 'Request outcome may be unknown. Retry explicitly to check for an existing request.';
      } finally { button.disabled = false; }
    };
    parent.append(explanation);
    parent.append(button);
    parent.append(result);
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
        const validRepo = /^isp-insoft-gmbh\/[A-Za-z0-9_.-]+$/.test(repository.repo) && !['.', '..'].includes(repository.repo.split('/')[1]);
        if (validRepo) {
          const link = document.createElement('a');
          link.href = `https://github.com/${repository.repo}`; link.textContent = repository.repo;
          heading.append(link);
        } else heading.textContent = repository.repo;
        parent.append(heading);
        const state = document.createElement('p');
        state.textContent = `${repository.metadata_status === 'unavailable' ? 'Metadata unavailable; enabled state unknown' : repository.enabled ? 'Enabled' : 'Not enabled'}.`;
        parent.append(state);
        if (validRepo && repository.metadata_status === 'available' && repository.enabled === false) onboarding(parent, repository.repo);
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
