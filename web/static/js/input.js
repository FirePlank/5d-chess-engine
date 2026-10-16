// Building an action on the board: select a piece, click a target, repeat on every board you
// must (or want to) play, then submit. Shared by the analysis board and play mode.
import { $, el, toast, store } from './util.js';

export class MoveInput {
  constructor(board) {
    this.board = board;
    this.source = null; // {rules, node, canMove(), submit(moves), onView(view)}
    this.draft = [];
    this.selected = null;
    this.targets = [];
    // Like the Steam game, an action waits for Submit; auto-submit (toolbar) sends a lone
    // action as soon as nothing else can be played.
    this.autoSubmit = store('autoSubmit') === true;
    this.peek = false;
    board.onSquare = (sq) => this.click(sq);
    const tool = $('#tool-autosubmit');
    const showTool = () => {
      tool.classList.toggle('on', this.autoSubmit);
      tool.title = this.autoSubmit
        ? 'Auto-submit is on: an action is sent once nothing else can be played (click to turn off)'
        : 'Auto-submit is off: press Submit or Enter after every action, like the Steam game (click to turn on)';
    };
    tool.onclick = () => {
      this.autoSubmit = !this.autoSubmit;
      store('autoSubmit', this.autoSubmit);
      showTool();
      toast(this.autoSubmit ? 'Auto-submit on' : 'Auto-submit off: press Submit (Enter) after each action');
    };
    showTool();
    $('#draft-undo').onclick = () => this.undo();
    $('#draft-reset').onclick = () => this.clear();
    $('#draft-submit').onclick = () => this.submit();
  }

  /** Attach to a game source; resets any action in progress. */
  attach(source) {
    this.source = source;
    this.reset();
  }

  reset() {
    this.draft = [];
    this.selected = null;
    this.targets = [];
    this.peek = false;
    this.board.setOverlay({ selected: null, targets: [], targetColor: null, peek: false });
    this.renderBar(null);
  }

  get active() { return this.draft.length > 0; }

  /** The current position (with the draft applied), from the rules core. */
  view() {
    const s = this.source;
    return s.rules.call({ cmd: 'view', node: s.node(), draft: this.draft }).view;
  }

  refresh(follow = true) {
    const v = this.view();
    this.board.setView(v, { follow });
    this.renderBar(v);
    renderHud(v);
    if (this.source.onView) this.source.onView(v, this.draft);
    return v;
  }

  /**
   * A click on the board: select one of your pieces on a playable board and move it, or
   * click any other piece on a board of its own colour (an opponent's piece on a board your
   * action just made, say) to see where it could go. Seeing moves never plays anything.
   */
  click(sq) {
    const s = this.source;
    if (!s) return;
    if (!sq) return this.deselect();
    const canMove = s.canMove();
    if (this.selected && !this.peek && canMove) {
      const tgt = this.targets.find((t) => t[0] === sq.l && t[1] === sq.t && t[2] === sq.x && t[3] === sq.y && sq.c === this.board.view.player);
      if (tgt) return this.move(this.selected, tgt);
    }
    const from = [sq.l, sq.t, sq.x, sq.y];
    if (this.selected && this.selected.l === sq.l && this.selected.v === sq.v && this.selected.x === sq.x && this.selected.y === sq.y) return this.deselect();
    const own = canMove && this.board.isInteresting(sq);
    const peek = !own && this.board.isPeekable(sq);
    if (!own && !peek) {
      if (!canMove && s.blocked && this.board.isInteresting(sq)) s.blocked();
      return this.deselect();
    }
    let targets = [];
    try {
      const req = { cmd: 'targets', node: s.node(), draft: this.draft, from };
      if (peek) req.color = sq.c;
      targets = s.rules.call(req).targets;
    } catch (e) {
      toast(e.message, { error: true });
    }
    this.selected = { ...sq };
    this.targets = targets;
    this.peek = peek;
    this.board.setOverlay({ selected: this.selected, targets, targetColor: peek ? sq.c : null, peek });
  }

  deselect() {
    this.selected = null;
    this.targets = [];
    this.peek = false;
    this.board.setOverlay({ selected: null, targets: [], targetColor: null, peek: false });
  }

  move(sel, tgt) {
    const s = this.source;
    const mv = { from: [sel.l, sel.t, sel.x, sel.y], to: tgt };
    try {
      s.rules.call({ cmd: 'check_move', node: s.node(), draft: this.draft, move: mv });
    } catch (e) {
      toast(e.message, { error: true });
      return;
    }
    this.draft.push(mv);
    this.deselect();
    const v = this.refresh();
    // A lone action (nothing else left to play) submits itself.
    const morePlayable = v.timelines.some((tl) => tl.end[1] === v.player && tl.status !== 'unplayable');
    if (this.autoSubmit && v.canSubmit && !morePlayable) this.submit();
  }

