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

  const el = (tag, text, className) => {
    const node = document.createElement(tag);
    if (text !== undefined) node.textContent = text;
    if (className) node.className = className;
    return node;
  };
  const button = (text, click) => {
    const control = el('button', text);
    control.type = 'button';
    control.onclick = click;
    return control;
  };
  const link = (text, url) => {
    const anchor = el('a', text); anchor.href = url; return anchor;
  };
  const operationLabel = (repo, golem) => `${repo} / ${golem}`;
  const validRepo = repo => /^isp-insoft-gmbh\/[A-Za-z0-9_.-]+$/.test(repo) && !['.', '..'].includes(repo.split('/')[1]);
  const readable = value => unknown(value).replaceAll('_', ' ');
  const time = value => typeof value === 'string' && Number.isFinite(Date.parse(value)) ? new Date(value).toLocaleString() : 'Unknown';
  const runLink = id => Number.isSafeInteger(id) && id > 0 ? link(`Ops run ${id}`, `https://github.com/isp-insoft-gmbh/isp-golem-xiv-ops/actions/runs/${id}`) : null;
  const facts = entries => {
    const list = el('dl', undefined, 'facts');
    for (const [key, value] of entries) list.append(el('dt', key), el('dd', unknown(value)));
    return list;
  };

  function searchable(id, label, describe, details, organize) {
    const parent = byId(id);
    const toolbar = el('div', undefined, 'toolbar');
    const search = el('input'); search.type = 'search'; search.setAttribute('aria-label', `Search ${label}`); search.placeholder = `Search ${label}`;
    const filter = el('select'); filter.setAttribute('aria-label', `Filter ${label} by state`);
    const count = el('span', undefined, 'muted'); count.setAttribute('role', 'status');
    toolbar.append(search, filter, count);
    const list = el(organize ? 'div' : 'ul', undefined, organize ? 'record-groups' : 'records');
    const detail = el('div', undefined, 'pad'); detail.hidden = true;
    parent.append(toolbar, list, detail);
    let model = [], selected = null, opener;
    const rowControls = new Map();
    function renderRows(target, entries) {
      for (const {meta} of entries) {
        const item = el('li');
        const row = button('', () => open(meta.key, meta.key));
        rowControls.set(meta.key, row);
        row.className = 'record';
        row.append(el('strong', meta.title));
        if (meta.note) row.append(el('small', meta.note));
        if (!organize || meta.state !== meta.group) row.append(el('span', readable(meta.state), organize ? 'row-state' : 'badge'));
        item.append(row);
        if (meta.references?.length) item.append(blockerReferences(meta.references, open, true));
        target.append(item);
      }
    }
    function paint() {
      toolbar.hidden = list.hidden = selected !== null;
      detail.hidden = selected === null;
      if (selected !== null) return;
      const query = search.value.trim().toLowerCase();
      const visible = model.filter(({meta}) => (!filter.value || meta.state === filter.value) && [meta.title, meta.note, meta.state, meta.key].join(' ').toLowerCase().includes(query));
      list.replaceChildren();
      count.textContent = `${visible.length} of ${model.length} loaded`;
      rowControls.clear();
      if (organize) {
        for (const {title, lanes} of organize(visible)) {
          const group = el('section', undefined, 'record-group');
          group.append(el('h3', title));
          for (const {entries, label, style} of lanes) {
            if (!entries.length) continue;
            const lane = el('ul', undefined, `records ${style ?? ''}`.trim());
            if (label) lane.setAttribute('aria-label', label);
            renderRows(lane, entries);
            group.append(lane);
          }
          list.append(group);
        }
      } else renderRows(list, visible);
      if (!visible.length) list.append(el(organize ? 'p' : 'li', model.length ? 'No loaded records match.' : 'No records loaded.', 'pad'));
    }
    function open(key, origin) {
      const entry = model.find(({meta}) => meta.key === key);
      if (!entry) return;
      opener = origin ?? opener;
      selected = key;
      const back = button(`Back to ${label}`, () => {
        selected = null; detail.replaceChildren(); paint();
        (rowControls.get(opener) ?? search).focus();
        opener = undefined;
      });
      const heading = el('h2', entry.meta.title); heading.tabIndex = -1;
      detail.replaceChildren(back, heading, el('p', readable(entry.meta.state), 'muted'));
      details(detail, entry.data, open);
      paint(); heading.focus();
    }
    search.oninput = filter.onchange = paint;
    return {set(data, {reset = false} = {}) {
      const former = filter.value;
      model = data.map(data => ({data, meta: describe(data)}));
      filter.replaceChildren(new Option('All states', ''), ...[...new Set(model.map(({meta}) => meta.state))].map(state => new Option(readable(state), state)));
      filter.value = [...filter.options].some(option => option.value === former) ? former : '';
      if (reset || !model.some(({meta}) => meta.key === selected)) { selected = null; detail.replaceChildren(); }
      paint();
    }};
  }

  function route() {
    const hash = location.hash.slice(1);
    const pages = [...document.querySelectorAll('[data-view]')];
    const page = pages.find(node => node.dataset.view === hash)?.dataset.view ?? 'operations';
    for (const node of pages) node.hidden = node.dataset.view !== page;
    for (const tab of document.querySelectorAll('[data-page]')) {
      if (tab.dataset.page === page) tab.setAttribute('aria-current', 'page'); else tab.removeAttribute('aria-current');
    }
  }
  window.addEventListener('hashchange', route);
  document.querySelector('.skip').onclick = event => { event.preventDefault(); byId('main').focus(); };
  route();

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
    historyList.set(rows);

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

  // Seat secret names hold logins uppercased with '-' as '_'; departed seats are known only in that form.
  const seatKey = (login, provider) => `${login.toUpperCase().replaceAll('-', '_')}/${provider}`;

  // Each seat's latest audited runs on loaded pages; sign-in expiry and refresh state are not recorded.
  function renderSeats() {
    if (!seatData) return;
    const last = new Map();
    for (const row of rows) {
      if (row.run_id === null || typeof row.completed_at !== 'string') continue;
      const kind = ['changed', 'replied', 'no_change'].includes(row.outcome) ? 'success' : row.outcome === 'blocked' ? 'blocked' : null;
      const slash = row.seat.lastIndexOf('/');
      if (!kind || slash < 1) continue;
      const key = seatKey(row.seat.slice(0, slash), row.seat.slice(slash + 1));
      const entry = last.get(key) ?? {};
      if (!entry[kind] || row.completed_at > entry[kind]) entry[kind] = row.completed_at;
      last.set(key, entry);
    }
    byId('seats').replaceChildren();
    table(byId('seats'), 'Registered seats', ['Person / identifier', 'Provider', 'Membership', 'Identity', 'Refreshing', 'Last successful run', 'Last blocked run'], seatData.seats.map(seat => {
      const entry = last.get(seatKey(seat.person, seat.provider)) ?? {};
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
    const review = el('div');
    const preview = button('Review starter PR', async () => {
      preview.disabled = true;
      review.replaceChildren(el('p', 'Loading starter PR preview…'));
      try {
        const data = await get(`/v1/admin/onboarding/preview?repository=${encodeURIComponent(repo)}`);
        const existing = starterLink(data.existing, repo);
        if (existing) {
          review.replaceChildren(el('p', 'Existing starter PR (may be closed).'), existing);
          return;
        }
        review.replaceChildren(facts([['Title', data.pull.title], ['Head', data.pull.head], ['Base', data.pull.base]]), el('pre', data.pull.body));
        for (const [path, content] of data.files) {
          const file = el('details'); file.append(el('summary', path), el('pre', content)); review.append(file);
        }
        review.append(el('p', 'Opens a PR only. No run or default-branch change. An existing PR is returned unchanged.'), submit, result);
        submit.focus();
      } catch (error) {
        review.replaceChildren(el('p', error.message === 'membership' ? 'Organization membership and repository write access required.' : 'Preview unavailable. Check repository state in GitHub and retry.'));
      } finally { preview.disabled = false; }
    });
    const submit = document.createElement('button');
    submit.type = 'button';
    submit.textContent = 'Create starter PR';
    submit.setAttribute('aria-label', `Create starter PR for ${repo}`);
    const result = document.createElement('div');
    result.setAttribute('role', 'status');
    result.setAttribute('aria-live', 'polite');
    submit.onclick = async () => {
      if (submit.disabled) return;
      submit.disabled = true;
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
        const anchor = starterLink(data.url, repo);
        if (!anchor || typeof data.created !== 'boolean' || !((response.status === 201 && data.created) || (response.status === 200 && !data.created))) {
          result.textContent = 'Unexpected response. Retry explicitly to check for an existing request.';
          return;
        }
        result.textContent = data.created ? 'Starter PR created: ' : 'Existing starter PR, which may be closed: ';
        result.append(anchor);
        if (data.created) result.append(el('p', 'Review and merge in GitHub. Then register a seat and mention @isp-golem-xiv hello.'));
      } catch (_) {
        result.textContent = 'Request outcome may be unknown. Retry explicitly to check for an existing request.';
      } finally { submit.disabled = false; }
    };
    const action = el('div', undefined, 'action-block');
    action.append(preview, review);
    parent.append(action);
  }

  function starterLink(url, repo) {
    const prefix = `https://github.com/${repo}/pull/`;
    return typeof url === 'string' && url.startsWith(prefix) && /^[1-9][0-9]*$/.test(url.slice(prefix.length)) ? link('View starter PR', url) : null;
  }

  async function repositories(page = 1) {
    const button = byId('more-repositories');
    const retry = () => repositories(page);
    button.disabled = true;
    button.hidden = false;
    button.onclick = retry;
    try {
      const data = await get(`/v1/admin/repositories?page=${page}`);
      for (const repository of data.repositories) loadedRepositories.set(repository.repo, repository);
      repositoryList.set([...loadedRepositories.values()]);
      status('repositories-status', `${data.total_repositories} installed repositories. ${data.complete ? 'Listing complete.' : data.next_page === null ? 'Scan limit reached; listing incomplete.' : 'More repositories available.'}`, null);
      button.hidden = data.next_page === null;
      button.onclick = data.next_page === null ? null : () => repositories(data.next_page);
    } catch (error) { button.hidden = true; failed('repositories-status', error, retry); }
    finally { button.disabled = false; }
  }

  const historyList = searchable('runs', 'run history', row => ({key: row.run_id ?? row.request_id, title: operationLabel(row.repo, row.golem), state: row.outcome ?? 'unknown', note: `${row.seat} · ${time(row.completed_at)}`}), (parent, row) => {
    parent.append(facts([['Trigger', row.trigger], ['Paying seat', row.seat], ['Outcome', readable(row.outcome)], ['Execution', row.run_id === null ? 'Not dispatched' : 'GitHub Actions'], ['Observed model', row.model], ['Duration (s)', row.duration_s], ['Completed', time(row.completed_at)]]));
    if (row.log && /^https:\/\/github\.com\/isp-insoft-gmbh\/isp-golem-xiv-ops\/actions\/runs\/[1-9][0-9]*$/.test(row.log)) parent.append(link('Open private run log', row.log));
  });
  const loadedRepositories = new Map();
  const repoGroups = ['Configured', 'Not configured', 'Unknown'];
  function repositorySummary(repo) {
    if (repo.metadata_status !== 'available' || typeof repo.enabled !== 'boolean') return {group: 'Unknown', state: 'Unknown'};
    if (!repo.enabled) return {group: 'Not configured', state: 'Not configured'};
    const enabled = repo.golems.filter(golem => golem.enabled).length;
    const disabled = repo.golems.length - enabled;
    return {group: 'Configured', state: disabled ? enabled ? 'Mixed' : 'Disabled' : enabled ? 'Configured' : 'No definitions',
      note: [enabled ? `${enabled} enabled` : '', disabled ? `${disabled} disabled` : ''].filter(Boolean).join(' · ')};
  }
  const repositoryList = searchable('repositories', 'repositories', repo => ({key: repo.repo, title: repo.repo, ...repositorySummary(repo)}), (parent, repo) => {
    if (!validRepo(repo.repo)) return;
    parent.append(link('Open repository', `https://github.com/${repo.repo}`));
    const {group, state} = repositorySummary(repo);
    if (group === 'Unknown') return;
    if (group === 'Not configured') onboarding(parent, repo.repo);
    if (state === 'No definitions') parent.append(el('p', 'Add a .golems/ definition to enable work.'));
    if (!repo.golems.length) return;
    const definitions = el('ul', undefined, 'definitions');
    for (const golem of repo.golems) {
      const definition = el('li');
      if (/^[A-Za-z0-9_-]{1,64}$/.test(golem.name)) definition.append(link(`.golems/${golem.name}.md`, `https://github.com/${repo.repo}/blob/HEAD/.golems/${golem.name}.md`));
      else definition.append(el('span', golem.name));
      if (!golem.enabled) definition.append(el('small', golem.reason ?? 'Disabled'));
      definitions.append(definition);
    }
    parent.append(definitions);
  }, visible => repoGroups.flatMap(title => {
    const entries = visible.filter(({meta}) => meta.group === title);
    return entries.length ? [{title, lanes: [{entries}]}] : [];
  }));

  let queueRows = [];
  let snapshotAt;
  let nextCandidates = new Set();
  const queueState = row => readable(row.state);
  const queueTitle = row => operationLabel(row.resource.slice(0, row.resource.lastIndexOf('/')), row.resource.split('/').at(-1));
  function operationTab(queue) {
    byId('queue-section').hidden = !queue;
    byId('runs-section').hidden = queue;
    byId('queue-tab').setAttribute('aria-pressed', String(queue));
    byId('history-tab').setAttribute('aria-pressed', String(!queue));
  }
  byId('history-tab').onclick = () => operationTab(false);
  byId('queue-tab').onclick = () => operationTab(true);
  function queueDenied() {
    queueRows = [];
    queueList.set([], {reset: true});
    byId('queue-status').textContent = '';
    byId('queue-tab').hidden = true;
    byId('queue-access').replaceChildren(document.createTextNode('Queue is restricted to active organization owners. '), link('Sign in again', '/v1/admin/queue/sign-in'));
    operationTab(false);
  }
  async function refreshQueue() {
    const refresh = byId('queue-refresh');
    refresh.disabled = true;
    try {
      const data = await get('/v1/admin/queue');
      queueRows = data.rows;
      snapshotAt = data.snapshot_at;
      nextCandidates = new Set(data.next);
      queueList.set(queueRows, {reset: true});
      byId('queue-access').replaceChildren();
      byId('queue-tab').hidden = false;
      const unknownCount = queueRows.filter(row => !row.request_verified).length;
      byId('queue-status').textContent = `${queueRows.length} requests · Snapshot ${time(data.snapshot_at)}.${unknownCount ? ` Request evidence unavailable for ${unknownCount}.` : ''}`;

    } catch (error) {
      queueDenied();
      if (error.message !== 'membership') byId('queue-access').replaceChildren(document.createTextNode('Queue unavailable. '), button('Retry queue lookup', refreshQueue));
    } finally { refresh.disabled = false; }
  }
  byId('queue-refresh').onclick = refreshQueue;

  const refusal = {
    terminal: ['Already terminal.', 'There is no remaining reservation to release.'],
    not_pending: ['No longer pending and undispatched.', 'Only the exact pending request with no dispatch intent, run or attempt can be cancelled here.'],
    not_running: ['No dispatched run.', 'Wait for normal dispatch; there is no execution to verify.'],
    unbound: ['No trusted bound run. The reservation stays held.', 'A similar workflow does not establish identity. Ask the operations owner to investigate.'],
    identity: ['Exact request or workflow identity not verified.', 'Check the immutable request and bound run. No change was made.'],
    check: ['Original target or queued check not verified.', 'Inspect the target check before retrying. No change was made.'],
    missing_run: ['Bound workflow unavailable.', 'Inspect the private ops run before rechecking.'],
    not_terminal: ['Terminal workflow state not verified.', 'Inspect the run and recheck after completion.'],
    audit: ['No verified completed check or trusted audit.', 'The workflow ended, but terminal status alone does not prove the outcome. Leave the reservation to normal reconciliation.'],
  };
  async function queuePost(path, body) {
    const response = await fetch(`/v1/admin/queue/${path}`, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store', redirect: 'error',
      headers: {'Content-Type': 'application/json', 'X-Golem-Csrf': byId('onboarding-csrf').value}, body: JSON.stringify(body),
    });
    if (response.status === 403) { queueDenied(); throw new Error('denied'); }
    if (response.status === 409 || response.status === 404) throw new Error('changed');
    if (!response.ok) throw new Error('unknown');
    return response.json();
  }
  function queueActions(parent, row) {
    if (row.state === 'terminal') return;
    const actions = el('section', undefined, 'action-block');
    const result = el('div');
    const announcement = el('p', '', 'sr-only'); announcement.setAttribute('role', 'status');
    const cancel = row.state === 'pending' && !row.dispatched && row.run_id === null && row.attempt === null;
    const operation = cancel ? 'cancel' : 'release';
    const heading = el('h3', cancel ? 'Cancel pending request' : 'Verify and release');
    const missing = el('p', row.dispatched && row.run_id === null ? refusal.unbound[0] : !row.request_verified ? 'Immutable request not verified.' : cancel ? '' : 'Terminal evidence not checked.');
    missing.hidden = !missing.textContent;
    const help = el('details'); help.hidden = true;
    const summary = el('summary', 'Why?');
    const guidance = el('p');
    const observation = el('div');
    help.append(summary, guidance, observation);
    function explain(fact, text, run) {
      summary.setAttribute('aria-label', `Why? ${fact}`);
      guidance.textContent = text;
      observation.replaceChildren();
      if (run && !run.status) observation.append(facts([['Workflow conclusion', readable(run.conclusion)], ['Workflow last update', time(run.updated_at)]]));
      help.hidden = false;
    }
    if (row.dispatched && row.run_id === null) explain(...refusal.unbound);
    function tell(message) { announcement.textContent = message; }
    const inspect = button(cancel ? 'Review pending cancellation' : 'Recheck release evidence', async () => {
      inspect.disabled = true;
      result.replaceChildren();
      tell('Checking current state.');
      try {
        const review = await queuePost('preview', {request_id: row.id, operation});
        if (!review.eligible) {
          const [fact, guide] = refusal[review.reason] ?? ['Evidence unavailable.', 'No change was made.'];
          missing.textContent = review.run?.status ? `Workflow ${readable(review.run.status)}. No terminal proof.` : fact;
          missing.hidden = false;
          explain(missing.textContent, guide, review.run);
          tell(missing.textContent);
          return;
        }
        const request = review.request;
        missing.hidden = true;
        help.hidden = true;
        result.replaceChildren(facts([['Request', request.id], ['Repository / golem', queueTitle(request)], ['Paying seat', request.seat]]));
        if (review.run) {
          result.append(facts([['Run / attempt', `${review.run.id} / ${review.run.attempt}`], ['Workflow conclusion', readable(review.run.conclusion)], ['Workflow last update', time(review.run.updated_at)], ['Audited outcome', readable(review.outcome)]]));
          const anchor = runLink(review.run.id); if (anchor) result.append(anchor);
        }
        result.append(el('p', cancel ? 'Stop only this never-dispatched request. Normal maintenance completes its check; no workflow is cancelled.' : 'Release only this request’s seat and repository/golem reservation. No workflow will be cancelled or dispatched by this action.'));
        if (review.unblocked.length) result.append(el('p', `${cancel ? 'After normal maintenance, queue blockers' : 'Queue blockers'} would clear for: ${review.unblocked.map(item => `${queueTitle(item)} (${item.id})`).join('; ')}. No dispatch is promised.`));
        const confirm = button(cancel ? 'Confirm pending cancellation' : 'Confirm verified release', async () => {
          inspect.disabled = confirm.disabled = true;
          try {
            const done = await queuePost('confirm', {request_id: row.id, operation, expires: review.expires, token: review.token});
            const message = cancel ? 'Cancellation recorded. Normal maintenance completes the check.' : 'Verified release recorded. This request is terminal.';
            tell(message);
            result.replaceChildren(el('p', message), facts([['Request', done.request.id], ['State', queueState(done.request)], ['Operator', `@${done.audit.actor_login}`], ['Recorded', time(done.audit.at)], ['Journal revision', done.journal_sha]]));
            if (done.audit.evidence_sha) result.append(facts([['Immutable run audit', done.audit.evidence_sha]]));
            const refresh = button('Refresh queue snapshot', refreshQueue);
            result.append(refresh);
            refresh.focus();
          } catch (error) {
            if (error.message === 'denied') return;
            const message = error.message === 'changed' ? 'Preview stale or expired. Recheck current evidence before confirming.' : 'Outcome unconfirmed. Refresh the queue before trying again.';
            missing.textContent = message; missing.hidden = false;
            result.replaceChildren(); tell(message);
            inspect.disabled = false; inspect.focus();
          }
        });
        const controls = el('div', undefined, 'action-controls');
        controls.append(confirm, button('Discard preview', () => { result.replaceChildren(); inspect.focus(); }));
        result.append(el('p', `Confirm before ${time(new Date(review.expires).toISOString())}.`), controls);
        confirm.focus();
      } catch (error) {
        if (error.message === 'denied') return;
        missing.textContent = error.message === 'changed' ? 'Request no longer in this snapshot. Refresh the queue.' : 'Could not check current evidence. No action submitted.';
        missing.hidden = false; tell(missing.textContent);
      } finally { inspect.disabled = false; }
    });
    actions.append(heading, missing, help, inspect, announcement, result);
    parent.append(actions);
  }
  function waitAge(since) {
    if (!since) return 'Unknown';
    const minutes = Math.floor((Date.parse(snapshotAt) - Date.parse(since)) / 60000);
    if (!Number.isFinite(minutes) || minutes < 0) return 'Unknown';
    return minutes < 1 ? 'Less than 1 min' : minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
  }
  function blockerReferences(blockers, open, compact = false) {
    const list = el('ul', undefined, 'blocker-references');
    list.setAttribute('aria-label', 'Blocking requests');
    for (const blocker of blockers) {
      const item = el('li');
      const target = button(`${queueTitle(blocker)} · ${blocker.seat} · ${compact ? blocker.id.slice(-8) : blocker.id}`, () => open(blocker.id));
      target.className = 'reference';
      item.append(el('span', `${blocker.on}: `), target);
      list.append(item);
    }
    return list;
  }
  function queueGroups(visible) {
    const seats = new Map();
    for (const entry of visible) {
      const {seat, state} = entry.data;
      if (!seats.has(seat)) seats.set(seat, [[], [], []]);
      // Non-pending, non-terminal rows fence resources. Reserved and stop-requested
      // rows belong here too; a reservation does not imply a workflow is running.
      seats.get(seat)[state === 'pending' ? 1 : state === 'terminal' ? 2 : 0].push(entry);
    }
    return [...seats].map(([title, [held, pending, terminal]]) => ({title, lanes: [
      {entries: held, label: 'Seat reservations', style: 'reservations'},
      {entries: pending, label: 'Pending requests in journal order', style: held.length ? 'waiting' : ''},
      {entries: terminal, label: 'Terminal requests', style: 'closed'},
    ]}));
  }
  function queueSummary(row) {
    const waiting = row.state === 'pending' || row.state === 'reserved';
    const age = waiting ? waitAge(row.waiting_since) : '';
    const note = [age === 'Unknown' ? 'Age unknown' : age, row.run_id !== null ? `Run ${row.run_id}` : row.dispatched ? 'No bound run' : '',
      !row.request_verified ? 'Request unverified' : nextCandidates.has(row.id) ? 'Next candidate' : ''].filter(Boolean).join(' · ');
    return {key: row.id, title: queueTitle(row), state: row.state, note,
      references: row.blockers.filter(blocker => blocker.seat !== row.seat)};
  }
  const queueList = searchable('queue-list', 'queue', queueSummary, (parent, row, open) => {
    const waiting = row.state === 'pending' || row.state === 'reserved';
    parent.append(facts([['Request', row.id], ['Paying seat', row.seat], ['Trigger', row.trigger], ['Harness', row.harness], ['Run / attempt', row.run_id === null ? row.dispatched ? 'Unknown: dispatch intent without a binding' : 'No dispatch intent recorded' : `${row.run_id} / ${row.attempt} · recorded binding`], ...(waiting ? [['Waiting age at snapshot', waitAge(row.waiting_since)], ['Request created', time(row.waiting_since)]] : [])]));
    queueActions(parent, row);
    const anchor = runLink(row.run_id);
    if (anchor) parent.append(anchor);
    const repo = row.resource.slice(0, row.resource.lastIndexOf('/'));
    if (validRepo(repo)) {
      const golem = row.resource.split('/').at(-1);
      const paths = el('p');
      paths.append(link(`.golems/${golem}.md`, `https://github.com/${repo}/blob/HEAD/.golems/${golem}.md`));
      if (row.check_id) paths.append(document.createTextNode(' · '), link('Target check', `https://github.com/${repo}/runs/${row.check_id}`));
      parent.append(paths);
    }
    if (row.blockers.length) {
      parent.append(el('h3', 'Blocked by'), blockerReferences(row.blockers, open));
    }
  }, queueGroups);

  void refreshQueue();
  void runs();
  void undispatched();
  void seats();
  void repositories();
})();
