'use strict';

const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));

const state = {
  results: [],
  saved: [],
  running: false,
  sortKey: 'speed',
  sortDir: -1,
  filter: '',
  hasToken: false,
};

const SIDEBAR_MIN = 280;
const SIDEBAR_MAX = 520;
const SIDEBAR_STORAGE_KEY = 'cfst-sidebar-width';

// ── 工具 ────────────────────────────────────────
async function api(path, opts) {
  const r = await fetch(path, opts);
  if (!r.ok) {
    let msg = r.statusText;
    try {
      const j = await r.json();
      msg = j.error || j.message || msg;
    } catch (_) {}
    throw new Error(msg);
  }
  return r.status === 204 ? null : r.json();
}

function toast(msg, kind) {
  const el = document.createElement('div');
  el.className = 'toast' + (kind ? ' ' + kind : '');
  el.textContent = msg;
  $('#toasts').appendChild(el);
  setTimeout(() => el.remove(), 3600);
}

function timeStr(ts) {
  const d = new Date(ts);
  const p = (n) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

function setupSidebarResize() {
  const layout = $('.layout');
  const handle = $('#sidebarResize');
  if (!layout || !handle) return;

  const saved = Number(localStorage.getItem(SIDEBAR_STORAGE_KEY));
  if (Number.isFinite(saved)) {
    const width = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, saved));
    layout.style.setProperty('--sidebar-width', `${width}px`);
  }

  let startX = 0;
  let startWidth = 0;

  handle.addEventListener('pointerdown', (event) => {
    if (window.matchMedia('(max-width: 900px)').matches) return;
    startX = event.clientX;
    startWidth = layout.querySelector('.panel').getBoundingClientRect().width;
    layout.classList.add('resizing');
    handle.setPointerCapture(event.pointerId);
  });

  handle.addEventListener('pointermove', (event) => {
    if (!layout.classList.contains('resizing')) return;
    const width = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, startWidth + event.clientX - startX));
    layout.style.setProperty('--sidebar-width', `${width}px`);
    handle.setAttribute('aria-valuenow', Math.round(width));
  });

  const stopResize = (event) => {
    if (!layout.classList.contains('resizing')) return;
    const width = Math.round(layout.querySelector('.panel').getBoundingClientRect().width);
    layout.classList.remove('resizing');
    localStorage.setItem(SIDEBAR_STORAGE_KEY, String(width));
    if (event.pointerId !== undefined && handle.hasPointerCapture(event.pointerId)) {
      handle.releasePointerCapture(event.pointerId);
    }
  };

  handle.addEventListener('pointerup', stopResize);
  handle.addEventListener('pointercancel', stopResize);
}

// ── 参数采集 / 回填 ─────────────────────────────
function collect() {
  // --dn 允许填 0（不设上限），不能用 `|| 默认值` 兜底，否则 0 会被吃掉
  const dnRaw = $('#inDnNum').value.trim();
  const dn = dnRaw === '' ? 10 : Math.max(0, Number(dnRaw) || 0);
  return {
    routines: +$('#inRoutines').value || 200,
    ping_times: +$('#inPingTimes').value || 4,
    download_num: dn,
    download_time: +$('#inDnTime').value || 10,
    port: +$('#inPort').value || 443,
    url: $('#inUrl').value.trim(),
    httping: $('#segPing button.on').dataset.ping === 'http',
    httping_code: +$('#inHttpCode').value || 0,
    cfcolo: $('#inColo').value.trim(),
    max_delay: +$('#inMaxDelay').value || 9999,
    min_delay: +$('#inMinDelay').value || 0,
    max_loss_rate: Math.min(1, Math.max(0, +$('#inLoss').value)),
    min_speed: +$('#inMinSpeed').value || 0,
    print_num: +$('#inPrintNum').value,
    ip_file: 'ip.txt',
    ip_text: $('#inIpText').value.trim(),
    output: $('#inOutput').value.trim() || 'result.csv',
    disable_download: $('#segDl button.on').dataset.dl === 'off',
    debug: $('#inDebug').checked,
  };
}

