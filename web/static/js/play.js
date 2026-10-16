// Play against Kairos. The server runs the engine and keeps the clocks; one human plays at a
// time and everyone else watches the live game (and may queue for the next one).
import { Rules } from './kairos.js';
import { lanText } from './input.js';
import { $, $$, el, toast, fmtClock, fmtScore, fmtNodes, store, confirmDialog } from './util.js';

export class Play {
  constructor(app) {
    this.app = app;
    this.board = app.board;
    this.input = app.input;
    this.token = store('play.token') || null;
    this.state = null;
    this.localMoves = [];
    this.localStart = null;
    this.cur = 0;
    this.color = store('play.color') || 'w';
    this.source = {
      rules: null,
      node: () => this.cur,
      canMove: () => this.myTurn(),
      blocked: () => this.blockedHint(),
      submit: (moves) => this.submit(moves),
      onView: (v) => this.onView(v),
    };
  }

  async init() {
    this.rules = await Rules.create();
    this.source.rules = this.rules;
    const variants = this.rules.call({ cmd: 'variants' }).variants;
    const saved = store('play.variant') || 'Standard';
    $('#play-variant').replaceChildren(...variants.map((v) => el('option', { value: v.name, text: v.name, selected: v.name === saved })));
    $$('#play-color button').forEach((b) => {
      b.classList.toggle('on', b.dataset.v === this.color);
      b.onclick = () => {
        this.color = b.dataset.v;
        store('play.color', this.color);
        $$('#play-color button').forEach((x) => x.classList.toggle('on', x === b));
        this.preview();
      };
    });
    const tcSaved = store('play.tc');
    $$('#play-tc button').forEach((b) => {
      if (tcSaved) b.classList.toggle('on', b.textContent === tcSaved);
      b.onclick = () => { $$('#play-tc button').forEach((x) => x.classList.toggle('on', x === b)); store('play.tc', b.textContent); };
    });
    $('#play-form').onsubmit = (e) => { e.preventDefault(); this.start(); };
    $('#play-variant').onchange = () => { store('play.variant', $('#play-variant').value); this.preview(); };
    $('#play-resign').onclick = async () => {
      if (await confirmDialog('Resign this game?', { text: 'Kairos wins the game.', ok: 'Resign', danger: true })) this.post('resign').catch(() => {});
    };
    $('#play-new').onclick = () => this.showSetup(true);
    $('#play-analyze').onclick = () => this.analyze();
    $('#play-queue').onclick = () => this.queue();
    $('#play-unqueue').onclick = () => this.post('leave');
    $('#play-moves').addEventListener('click', (e) => {
      const c = e.target.closest('[data-id]');
      if (c) { this.cur = +c.dataset.id; this.input.reset(); this.show(); }
    });
    this.preview();
    setInterval(() => this.tickClocks(), 100);
  }

  /** No game in progress is on the board (none, a finished one, or you are setting up the next). */
  previewable(s = this.state) {
    const g = s && s.game;
    return !g || !!g.over || !!(s.you && this.wantSetup);
  }

  /** Show the chosen variant's start position (from the chosen side) while setting up a game. */
  preview() {
    if (!this.rules || !this.previewable()) return;
    try {
      const r = this.rules.call({ cmd: 'new', position: $('#play-variant').value || 'Standard' });
      this.tree = r.tree;
    } catch (e) {
      toast(e.message, { error: true });
      return;
    }
    this.previewing = true;
    this.cur = 0;
    this.localStart = null;
    this.localMoves = [];
    this.input.reset();
    this.board.setFlipped(this.color === 'b');
    $('#tool-flip').classList.toggle('on', this.board.flipped);
    this.board.fitted = false;
    this.show();
  }

  activate() {
    this.active = true;
    this.input.attach(this.source);
    if (!this.app.server) {
      $('#play-setup').replaceChildren(el('h2', { class: 'card-title', text: 'Play Kairos' }), el('p', { class: 'muted', text: 'Playing needs the Kairos server. Run kairos-web (see the README) and open the site from it.' }));
      this.show();
      return;
    }
    this.connect();
    if (this.previewable()) this.preview();
    else this.show();
  }

