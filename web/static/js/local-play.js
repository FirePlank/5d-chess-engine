// Play against Kairos without a server (the static site): the game, its rules and clocks run
// in this page and the engine is the in-browser one (the analysis engine, with its settings).
// It answers the same requests as kairos-web's /api/play/* (start, move, resign, leave) with
// the same state object, so play.js drives both alike. Every visitor plays their own engine:
// there is no queue and nobody watches.
import { Rules, parseInfo } from './kairos.js';

const PONDER_MS = 86_400_000;

export class LocalPlay {
  constructor(engine, { onState, onThinking }) {
    this.engine = engine;
    this.onState = onState;
    this.onThinking = onThinking;
    this.game = null;
    this.gameId = 0;
    this.searchId = null; // the engine search playing Kairos's action
    this.lastEmit = 0;
  }

  async init() {
    this.rules = await Rules.create();
    this.engine.on((m) => this.onEngine(m));
    setInterval(() => this.tick(), 200);
  }

  /** The kairos-web play state, for this page's own game. */
  state() {
    const g = this.game;
    return {
      ok: true,
      busy: !!g,
      locked: false,
      you: !!g,
      queue: 0,
      queuePos: null,
      canStart: true,
      spectators: 1,
      game: g && {
        variant: g.variant,
        start: g.start,
        humanColor: g.humanColor,
        moves: [...g.moves],
        clock: g.clock,
        left: this.clockNow(),
        toMove: g.toMove,
        movetime: g.movetime,
        book: g.book,
        thinking: g.thinking,
        over: g.over,
        info: g.info,
        evals: [...g.evals],
      },
    };
  }

  async post(what, body = {}) {
    const g = this.game;
    if (what === 'start') this.start(body);
    else if (what === 'move') {
      if (!g) throw new Error('not your game');
      if (g.over) throw new Error('the game is over');
      if (g.toMove !== g.humanColor) throw new Error('not your turn');
      this.play(body.lan);
    } else if (what === 'resign' || what === 'leave') {
      if (g && !g.over) this.finish(g.humanColor === 'b' ? '1-0' : '0-1', what === 'leave' ? 'abandoned' : 'resignation');
      if (what === 'leave') this.game = null;
    } else if (what !== 'queue') throw new Error('unknown request');
    return this.state();
  }

  start({ variant = 'Standard', color = 'random', clock = null, movetime = 2000, book = true }) {
    this.engine.stop();
    this.rules.call({ cmd: 'new', position: variant });
    const start = this.view(0).position;
    const humanColor = color === 'w' || color === 'b' ? color : Math.random() < 0.5 ? 'w' : 'b';
    const base = clock ? clock.base : 0;
    this.gameId++;
    this.game = {
      id: this.gameId, variant, start, humanColor, node: 0, moves: [], evals: [],
      clock: clock && { base: clock.base, inc: clock.inc }, left: [base, base], turnStart: performance.now(),
      movetime, book: book !== false, toMove: 'w', thinking: false, over: null, info: null, fresh: true,
    };
    this.afterAction();
  }

  view(node) { return this.rules.call({ cmd: 'view', node, draft: [] }).view; }

  /** Play an action (LAN) for the side to move. Throws if it is illegal. */
  play(lan) {
    const g = this.game;
    const r = this.rules.call({ cmd: 'play', node: g.node, lan });
    this.chargeClock();
    g.node = r.node;
    g.moves.push(r.tree.nodes[r.node].lan);
    g.turnStart = performance.now();
    this.afterAction();
  }

  /** Remaining clock times, the running side's elapsed time taken off. */
  clockNow() {
    const g = this.game;
    if (!g || !g.clock) return null;
    const l = [...g.left];
    if (!g.over) {
      const side = g.toMove === 'w' ? 0 : 1;
      l[side] = Math.max(0, l[side] - (performance.now() - g.turnStart));
    }
    return l;
  }

  chargeClock() {
    const g = this.game;
    if (!g.clock) return;
    const side = g.toMove === 'w' ? 0 : 1;
    g.left[side] = Math.max(0, g.left[side] - (performance.now() - g.turnStart)) + g.clock.inc;
  }