function toCliArgs(c) {
  const a = [];
  if (c.routines !== 200) a.push(`-n ${c.routines}`);
  if (c.ping_times !== 4) a.push(`-t ${c.ping_times}`);
  if (c.download_num !== 10) a.push(`--dn ${c.download_num}`);
  if (c.download_time !== 10) a.push(`--dt ${c.download_time}`);
  if (c.port !== 443) a.push(`--tp ${c.port}`);
  if (c.url && c.url !== 'https://speed.cloudflare.com/__down?bytes=99000000') a.push(`--url ${c.url}`);
  if (c.httping) a.push('--httping');
  if (c.httping_code) a.push(`--httping-code ${c.httping_code}`);
  if (c.cfcolo) a.push(`--cfcolo ${c.cfcolo}`);
  if (c.max_delay !== 9999) a.push(`--tl ${c.max_delay}`);
  if (c.min_delay) a.push(`--tll ${c.min_delay}`);
  if (c.max_loss_rate !== 1) a.push(`--tlr ${c.max_loss_rate}`);
  if (c.min_speed) a.push(`--sl ${c.min_speed}`);
  if (c.print_num !== 10) a.push(`-p ${c.print_num}`);
  if (c.ip_file && c.ip_file !== 'ip.txt') a.push(`-f ${c.ip_file}`);
  if (c.ip_text) a.push(`--ip ${c.ip_text.replace(/\n/g, ',')}`);
  if (c.output && c.output !== 'result.csv') a.push(`-o ${c.output}`);
  if (c.disable_download) a.push('--dd');
  if (c.debug) a.push('--debug');
  return a.length ? a.join(' ') : '（全部默认）';
}

function fillForm(c) {
  $('#inRoutines').value = c.routines;
  $('#inPingTimes').value = c.ping_times;
  $('#inDnNum').value = c.download_num;
  $('#inDnTime').value = c.download_time;
  $('#inPort').value = c.port;
  $('#inUrl').value = c.url;
  $('#inHttpCode').value = c.httping_code;
  $('#inColo').value = c.cfcolo;
  $('#inMaxDelay').value = c.max_delay;
  $('#inMinDelay').value = c.min_delay;
  $('#inLoss').value = c.max_loss_rate;
  $('#inMinSpeed').value = c.min_speed;
  $('#inPrintNum').value = c.print_num;
  $('#inIpText').value = c.ip_text;
  $('#inOutput').value = c.output;
  $('#inDebug').checked = c.debug;
  setSeg('#segPing', 'ping', c.httping ? 'http' : 'tcp');
  setSeg('#segDl', 'dl', c.disable_download ? 'off' : 'on');
  refreshDerived();
}

function loadIpFile(file) {
  const reader = new FileReader();
  reader.onload = () => {
    $('#inIpText').value = String(reader.result || '').trim();
    $('#ipFileName').textContent = `已读入本地文件：${file.name}（内容已放进上方输入框，可再编辑）`;
    refreshDerived();
    toast('IP 段文件已读取', 'ok');
  };
  reader.onerror = () => toast('无法读取 IP 段文件', 'err');
  reader.readAsText(file);
}

function setSeg(sel, key, val) {
  $$(sel + ' button').forEach((b) => b.classList.toggle('on', b.dataset[key] === val));
}

function refreshDerived() {
  const httping = $('#segPing button.on').dataset.ping === 'http';
  $$('.httping-only').forEach((el) => el.classList.toggle('hidden', !httping));
  $('#pingNote').textContent = httping
    ? '真实 HTTP 请求，慢一些，但能识别地区码'
    : 'TCP 握手，快；拿不到地区码';
  $('#cliArgs').textContent = toCliArgs(collect());
}

