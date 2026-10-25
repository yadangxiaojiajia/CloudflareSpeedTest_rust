// 前端「保留当前结果」逻辑冒烟：真 DOM + 真事件，stub 掉 fetch / EventSource / 剪贴板
const fs = require('fs');
const path = require('path');
const { JSDOM } = require('jsdom');

const root = process.argv[2] || process.cwd();
const html = fs
  .readFileSync(path.join(root, 'frontend/index.html'), 'utf8')
  .replace('<script src="/app.js"></script>', '');
const appJs = fs.readFileSync(path.join(root, 'frontend/app.js'), 'utf8');

const dom = new JSDOM(html, {
  url: 'http://127.0.0.1/',
  runScripts: 'dangerously',
  pretendToBeVisual: true,
});
const { window } = dom;
const doc = window.document;

// ---- stubs ----
let lastUpload = null;
const json = (o) => ({ ok: true, status: 200, statusText: 'OK', json: async () => o });

window.matchMedia = () => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} });
window.confirm = () => true;
window.alert = () => {};
window.EventSource = class {
  constructor() { this.onmessage = null; this.onerror = null; }
  close() {}
};
const copied = [];
Object.defineProperty(window.navigator, 'clipboard', {
  value: { writeText: async (t) => { copied.push(t); } },
  configurable: true,
});
let csvParts = null;
window.Blob = class { constructor(parts, opts) { csvParts = parts; this.opts = opts; } };
window.URL.createObjectURL = () => 'blob:fake';
window.URL.revokeObjectURL = () => {};
const anchorClick = [];
const origCreate = doc.createElement.bind(doc);
doc.createElement = (tag) => {
  const el = origCreate(tag);
  if (tag === 'a') el.click = () => anchorClick.push(el.download);
  return el;
};

window.fetch = async (url, opts) => {
  const u = String(url);
  if (u === '/api/config') {
    return json({
      config: {
        routines: 200, ping_times: 4, download_num: 10, download_time: 10, port: 443,
        url: 'https://speed.cloudflare.com/__down?bytes=99000000', httping: false,
        httping_code: 0, cfcolo: '', max_delay: 9999, min_delay: 0, max_loss_rate: 1,
        min_speed: 0, print_num: 10, ip_file: 'ip.txt', ip_text: '', output: 'result.csv',
        disable_download: false, debug: false,
      },
    });
  }
  if (u === '/api/report-config') {
    return json({ worker_domain: '', uuid: '', github_repo: '', github_path: 'cloudflare_ips.txt', has_github_token: true });
  }
  if (u === '/api/status') return json({ running: false, count: 0 });
  if (u === '/api/logs') return json({ logs: [] });
  if (u === '/api/results') return json({ results: [] });
  if (u.startsWith('/api/upload/')) {
    lastUpload = { url: u, body: JSON.parse(opts.body) };
    return json({ ok: true, count: lastUpload.body.rows.length });
  }
  return json({ ok: true });
};

// ---- 执行 app.js（真实 script，顶层 const 成为全局词法绑定）----
const s = doc.createElement('script');
s.textContent = appJs;
doc.body.appendChild(s);

const results = [];
function check(name, cond, extra) {
  results.push({ name, ok: !!cond, extra });
}

const ROUND1 = [
  { ip: '1.1.1.1', sent: 4, received: 4, loss: 0, delay: 20, speed: 5.5, colo: 'HKG', port: 443, remark: '香港-5.50MB/s' },
  { ip: '2.2.2.2', sent: 4, received: 4, loss: 0, delay: 40, speed: 2.5, colo: 'NRT', port: 2053, remark: '东京-2.50MB/s' },
];
const ROUND2 = [
  { ip: '3.3.3.3', sent: 4, received: 4, loss: 0, delay: 30, speed: 8.8, colo: 'SJC', port: 443, remark: '圣何塞-8.80MB/s' },
];

const $ = (sel) => doc.querySelector(sel);

