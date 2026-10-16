// Game browser: record files served by kairos-web, filtered and paged, opening in the viewer.
import { $, el, fmtBytes, debounce, store } from './util.js';

const PAGE = 100;

export class Games {
  constructor(app) {
    this.app = app;
    this.files = [];
    this.file = null;
    this.offset = 0;
    this.seq = 0;
  }

  init() {
    for (const id of ['f-variant', 'f-result', 'f-reason', 'f-sort']) $('#' + id).onchange = () => { this.offset = 0; this.list(); };
    const reload = debounce(() => { this.offset = 0; this.list(); }, 300);
    $('#f-min').oninput = reload;
    $('#f-max').oninput = reload;
    $('#file-filter').oninput = () => this.renderFiles();
    $('#games-body').addEventListener('click', (e) => {
      const tr = e.target.closest('tr[data-index]');
      if (tr) this.open(+tr.dataset.index);
    });
  }

  async activate(params) {
    if (!this.app.server) {
      $('#file-list').replaceChildren(el('div', { class: 'empty-state', text: 'The game browser needs the Kairos server (kairos-web).' }));
      return;
    }
    if (!this.loaded) {
      this.loaded = true;
      try {
        const r = await fetch('api/games/files').then((r) => r.json());
        this.files = r.files || [];
      } catch (e) { this.files = []; }
      this.renderFiles();
      const want = params.get('file') || store('games.file');
      const f = this.files.find((x) => x.id === want) || this.files.find((x) => x.name.endsWith('.jsonl'));
      if (f) this.select(f.id);
      else $('#games-body').replaceChildren(el('tr', {}, el('td', { colspan: 8, class: 'empty-state', text: 'No record files found. Start kairos-web with --games <dir> (default: games/ in the repository).' })));
    }
  }

  renderFiles() {
    const q = $('#file-filter').value.trim().toLowerCase();
    const groups = new Map();
    for (const f of this.files) {
      if (q && !f.id.toLowerCase().includes(q)) continue;
      const g = f.dir || 'games';
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g).push(f);
    }
    const out = [];
    for (const [g, fs] of groups) {
      out.push(el('div', { class: 'file-group', text: g }));
      fs.sort((a, b) => b.mtime - a.mtime);
      for (const f of fs) {
        out.push(el('div', { class: 'file' + (f.id === this.file ? ' on' : ''), title: f.id, onclick: () => this.select(f.id) },
          el('span', { class: 'name', text: f.name.replace(/\.jsonl$/, '') }), el('span', { class: 'size', text: fmtBytes(f.size) })));
      }
    }
    $('#file-list').replaceChildren(...(out.length ? out : [el('div', { class: 'empty-state', text: 'No files' })]));
  }

  select(id) {
    this.file = id;
    store('games.file', id);
    this.offset = 0;
    for (const s of ['f-variant', 'f-reason']) $('#' + s).value = '';
    this.facetsFor = null;
    this.renderFiles();
    const f = this.files.find((x) => x.id === id);
    if (f && f.name.endsWith('.5dpgn')) {
      location.hash = `#/analysis?file=${encodeURIComponent(id)}&index=0`;
      return;
    }
    this.list();
  }

  async list() {
    if (!this.file) return;
    const p = new URLSearchParams({ file: this.file, offset: this.offset, limit: PAGE });
    const add = (k, id) => { const v = $('#' + id).value.trim(); if (v) p.set(k, v); };
    add('variant', 'f-variant'); add('result', 'f-result'); add('reason', 'f-reason'); add('min', 'f-min'); add('max', 'f-max'); add('sort', 'f-sort');
    $('#browser-title').textContent = 'Loading…';
    const seq = ++this.seq;
    let r;
    try {
      r = await fetch('api/games/list?' + p).then((r) => r.json());
    } catch (e) { r = { ok: false, error: e.message }; }
    if (seq !== this.seq) return;
    const f = this.files.find((x) => x.id === this.file);
    if (!r.ok) {
      $('#browser-title').textContent = f ? f.name : 'Games';
      $('#games-body').replaceChildren(el('tr', {}, el('td', { colspan: 8, class: 'empty-state', text: r.error || 'Failed' })));
      return;
    }
    $('#browser-title').textContent = `${f ? f.name : 'Games'} · ${r.total.toLocaleString()} of ${r.count.toLocaleString()} games`;
    if (this.facetsFor !== this.file) {
      this.facetsFor = this.file;
      const fill = (id, label, items) => {
        const sel = $('#' + id);
        const cur = sel.value;
        sel.replaceChildren(el('option', { value: '', text: label }), ...items.map(([k, n]) => el('option', { value: k, text: `${k || '(none)'} (${n})` })));
        sel.value = cur;
      };
      fill('f-variant', 'All variants', r.variants);
      fill('f-reason', 'All endings', r.reasons.filter(([k]) => k));
    }
    const res = (g) => (g.result || '?').replace('1/2-1/2', '½–½');
    const names = (g) => g.newFirst == null ? ['White', 'Black'] : g.newFirst ? ['New', 'Base'] : ['Base', 'New'];
    $('#games-body').replaceChildren(...r.games.map((g) => {
      const [w, b] = names(g);
      return el('tr', { dataset: { index: g.index } },
        el('td', { class: 'num', text: g.game ?? g.index }),
        el('td', { text: g.variant }),
        el('td', { text: w }),
        el('td', { class: 'res', text: res(g) }),
        el('td', { text: b }),
        el('td', { class: 'reason', text: g.reason || '', title: g.reason || '' }),
        el('td', { class: 'num', text: g.actions }),
        el('td', { class: 'num', text: g.book || '' }));
    }));
    if (!r.games.length) $('#games-body').replaceChildren(el('tr', {}, el('td', { colspan: 8, class: 'empty-state', text: 'No games match these filters.' })));
    const pager = $('#pager');
    const from = r.total ? this.offset + 1 : 0, to = Math.min(r.total, this.offset + PAGE);
    pager.replaceChildren(
      el('span', { text: `${from}–${to} of ${r.total.toLocaleString()}` }),
      el('div', { class: 'row gap' },
        el('button', { class: 'btn sm', disabled: this.offset === 0, text: 'Previous', onclick: () => { this.offset = Math.max(0, this.offset - PAGE); this.list(); } }),
        el('button', { class: 'btn sm', disabled: to >= r.total, text: 'Next', onclick: () => { this.offset += PAGE; this.list(); } })));
  }

  open(index) {
    location.hash = `#/analysis?file=${encodeURIComponent(this.file)}&index=${index}`;
  }
}