// ── 任务控制 ────────────────────────────────────
async function start() {
  const cfg = collect();
  if (!cfg.ip_text && !cfg.ip_file) {
    toast('请指定 IP 段文件或直接填 IP 段', 'err');
    return;
  }
  // 当前轮结果会被新一轮覆盖，先提醒保留
  if (state.results.length && !state.saved.length) {
    if (!confirm(`当前有 ${state.results.length} 个结果尚未保留，开始新一轮会覆盖它们。\n\n点「确定」继续（结果将丢失），点「取消」返回并先点「保留当前结果」。`)) {
      return;
    }
  } else if (state.results.length) {
    if (!confirm(`当前有 ${state.results.length} 个未保留的结果，开始新一轮会覆盖它们（已保留的 ${state.saved.length} 批不受影响）。继续？`)) {
      return;
    }
  }
  try {
    await api('/api/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(cfg),
    });
    state.results = [];
    renderTable();
    setRunning(true);
    $('#statusText').textContent = '正在启动…';
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function stop() {
  try {
    await api('/api/cancel', { method: 'POST' });
    $('#statusText').textContent = '正在停止…';
  } catch (e) {
    toast(e.message, 'err');
  }
}

function setRunning(on) {
  state.running = on;
  const dot = $('#statusDot');
  dot.className = 'dot' + (on ? ' run' : '');
  $('#btnStart').classList.toggle('hidden', on);
  $('#btnStop').classList.toggle('hidden', !on);
  $('#statRunning').textContent = on ? '测速中' : '就绪';
  if (on) {
    $('#progressFill').className = 'indet';
    $('#progressFill').style.width = '';
  } else {
    $('#progressFill').className = '';
    $('#progressFill').style.width = '0%';
  }
  refreshKeepButton();
}

function setProgress(cur, total) {
  const fill = $('#progressFill');
  if (total > 0) {
    fill.className = '';
    fill.style.width = Math.min(100, Math.round((cur / total) * 100)) + '%';
  }
}

// ── SSE ────────────────────────────────────────
function connectEvents() {
  const es = new EventSource('/api/events');
  es.onmessage = (ev) => {
    let e;
    try { e = JSON.parse(ev.data); } catch (_) { return; }

    if (e.message) {
      $('#statusText').textContent = e.total > 0 ? `${e.message}  ${e.current}/${e.total}` : e.message;
    }
    if (e.type === 'log') {
      addLog(e.stage, e.message, e.at);
      if (e.stage !== 'result') return;
    }
    if (e.type === 'progress') {
      setRunning(true);
      setProgress(e.current, e.total);
    } else if (e.type === 'result' && e.result) {
      state.results.push(e.result);
      renderTable();
      setRunning(true);
    } else if (e.type === 'done') {
      if (e.results) state.results = e.results;
      renderTable();
      setProgress(1, 1);
      setRunning(false);
      $('#statusDot').className = 'dot done';
      addLog('done', e.message, e.at, 'ok');
      toast(`测速完成，${state.results.length} 个结果`, 'ok');
    } else if (e.type === 'error') {
      setRunning(false);
      $('#statusDot').className = 'dot err';
      addLog(e.stage, e.message, e.at, 'err');
      toast(e.message || '测速失败', 'err');
    }
  };
  es.onerror = () => { /* 浏览器自动重连 */ };
}

// ── 日志 ────────────────────────────────────────
function addLog(stage, msg, at, kind) {
  const el = document.createElement('div');
  el.className = 'logline' + (kind ? ' ' + kind : '');
  el.innerHTML = `<span class="t">${timeStr(at || Date.now())}</span><span class="s">${stage || ''}</span><span class="m"></span>`;
  el.querySelector('.m').textContent = msg;
  $('#logBody').appendChild(el);
  $('#logBody').scrollTop = $('#logBody').scrollHeight;
}

async function loadLogs() {
  try {
    const r = await api('/api/logs');
    (r.logs || []).forEach((l) => addLog(l.stage, l.message, l.at));
  } catch (_) {}
}

// ── 表格 ────────────────────────────────────────
function visibleRows() {
  const f = state.filter.trim().toLowerCase();
  let rows = state.results.slice();
  if (f) rows = rows.filter((r) => (r.ip || '').toLowerCase().includes(f) || (r.colo || '').toLowerCase().includes(f));
  rows.sort((a, b) => {
    const k = state.sortKey;
    let x = a[k], y = b[k];
    if (typeof x === 'string') return x.localeCompare(y) * state.sortDir;
    return ((x || 0) - (y || 0)) * state.sortDir;
  });
  return rows;
}

function renderTable() {
  const rows = visibleRows();
  const tb = $('#tbody');
  tb.innerHTML = '';
  $('#emptyBox').classList.toggle('hidden', rows.length > 0);
  const LIMIT = 1000;
  const shown = rows.length > LIMIT ? rows.slice(0, LIMIT) : rows;
  $('#statResult').textContent =
    rows.length > LIMIT ? `结果 ${rows.length}（表格仅显示前 ${LIMIT}）` : `结果 ${rows.length}`;

  shown.forEach((r, i) => {
    const tr = document.createElement('tr');
    const goodDelay = r.delay > 0 && r.delay < 180;
    const goodSpeed = r.speed >= 1;
    tr.innerHTML = `
      <td class="c-idx">${i + 1}</td>
      <td class="ip">${r.ip}</td>
      <td class="c-num">${r.sent}</td>
      <td class="c-num">${r.received}</td>
      <td class="c-num">${(r.loss * 100).toFixed(0)}%</td>
      <td class="c-num ${goodDelay ? 'delay-good' : ''}">${r.delay} ms</td>
      <td class="c-num ${goodSpeed ? 'speed-good' : ''}">${r.speed.toFixed(2)} MB/s</td>
      <td>${r.colo ? `<span class="tag">${r.colo}</span>` : '-'}</td>
      <td><button class="rowbtn" data-copy="${r.ip}:${r.port}#${r.remark}">复制</button></td>`;
    tb.appendChild(tr);
  });
  refreshKeepButton();
}

function copy(text) {
  navigator.clipboard.writeText(text).then(
    () => toast('已复制', 'ok'),
    () => toast('复制失败，请手动选择', 'err')
  );
}

// ── 保留当前结果 ────────────────────────────────
const SAVED_KEY = 'cfst-saved-batches';
const SAVED_LIMIT = 1000;

function loadSaved() {
  try {
    const raw = localStorage.getItem(SAVED_KEY);
    const arr = raw ? JSON.parse(raw) : [];
    return Array.isArray(arr) ? arr.filter((b) => b && Array.isArray(b.rows)) : [];
  } catch (_) {
    return [];
  }
}

function persistSaved() {
  try {
    localStorage.setItem(SAVED_KEY, JSON.stringify(state.saved));
  } catch (_) {
    toast('保留批次过大，未能写入本地存储（本次会话内仍有效）', 'err');
  }
}

/// 导出 / 复制 / 上报的统一数据源：所有保留批次 + 当前轮
function allRows() {
  const merged = [];
  state.saved.forEach((b) => merged.push(...b.rows));
  merged.push(...state.results);
  return merged;
}

function keepCurrent() {
  if (!state.results.length) return toast('当前没有可保留的结果', 'err');
  const rows = state.results.slice();
  state.saved.push({ at: Date.now(), rows });
  persistSaved();
  state.results = [];
  renderTable();
  renderSaved();
  toast(`已保留第 ${state.saved.length} 批（${rows.length} 个节点）`, 'ok');
}

function removeSaved(idx) {
  state.saved.splice(idx, 1);
  persistSaved();
  renderSaved();
  toast('已删除该批', 'ok');
}

function clearSaved() {
  if (!state.saved.length) return;
  if (!confirm(`确定清空全部 ${state.saved.length} 批保留结果？此操作不可撤销。`)) return;
  state.saved = [];
  persistSaved();
  renderSaved();
  toast('已清空全部保留结果', 'ok');
}

function batchTableHtml(rows) {
  const shown = rows.length > SAVED_LIMIT ? rows.slice(0, SAVED_LIMIT) : rows;
  const body = shown
    .map(
      (r, i) => `<tr>
      <td>${i + 1}</td>
      <td class="ip">${r.ip}</td>
      <td>${(r.loss * 100).toFixed(0)}%</td>
      <td>${r.delay} ms</td>
      <td>${r.speed.toFixed(2)} MB/s</td>
      <td>${r.colo || '-'}</td>
      <td><button class="rowbtn" data-copy="${r.ip}:${r.port}#${r.remark}">复制</button></td>
    </tr>`
    )
    .join('');
  const note =
    rows.length > SAVED_LIMIT
      ? `<div class="batch-more">仅显示前 ${SAVED_LIMIT} 行，共 ${rows.length} 行</div>`
      : '';
  return `<table>
    <thead><tr><th>#</th><th>IP</th><th>丢包</th><th>延迟</th><th>速度</th><th>地区</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>${note}`;
}

function renderSaved() {
  const box = $('#savedBox');
  const list = $('#savedList');
  $('#savedCount').textContent = state.saved.length;
  $('#savedTotal').textContent = state.saved.reduce((n, b) => n + b.rows.length, 0);
  box.classList.toggle('hidden', !state.saved.length);
  list.innerHTML = '';
  state.saved.forEach((b, i) => {
    const d = document.createElement('details');
    d.className = 'batch';

    const sum = document.createElement('summary');
    const label = document.createElement('span');
    label.textContent = `第 ${i + 1} 批`;
    const meta = document.createElement('span');
    meta.className = 'b-time';
    meta.textContent = `${timeStr(b.at)} · ${b.rows.length} 个节点`;
    const del = document.createElement('button');
    del.className = 'btn ghost sm batch-del';
    del.textContent = '删除';
    del.onclick = (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      removeSaved(i);
    };
    sum.append(label, meta, del);

    const body = document.createElement('div');
    body.className = 'batch-body';
    d.addEventListener('toggle', () => {
      if (d.open && !body.dataset.rendered) {
        body.innerHTML = batchTableHtml(b.rows);
        body.dataset.rendered = '1';
      }
    });

    d.append(sum, body);
    list.appendChild(d);
  });
  refreshKeepButton();
}

function refreshKeepButton() {
  $('#btnKeep').disabled = state.running || !state.results.length;
}

function toCsv(rows) {
  const head = 'IP 地址,已发送,已接收,丢包率,平均延迟,下载速度(MB/s),地区码';
  const lines = rows.map((r) =>
    [r.ip, r.sent, r.received, r.loss.toFixed(2), r.delay, r.speed.toFixed(2), r.colo].join(',')
  );
  return '\ufeff' + [head, ...lines].join('\n');
}

function downloadCsv(rows) {
  const blob = new Blob([toCsv(rows)], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `cfst-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.csv`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

// ── 上报配置与上报 ──────────────────────────────
async function loadReportConfig() {
  try {
    const c = await api('/api/report-config');
    $('#cfgDomain').value = c.worker_domain || '';
    $('#cfgUUID').value = c.uuid || '';
    $('#cfgRepo').value = c.github_repo || '';
    $('#cfgPath').value = c.github_path || 'cloudflare_ips.txt';
    state.hasToken = !!c.has_github_token;
    $('#tokenHint').textContent = state.hasToken ? '已保存' : '';
  } catch (_) {}
}

async function saveReportConfig() {
  const body = {
    worker_domain: $('#cfgDomain').value.trim(),
    uuid: $('#cfgUUID').value.trim(),
    github_repo: $('#cfgRepo').value.trim(),
    github_path: $('#cfgPath').value.trim(),
  };
  const tok = $('#cfgToken').value.trim();
  if (tok) body.github_token = tok;
  try {
    const r = await api('/api/report-config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    $('#cfgToken').value = '';
    state.hasToken = !!r.has_github_token;
    $('#tokenHint').textContent = state.hasToken ? '已保存' : '';
    $('#mask').classList.add('hidden');
    toast('上报配置已保存', 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function uploadWorker() {
  const domain = $('#cfgDomain').value.trim();
  const uuid = $('#cfgUUID').value.trim();
  if (!domain || !uuid) {
    $('#mask').classList.remove('hidden');
    toast('请先填写 Worker 域名和 UUID', 'err');
    return;
  }
  const rows = allRows();
  if (!rows.length) {
    toast('没有可上报的节点', 'err');
    return;
  }
  try {
    const r = await api('/api/upload/worker', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        worker_domain: domain,
        uuid: uuid,
        limit: +$('#cfgLimit').value || 0,
        clear: $('#cfgClear').checked,
        rows: rows,
      }),
    });
    toast(`已上报 ${r.count} 个 IP`, 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function uploadGitHub() {
  const repo = $('#cfgRepo').value.trim();
  if (!repo || (!state.hasToken && !$('#cfgToken').value.trim())) {
    $('#mask').classList.remove('hidden');
    toast('请先填写 GitHub 仓库和 Token', 'err');
    return;
  }
  const rows = allRows();
  if (!rows.length) {
    toast('没有可上报的节点', 'err');
    return;
  }
  try {
    const r = await api('/api/upload/github', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        repo: repo,
        token: $('#cfgToken').value.trim(),
        path: $('#cfgPath').value.trim(),
        limit: +$('#cfgLimit').value || 0,
        rows: rows,
      }),
    });
    toast(`已上传 ${r.count} 个 IP 到 GitHub`, 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

// ── 初始化 ──────────────────────────────────────
(async function init() {
  setupSidebarResize();

  state.saved = loadSaved();
  renderSaved();

  try {
    const r = await api('/api/config');
    fillForm(r.config);
  } catch (_) {}

  await loadReportConfig();

  try {
    const st = await api('/api/status');
    if (st.running) setRunning(true);
    if (st.count) {
      state.results = (await api('/api/results')).results || [];
      renderTable();
    }
    await loadLogs();
  } catch (_) {}

  connectEvents();

  // 分段按钮
  $$('.seg button').forEach((b) => {
    b.addEventListener('click', () => {
      if (!b.parentElement.id) return;
      const parent = b.parentElement;
      Array.from(parent.children).forEach((x) => x.classList.toggle('on', x === b));
      refreshDerived();
    });
  });

  $$('#panel input, .panel input, .panel textarea').forEach((el) => {
    el.addEventListener('input', refreshDerived);
    el.addEventListener('change', refreshDerived);
  });

  $('#btnPickIpFile').addEventListener('click', () => $('#inIpFile').click());

  $('#inIpFile').addEventListener('change', (event) => {
    const file = event.target.files[0];
    if (file) loadIpFile(file);
  });

  $('#btnClearIpText').addEventListener('click', () => {
    $('#inIpText').value = '';
    $('#ipFileName').textContent = '按照换行符分隔多个 IP 段；也可点右上角「选择文件」读入本地 txt';
    refreshDerived();
  });

  $('#btnStart').onclick = start;
  $('#btnStop').onclick = stop;
  $('#btnStart').addEventListener('click', () => {}, { once: true });

  $('#btnClearLog').onclick = () => ($('#logBody').innerHTML = '');
  $('#btnCopyAll').onclick = () => {
    const rows = allRows();
    if (!rows.length) return toast('没有可复制的节点', 'err');
    copy(rows.map((r) => `${r.ip}:${r.port}#${r.remark}`).join('\n'));
  };
  $('#btnExport').onclick = () => {
    const rows = allRows();
    if (!rows.length) return toast('没有可导出的节点', 'err');
    downloadCsv(rows);
    toast(`已导出 ${rows.length} 个节点`, 'ok');
  };
  $('#btnKeep').onclick = keepCurrent;
  $('#btnClearSaved').onclick = clearSaved;
  $('#savedList').addEventListener('click', (ev) => {
    const btn = ev.target.closest('.rowbtn');
    if (btn) copy(btn.dataset.copy);
  });

  $('#btnReportCfg').onclick = () => $('#mask').classList.remove('hidden');
  $('#btnCfgClose').onclick = () => $('#mask').classList.add('hidden');
  $('#btnCfgSave').onclick = saveReportConfig;
  $('#mask').onclick = (e) => {
    if (e.target === $('#mask')) $('#mask').classList.add('hidden');
  };
  $('#btnUploadWorker').onclick = uploadWorker;
  $('#btnUploadGH').onclick = uploadGitHub;

  $('#filterText').addEventListener('input', (e) => {
    state.filter = e.target.value;
    renderTable();
  });

  $$('#tbl thead th[data-sort]').forEach((th) => {
    th.addEventListener('click', () => {
      const k = th.dataset.sort;
      if (state.sortKey === k) state.sortDir *= -1;
      else {
        state.sortKey = k;
        state.sortDir = typeof state.results[0]?.[k] === 'string' ? 1 : -1;
      }
      renderTable();
    });
  });

  $('#tbody').addEventListener('click', (ev) => {
    const btn = ev.target.closest('.rowbtn');
    if (btn) copy(btn.dataset.copy);
  });

  refreshDerived();
})();
