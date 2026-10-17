// Your games: the games you finished against Kairos on the Play page and the analyses you saved
// on the analysis board, kept in this browser (history.js). Each opens on the analysis board;
// its name and players can be edited, and all of them exported as JSONL records.
import { $, el, toast, confirmDialog } from './util.js';
import { listGames, deleteGame, clearGames, recordLine, updateGame } from './history.js';

export class Games {
  constructor(app) {
    this.app = app;
    this.games = [];
  }

  init() {
    for (const id of ['f-result', 'f-variant', 'f-kind']) $('#' + id).onchange = () => this.render();
    $('#games-body').addEventListener('click', (e) => {
      const act = e.target.closest('button[data-act]');
      const tr = e.target.closest('tr[data-id]');
      if (!tr) return;
      const id = +tr.dataset.id;
      if (act && act.dataset.act === 'delete') return this.remove(id);
      if (act && act.dataset.act === 'edit') return this.edit(id);
      location.hash = `#/analysis?game=${id}`;
    });
    $('#games-export').onclick = () => this.exportAll();
    $('#games-clear').onclick = async () => {
      if (!this.games.length) return;
      if (await confirmDialog('Delete every game?', { text: `All ${this.games.length} games kept in this browser are deleted.`, ok: 'Delete all', danger: true })) {
        await clearGames();
        this.load();
      }
    };
    $('#edit-save').onclick = (e) => { e.preventDefault(); this.saveEdit(); };
  }

  activate() { this.load(); }

  async load() {
    try {
      this.games = await listGames();
    } catch (e) {
      this.games = [];
      toast('Could not read the saved games: ' + e.message, { error: true });
    }
    const variants = [...new Set(this.games.map((g) => g.variant).filter(Boolean))].sort();
    const sel = $('#f-variant'), cur = sel.value;
    sel.replaceChildren(el('option', { value: '', text: 'All variants' }), ...variants.map((v) => el('option', { value: v, text: v })));
    sel.value = variants.includes(cur) ? cur : '';
    this.render();
  }

  /** The outcome for you: won/lost/drawn in your games, else the result. */
  static outcome(g) {
    if (g.kind !== 'analysis' && g.human && g.result) {
      const mine = g.human === 'white' ? '1-0' : '0-1';
      if (g.result === mine) return { key: 'win', text: 'Won' };
      if (g.result === '1/2-1/2') return { key: 'draw', text: 'Drawn' };
      return { key: 'loss', text: 'Lost' };
    }
    return { key: g.result === '1/2-1/2' ? 'draw' : '', text: g.result ? g.result.replace('1/2-1/2', '½–½') : '*' };
  }

  static title(g) {
    return g.name || `${g.white || 'White'} vs ${g.black || 'Black'}`;
  }

  render() {
    const result = $('#f-result').value, variant = $('#f-variant').value, kind = $('#f-kind').value;
    const shown = this.games.filter((g) => (!variant || g.variant === variant)
      && (!kind || (g.kind || 'played') === kind)
      && (!result || Games.outcome(g).key === result));
    $('#browser-title').textContent = this.games.length ? `Your games · ${shown.length} of ${this.games.length}` : 'Your games';
    const fmtDate = (t) => new Date(t).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
    const rows = shown.map((g) => {
      const o = Games.outcome(g);
      const saved = g.kind === 'analysis';
      return el('tr', { dataset: { id: g.id }, title: 'Open on the analysis board' },
        el('td', { class: 'date-cell', text: fmtDate(g.ended || g.started), title: saved ? (g.saved ? `Last saved ${fmtDate(g.saved)}` : '')
          : [g.started && g.ended ? `Started ${fmtDate(g.started)} · ended ${fmtDate(g.ended)}` : '', g.saved ? `analysis saved ${fmtDate(g.saved)}` : ''].filter(Boolean).join(' · ') }),
        el('td', { class: 'name-cell' }, el('span', { class: 'kind ' + (saved ? 'saved' : 'played'), text: saved ? 'Analysis' : 'Played' }), Games.title(g),
          g.notes ? el('span', { class: 'notes', text: g.notes, title: g.notes }) : null),
        el('td', { class: 'variant-cell', text: g.variant || '' }),
        el('td', { class: 'res ' + o.key, text: o.text, title: g.result || '' }),
        el('td', { class: 'reason', text: g.reason || '', title: g.reason || '' }),
        el('td', { class: 'num', text: g.moves ? g.moves.length : '', dataset: { unit: g.moves && g.moves.length === 1 ? ' action' : ' actions' } }),
        el('td', { class: 'tc-cell', text: g.timeControl || '' }),
        el('td', { class: 'engine-cell', text: g.engine ? g.engine.version : '', title: g.engine ? engineTitle(g.engine) : '' }),
        el('td', { class: 'del' },
          el('button', { type: 'button', dataset: { act: 'edit' }, title: 'Edit name and players', 'aria-label': 'Edit', text: '✎' }),
          el('button', { type: 'button', dataset: { act: 'delete' }, title: 'Delete this game', 'aria-label': 'Delete', text: '×' })));
    });
    $('#games-body').replaceChildren(...(rows.length ? rows : [el('tr', {}, el('td', { colspan: 9, class: 'empty-state',
      text: this.games.length ? 'No games match these filters.' : 'No games yet. Finish a game against Kairos on the Play page, or save one from the analysis board.' }))]));
  }

  async remove(id) {
    const g = this.games.find((x) => x.id === id);
    if (!g || !(await confirmDialog('Delete this game?', { text: Games.title(g), ok: 'Delete', danger: true }))) return;
    await deleteGame(id);
    this.load();
  }

  /** The edit dialog: name, players and notes. */
  edit(id) {
    const g = this.games.find((x) => x.id === id);
    if (!g) return;
    this.editing = id;
    $('#edit-name').value = g.name || '';
    $('#edit-name').placeholder = `${g.white || 'White'} vs ${g.black || 'Black'}`;
    $('#edit-white').value = g.white || '';
    $('#edit-black').value = g.black || '';
    $('#edit-notes').value = g.notes || '';
    $('#dlg-edit').showModal();
  }

  async saveEdit() {
    const val = (id) => $(id).value.trim();
    await updateGame(this.editing, { name: val('#edit-name'), white: val('#edit-white') || 'White', black: val('#edit-black') || 'Black', notes: val('#edit-notes') });
    $('#dlg-edit').close();
    this.load();
  }

  exportAll() {
    if (!this.games.length) return toast('No games to export');
    const blob = new Blob([this.games.map(recordLine).join('\n') + '\n'], { type: 'application/x-ndjson' });
    const a = el('a', { href: URL.createObjectURL(blob), download: `kairos-games-${new Date().toISOString().slice(0, 10)}.jsonl` });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  }
}

function engineTitle(e) {
  const parts = [e.version];
  if (e.where === 'browser') parts.push(`in the browser, ${e.threads} thread${e.threads === 1 ? '' : 's'}, ${e.hashMb} MB hash`);
  if (e.where === 'server') parts.push('on the kairos-web server');
  return parts.join(' · ');
}