  undo() {
    if (!this.draft.length) return;
    this.draft.pop();
    this.deselect();
    this.refresh();
  }

  clear() {
    if (!this.draft.length) return this.deselect();
    this.draft = [];
    this.deselect();
    this.refresh();
  }

  /**
   * Submit the action once. The draft is cleared before the source applies it: the source
   * (and any state update arriving meanwhile) redraws the new position, which must not
   * replay the submitted moves on top of it.
   */
  async submit() {
    if (!this.draft.length || this.submitting) return;
    const moves = this.draft.slice();
    const node = this.source.node();
    this.submitting = true;
    this.draft = [];
    this.deselect();
    this.renderBar(null);
    try {
      await this.source.submit(moves);
    } catch (e) {
      toast(e.message, { error: true });
      // Rejected before the position changed: give the moves back to keep editing.
      if (this.source.node() === node) {
        this.draft = moves;
        try { this.refresh(false); } catch (err) { this.reset(); }
      }
    } finally {
      this.submitting = false;
    }
  }

  renderBar(v) {
    const bar = $('#draft-bar');
    if (!this.draft.length || !v) {
      bar.classList.add('hidden');
      return;
    }
    bar.classList.remove('hidden');
    const moves = $('#draft-moves');
    moves.replaceChildren(...this.draft.map((m) => el('span', { class: 'chip', text: lanText(m) })));
    const hint = $('#draft-hint');
    $('#draft-submit').disabled = !v.canSubmit;
    if (v.canSubmit) {
      hint.textContent = 'Ready: Submit (Enter), or keep playing other boards. Click any piece to see its moves.';
      hint.classList.remove('bad');
    } else {
      hint.textContent = v.submitError ? cap(v.submitError) : '';
      hint.classList.toggle('bad', !!v.submitError && !/every board on the present/.test(v.submitError));
      if (/every board on the present/.test(v.submitError || '')) hint.textContent = 'Play a move on every highlighted board on the present, then submit.';
    }
  }
}

/**
 * The present and timeline advantage, as in the Steam game: timelines each side has created,
 * who leads, and whether the side to move would get an active timeline by branching now (a
 * side may lead the other by one active timeline; further ones are inactive).
 */
export function renderHud(v) {
  const hud = $('#mv-hud');
  if (!v) return hud.replaceChildren();
  const side = v.player ? 'Black' : 'White';
  const dot = (black) => el('i', { class: 'side-dot ' + (black ? 'black' : 'white') });
  const present = el('span', { class: 'hud-item', title: 'The present: the earliest turn among the active timelines. Every active board on it must be played.' },
    dot(v.player), el('b', { text: `Present T${v.present}` }), el('span', { class: 'muted', text: `${side} to move` }));
  const white = v.lines[1] - v.initialLines[1];
  const black = v.initialLines[0] - v.lines[0];
  const inactive = (v.active[0] - v.lines[0]) + (v.lines[1] - v.active[1]);
  const lead = white - black;
  const adv = lead > 0 ? `White +${lead}` : lead < 0 ? `Black +${-lead}` : 'Even';
  const mine = v.player ? black : white, theirs = v.player ? white : black;
  const next = mine <= theirs ? 'active' : 'inactive';
  const items = [present];
  if (white || black || inactive) {
    items.push(el('span', { class: 'hud-item', title: `Timelines created: White ${white}, Black ${black}. A side may lead by one active timeline; ${inactive} timeline(s) are inactive. A new timeline by ${side} now would be ${next}.` },
      el('b', { text: 'Timelines' }), el('span', { text: `W ${white} · B ${black}` }), el('b', { text: adv }),
      el('span', { class: 'muted', text: inactive ? `${inactive} inactive` : `next ${side[0]} branch ${next}` })));
  }
  hud.replaceChildren(...items);
}

function cap(s) { return s ? s[0].toUpperCase() + s.slice(1) : s; }

const files = 'abcdefgh';
export function lanText(m) {
  const sq = (p) => files[p[2]] + (p[3] + 1);
  const a = `(${m.from[0]}T${m.from[1]})${sq(m.from)}`;
  const same = m.from[0] === m.to[0] && m.from[1] === m.to[1];
  return same ? a + sq(m.to) : `${a}(${m.to[0]}T${m.to[1]})${sq(m.to)}`;
}