  deactivate() {
    this.active = false;
  }

  connect() {
    if (this.es) return;
    const url = 'api/play/events' + (this.token ? `?token=${encodeURIComponent(this.token)}` : '');
    this.es = new EventSource(url);
    this.es.addEventListener('state', (e) => this.onState(JSON.parse(e.data)));
    this.es.addEventListener('thinking', (e) => this.onThinking(JSON.parse(e.data)));
    this.es.onerror = () => {
      // EventSource reconnects by itself; refresh the state when it does.
      $('#play-status').textContent = 'Reconnecting…';
    };
  }
  reconnect() {
    if (this.es) this.es.close();
    this.es = null;
    this.connect();
  }

  async post(what, body = {}) {
    try {
      const r = await fetch(`api/play/${what}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ token: this.token, ...body }) });
      const j = await r.json();
      if (!j.ok) throw new Error(j.error === 'busy' ? 'Kairos is busy with another player' : j.error);
      if (j.token && j.token !== this.token) {
        this.token = j.token;
        store('play.token', this.token);
        this.reconnect();
      }
      this.onState(j);
      return j;
    } catch (e) {
      toast(e.message, { error: true });
      throw e;
    }
  }

  start() {
    const tc = $('#play-tc button.on') || $('#play-tc button');
    const base = +tc.dataset.base, inc = +tc.dataset.inc;
    const variant = $('#play-variant').value;
    store('play.variant', variant);
    const body = {
      variant,
      color: this.color === 'r' ? 'random' : this.color,
      clock: base ? { base: base * 1000, inc: inc * 1000 } : null,
      movetime: +(tc.dataset.movetime || 2000),
    };
    this.wantSetup = false;
    this.post('start', body).then(() => {
      const g = this.state && this.state.game;
      if (g) {
        this.board.setFlipped(g.humanColor === 'b');
        $('#tool-flip').classList.toggle('on', this.board.flipped);
        this.board.fitted = false;
      }
    }).catch(() => {});
  }

  queue() { this.post('queue').catch(() => {}); }

  async submit(moves) {
    const lan = moves.map(lanText).join(' ');
    // Apply locally at once; the server's state event confirms it.
    const r = this.rules.call({ cmd: 'play', node: this.cur, moves });
    this.cur = r.node;
    this.tree = r.tree;
    this.localMoves.push(this.tree.nodes[r.node].lan);
    this.show();
    try {
      await this.post('move', { lan });
    } catch (e) {
      this.localStart = null; // resync from the server
      if (this.state) this.onState(this.state);
    }
  }

  myTurn() {
    const s = this.state;
    if (!s || !s.you || !s.game || s.game.over) return false;
    const atEnd = this.cur === this.lastNode();
    return atEnd && s.game.toMove === s.game.humanColor && this.localMoves.length === s.game.moves.length;
  }
  blockedHint() {
    const s = this.state;
    if (!s || !s.game) toast('Start a game first');
    else if (!s.you) toast('You are watching this game');
    else if (s.game.over) toast('The game is over');
    else if (this.cur !== this.lastNode()) toast('Go to the latest position to move');
    else if (s.game.toMove !== s.game.humanColor) toast('Kairos is thinking…');
  }

  lastNode() {
    if (!this.tree) return 0;
    let c = 0;
    while (this.tree.nodes[c].children.length) c = this.tree.nodes[c].children[this.tree.nodes[c].children.length - 1];
    return c;
  }

  /** Mirror the server game in the local rules instance. */
  sync(game) {
    const prefix = this.localStart === game.start && game.moves.length >= this.localMoves.length && this.localMoves.every((m, i) => m === game.moves[i]);
    if (!prefix) {
      const r = this.rules.call({ cmd: 'new', position: game.start });
      this.tree = r.tree;
      this.localStart = game.start;
      this.localMoves = [];
      this.cur = 0;
      this.board.fitted = false;
    }
    const atEnd = this.cur === this.lastNode();
    let node = this.lastNode();
    for (let i = this.localMoves.length; i < game.moves.length; i++) {
      try {
        const r = this.rules.call({ cmd: 'play', node, lan: game.moves[i] });
        this.tree = r.tree;
        node = r.node;
        this.localMoves.push(game.moves[i]);
      } catch (e) {
        toast('Could not replay the server game: ' + e.message, { error: true });
        break;
      }
    }
    if (atEnd || !prefix) {
      if (this.cur !== node) this.input.reset();
      this.cur = node;
    }
  }

  onState(s) {
    this.state = s;
    this.stateAt = performance.now();
    // A setup preview stays on the board until a game is in progress.
    if (this.previewing && this.previewable(s)) {
      if (this.active) this.show();
      return;
    }
    this.previewing = false;
    if (s.game) this.sync(s.game);
    if (s.you) this.wantSetup = this.wantSetup && !!(s.game && s.game.over);
    if (this.active) this.show();
  }

  onThinking(info) {
    this.thinking = info;
    if (!this.active) return;
    const g = this.state && this.state.game;
    if (!g || !g.thinking) return;
    const engineBlack = g.humanColor === 'w';
    const score = info.cp != null ? { cp: engineBlack ? -info.cp : info.cp } : info.mate != null ? { mate: engineBlack ? -info.mate : info.mate } : null;
    $('#play-thinking').textContent = `Depth ${info.depth ?? '-'} · ${fmtScore(score)}${info.nps ? ' · ' + fmtNodes(info.nps) + ' nodes/s' : ''}`;
  }

  showSetup(on) {
    this.wantSetup = on;
    if (on) this.preview();
    else this.show();
  }

  show() {
    if (!this.active) return;
    const s = this.state;
    const g = s && s.game;
    const live = !!g;
    const mine = !!(s && s.you);
    const watching = live && !mine && !g.over;
    $('#play-busy').classList.toggle('hidden', !watching && !(s && s.queuePos != null));
    $('#play-queue').classList.toggle('hidden', !!(s && s.queuePos != null));
    $('#play-unqueue').classList.toggle('hidden', !(s && s.queuePos != null));
    if (s && s.queuePos != null) {
      $('#play-queue-text').textContent = s.canStart ? 'Kairos is free: start your game now.' : `You are number ${s.queuePos + 1} in the queue. You can watch the game meanwhile.`;
    } else if (watching) {
      $('#play-queue-text').textContent = s.queue ? `${s.queue} waiting. You can watch the game live, or join the queue.` : 'You can watch the game live. Join the queue to play next.';
    }
    const showForm = !s || s.canStart || (!live && !watching);
    $('#play-form').classList.toggle('hidden', !(showForm && (!mine || !g || g.over || this.wantSetup)));
    $('#play-start').disabled = !!(s && !s.canStart);
    $('#play-setup').classList.toggle('hidden', $('#play-form').classList.contains('hidden') && $('#play-busy').classList.contains('hidden'));
    $('#play-live').classList.toggle('hidden', !live || (mine && this.wantSetup && g.over));
    $('#play-spectators').textContent = s && s.spectators > 1 ? `${s.spectators - 1} watching` : '';

    if (live) {
      const human = g.humanColor;
      const youName = mine ? 'You' : 'Challenger';
      const topBlack = human === 'w';
      $('#clock-top-name').textContent = 'Kairos';
      $('#clock-bottom-name').textContent = youName;
      this.clockSides = topBlack ? ['b', 'w'] : ['w', 'b'];
      let status;
      if (g.over) status = `${g.over.result.replace('1/2-1/2', '½-½')} · ${g.over.reason}`;
      else if (g.toMove === human) status = mine ? 'Your move' : `${youName} to move`;
      else status = 'Kairos is thinking…';
      $('#play-status').textContent = status;
      $('#play-status').classList.toggle('yours', !g.over && mine && g.toMove === human);
      if (!g.thinking) $('#play-thinking').textContent = g.over ? '' : (g.clock ? `${g.clock.base / 60000}+${g.clock.inc / 1000}` : `untimed · ${g.movetime / 1000} s per action`) + ` · ${g.variant}`;
      $('#play-resign').classList.toggle('hidden', !mine || !!g.over);
      $('#play-new').classList.toggle('hidden', !mine || !g.over);
      this.tickClocks();
      if (!mine && !this.flippedFor && !this.previewing) {
        this.board.setFlipped(human === 'b');
        this.flippedFor = true;
      }
    }
    this.board.setOverlay({ readonly: !this.myTurn(), pv: [] });
    this.renderMoves();
    if (this.tree) {
      try { this.input.refresh(true); } catch (e) { toast(e.message, { error: true }); }
    }
    $('#eval-bar').classList.add('hidden');
  }

  onView(v) {
    const chip = $('#status-chip');
    chip.className = 'status-line';
    const g = this.state && this.state.game;
    if (v.status === 'checkmate') { chip.textContent = `Checkmate · ${v.player ? 'White' : 'Black'} wins`; chip.classList.add('over'); }
    else if (v.status === 'stalemate') { chip.textContent = 'Stalemate · draw'; chip.classList.add('over'); }
    else if (g) {
      const side = v.player ? 'Black' : 'White';
      chip.textContent = `T${v.present} · ${side} to move${v.checks.length ? ' · in check' : ''}`;
      if (v.checks.length) chip.classList.add('check');
    } else chip.textContent = '';
  }

  renderMoves() {
    const box = $('#play-moves');
    if (!this.tree || this.tree.nodes.length < 2) {
      box.replaceChildren(el('div', { class: 'moves-empty', text: this.state && this.state.game ? 'No actions yet.' : 'Choose a variant, side and time control, then start.' }));
      return;
    }
    const rows = [];
    let row = null;
    const rootBlack = this.rules.call({ cmd: 'view', node: 0, draft: [] }).view.player;
    for (let c = 0; this.tree.nodes[c].children.length;) {
      c = this.tree.nodes[c].children[this.tree.nodes[c].children.length - 1];
      const n = this.tree.nodes[c];
      const k = n.ply - 1 + rootBlack;
      const black = k % 2 === 1;
      if (!row || !black) {
        row = el('div', { class: 'mv-row' }, el('div', { class: 'mv-num', text: Math.floor(k / 2) + 1 + '.' }));
        rows.push(row);
        if (black) row.append(el('div', { class: 'mv empty', text: '…' }));
      }
      row.append(el('div', { class: 'mv' + (c === this.cur ? ' cur' : ''), dataset: { id: c }, title: n.lan, text: n.san || n.lan }));
      if (black) row = null;
    }
    box.replaceChildren(...rows);
    const cur = box.querySelector('.cur');
    if (cur) cur.scrollIntoView({ block: 'nearest' });
  }

  tickClocks() {
    const s = this.state;
    const g = s && s.game;
    if (!this.active || !g || !this.clockSides) return;
    const left = g.left ? [...g.left] : null;
    const running = !g.over ? (g.toMove === 'w' ? 0 : 1) : -1;
    if (left && running >= 0) left[running] = Math.max(0, left[running] - (performance.now() - this.stateAt));
    const [top, bottom] = this.clockSides;
    const idx = (c) => (c === 'w' ? 0 : 1);
    const set = (pos, side) => {
      const box = $(`#clock-${pos}`);
      const ms = left ? left[idx(side)] : null;
      $(`#clock-${pos}-time`).textContent = left ? fmtClock(ms) : '∞';
      box.classList.toggle('running', running === idx(side));
      box.classList.toggle('low', left != null && ms < 20000);
    };
    set('top', top);
    set('bottom', bottom);
  }

  analyze() {
    const g = this.state && this.state.game;
    if (!g) return;
    const uci = g.start + (g.moves.length ? ' moves ' + g.moves.map((m) => m + ' submit').join(' ') : '');
    location.hash = `#/analysis?uci=${encodeURIComponent(uci)}`;
  }

  key(e) {
    const k = e.key;
    if (k === 'Enter') this.input.submit();
    else if (k === 'Backspace') this.input.undo();
    else if (k === 'Escape') this.input.clear();
    else if (k === 'ArrowLeft' || k === 'ArrowRight') {
      const n = this.tree && this.tree.nodes[this.cur];
      if (!n) return false;
      const next = k === 'ArrowLeft' ? n.parent : n.children[n.children.length - 1];
      if (next != null) { this.cur = next; this.input.reset(); this.show(); }
    } else return false;
    return true;
  }
}