(async () => {
  await new Promise((r) => setTimeout(r, 60)); // 等 init() 走完

  // 1. 初始：无结果时按钮禁用、已保留区隐藏
  check('初始 keep 按钮禁用', $('#btnKeep').disabled === true);
  check('初始已保留区隐藏', $('#savedBox').classList.contains('hidden'));

  // 1b. --dn 允许 0（不设上限），不能被 `|| 10` 吃掉
  $('#inDnNum').value = '0';
  check('dn 填 0 → download_num=0', window.eval('collect().download_num') === 0);
  check('dn 填 0 → 命令行带 --dn 0', window.eval('toCliArgs(collect())').includes('--dn 0'));
  $('#inDnNum').value = '';
  check('dn 留空 → 回落 10', window.eval('collect().download_num') === 10);
  $('#inDnNum').value = '7';
  check('dn 填 7 → 7', window.eval('collect().download_num') === 7);
  $('#inDnNum').value = '10';

  // 2. 出结果后按钮可用
  window.eval(`state.results = ${JSON.stringify(ROUND1)}; renderTable();`);
  check('有结果后 keep 按钮可用', $('#btnKeep').disabled === false);
  check('主表渲染 2 行', doc.querySelectorAll('#tbody tr').length === 2);

  // 3. 点「保留当前结果」
  $('#btnKeep').click();
  const afterKeep = window.eval('({saved: state.saved.length, cur: state.results.length})');
  check('保留后批次=1', afterKeep.saved === 1, afterKeep);
  check('保留后当前轮清空', afterKeep.cur === 0);
  check('主表被清空', doc.querySelectorAll('#tbody tr').length === 0);
  check('已保留区显示', !$('#savedBox').classList.contains('hidden'));
  check('批次数文案=1', $('#savedCount').textContent === '1');
  check('节点总数文案=2', $('#savedTotal').textContent === '2');
  check('keep 按钮再次禁用', $('#btnKeep').disabled === true);

  // 4. 折叠条懒渲染：展开后才生成表格
  const det = doc.querySelector('#savedList details');
  check('折叠条默认未展开', det.open === false);
  check('展开前无表格', doc.querySelectorAll('#savedList .batch-body table').length === 0);
  det.open = true;
  det.dispatchEvent(new window.Event('toggle'));
  check('展开后渲染表格 2 行', doc.querySelectorAll('#savedList .batch-body tbody tr').length === 2);

  // 5. 第二轮：再来一次，两批并存
  window.eval(`state.results = ${JSON.stringify(ROUND2)}; renderTable();`);
  $('#btnKeep').click();
  check('两批并存', window.eval('state.saved.length') === 2);
  check('节点总数=3', $('#savedTotal').textContent === '3');
  check('折叠条数量=2', doc.querySelectorAll('#savedList details').length === 2);

  // 6. localStorage 持久化
  const stored = JSON.parse(window.localStorage.getItem('cfst-saved-batches'));
  check('写入 localStorage 2 批', Array.isArray(stored) && stored.length === 2, stored && stored.length);

  // 7. 复制节点 = 全部保留批次（2+1）
  window.eval(`state.results = ${JSON.stringify(ROUND1)}; renderTable();`);
  $('#btnCopyAll').click();
  await new Promise((r) => setTimeout(r, 20));
  const copyLines = (copied[copied.length - 1] || '').split('\n').filter(Boolean);
  check('复制节点含 2 批 + 当前轮 = 5 行', copyLines.length === 5, copyLines);
  check('复制格式含 IP:端口#备注', copyLines[0] === '1.1.1.1:443#香港-5.50MB/s', copyLines[0]);

  // 8. 导出 CSV = 全部保留批次 + 当前轮
  $('#btnExport').click();
  const csv = String((csvParts && csvParts[0]) || '');
  const csvLines = csv.replace(/^\ufeff/, '').split('\n').filter(Boolean);
  check('导出 CSV 含表头 + 5 行', csvLines.length === 6, csvLines.length);
  check('导出 CSV 带 BOM', csv.startsWith('\ufeff'));
  check('导出触发下载', anchorClick.length === 1 && anchorClick[0].endsWith('.csv'), anchorClick);

  // 9. 上报 Worker 带 rows（全部批次）
  $('#cfgDomain').value = 'demo.workers.dev';
  $('#cfgUUID').value = 'uuid-1';
  window.eval('uploadWorker()');
  await new Promise((r) => setTimeout(r, 30));
  check('上报请求带 rows', !!lastUpload && Array.isArray(lastUpload.body.rows), lastUpload && lastUpload.url);
  check('上报 rows = 5 条', lastUpload && lastUpload.body.rows.length === 5, lastUpload && lastUpload.body.rows.length);

  // 10. 上报 GitHub 同样带 rows
  lastUpload = null;
  $('#cfgRepo').value = 'owner/repo';
  window.eval('uploadGitHub()');
  await new Promise((r) => setTimeout(r, 30));
  check('GitHub 上报带 rows=5', lastUpload && lastUpload.body.rows.length === 5, lastUpload && lastUpload.url);

  // 11. 删除单批
  const delBtn = doc.querySelector('#savedList details .batch-del');
  delBtn.click();
  check('删除后剩 1 批', window.eval('state.saved.length') === 1);

  // 12. 清空全部
  $('#btnClearSaved').click();
  check('清空后 0 批', window.eval('state.saved.length') === 0);
  check('清空后已保留区隐藏', $('#savedBox').classList.contains('hidden'));

  // ---- 汇总 ----
  const fail = results.filter((r) => !r.ok);
  results.forEach((r) => console.log(`${r.ok ? 'PASS' : 'FAIL'}  ${r.name}${r.ok ? '' : '  → ' + JSON.stringify(r.extra)}`));
  console.log(`\n${results.length - fail.length}/${results.length} 通过`);
  process.exit(fail.length ? 1 : 0);
})().catch((e) => {
  console.error('运行异常:', e);
  process.exit(2);
});