  /** After an action: game over, or Kairos's turn (search), or the human's (ponder). */
  afterAction() {
    const g = this.game;
    const v = this.view(g.node);
    g.toMove = v.player ? 'b' : 'w';
    if (v.status === 'checkmate') return this.finish(v.player ? '1-0' : '0-1', 'checkmate');
    if (v.status === 'stalemate') return this.finish('1/2-1/2', 'stalemate');
    if (v.status === 'exiled-white') return this.finish('0-1', 'exiled-white');
    if (v.status === 'exiled-black') return this.finish('1-0', 'exiled-black');
    if (g.toMove !== g.humanColor) this.think(v.position);
    else this.ponder(v.position);
  }

  async think(position) {
    const g = this.game;
    g.thinking = true;
    g.info = null;
    const side = g.toMove === 'w' ? 0 : 1;
    const limits = g.clock
      ? { movetime: null, timeLeft: Math.round(this.clockNow()[side]), increment: g.clock.inc }
      : { movetime: g.movetime };
    const newGame = g.fresh;
    g.fresh = false;
    this.searchId = null;
    this.searchFor = { id: g.id, position, lastScore: null };
    try {
      // The opening book answers book positions at once (the in-browser engine's OwnBook).
      const id = await this.engine.search({ position, newGame, book: g.book, ...limits });
      if (this.game === g && this.searchFor && this.searchFor.position === position) this.searchId = id;
    } catch (e) {
      if (this.game === g && !g.over) this.finish(g.humanColor === 'w' ? '1-0' : '0-1', 'engine failed to start: ' + e.message);
      this.onState(this.state());
    }
  }

  /** Think on the human's time: the table (kept across actions) fills for the reply. */
  ponder(position) {
    this.searchId = null;
    this.searchFor = null;
    // Without shared memory a search cannot be stopped (only its worker replaced), so no ponder.
    if (!this.engine.shared) return;
    this.engine.search({ position, movetime: PONDER_MS }).catch(() => {});
  }

  /** Back on the Play page: if Kairos's search was replaced meanwhile (analysis), search again. */
  resume() {
    const g = this.game;
    if (!g || g.over || g.toMove === g.humanColor) return;
    if (this.searchId != null && this.engine.busy && this.engine.reqId === this.searchId) return;
    this.think(this.view(g.node).position);
  }

  onEngine(m) {
    const g = this.game;
    if (!g || g.over || this.searchId == null || m.id !== this.searchId) return;
    if (m.type === 'info') {
      const info = parseInfo(m.line, false);
      if (info.cp != null) this.searchFor.lastScore = info.cp;
      else if (info.mate != null) this.searchFor.lastScore = info.mate > 0 ? 20000 : -20000;
      g.info = info;
      const now = performance.now();
      if (now - this.lastEmit > 100) {
        this.lastEmit = now;
        this.onThinking(info);
      }
    } else if (m.type === 'done') {
      this.searchId = null;
      g.thinking = false;
      const score = this.searchFor ? this.searchFor.lastScore : null;
      if (!m.bestmove) {
        // No action found: the position is over (the status check ran out of time on it).
        const v = this.view(g.node);
        this.finish(v.checks.length ? (v.player ? '1-0' : '0-1') : '1/2-1/2', v.checks.length ? 'checkmate' : 'stalemate');
      } else {
        try {
          g.evals.push(score);
          this.play(m.bestmove);
        } catch (e) {
          this.finish(g.humanColor === 'w' ? '1-0' : '0-1', 'engine played an illegal action: ' + e.message);
        }
      }
      this.onState(this.state());
    } else if (m.type === 'error') {
      this.searchId = null;
      g.thinking = false;
      this.finish(g.humanColor === 'w' ? '1-0' : '0-1', 'engine error: ' + m.message);
      this.onState(this.state());
    }
  }

  finish(result, reason) {
    const g = this.game;
    g.left = this.clockNow() || g.left;
    g.over = { result, reason };
    g.thinking = false;
    this.searchId = null;
    this.searchFor = null;
    this.engine.stop();
  }

  /** Flag fall. */
  tick() {
    const g = this.game;
    if (!g || g.over || !g.clock) return;
    const side = g.toMove === 'w' ? 0 : 1;
    if (this.clockNow()[side] > 0) return;
    const human = g.toMove === g.humanColor;
    this.finish(side === 0 ? '0-1' : '1-0', human ? 'time forfeit' : 'engine time forfeit');
    this.onState(this.state());
  }
}
