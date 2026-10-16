// Building an action on the board: select a piece, click a target, repeat on every board you
// must (or want to) play, then submit. Shared by the analysis board and play mode.
import { $, el, toast } from './util.js';

export class MoveInput {
  constructor(board) {
    this.board = board;
    this.source = null; // {rules, node, canMove(), submit(moves), onView(view)}
    this.draft = [];
    this.selected = null;
    this.targets = [];
    this.autoSubmit = true;
    board.onSquare = (sq) => this.click(sq);
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
    this.board.setOverlay({ selected: null, targets: [] });
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
    if (this.source.onView) this.source.onView(v, this.draft);
    return v;
  }

  click(sq) {
    const s = this.source;
    if (!s || !s.canMove()) {
      if (sq && s && s.blocked) s.blocked();
      return;
    }
    if (!sq) return this.deselect();
    if (this.selected) {
      const tgt = this.targets.find((t) => t[0] === sq.l && t[1] === sq.t && t[2] === sq.x && t[3] === sq.y && sq.c === this.board.view.player);
      if (tgt) return this.move(this.selected, tgt);
    }
    const from = [sq.l, sq.t, sq.x, sq.y];
    if (this.selected && this.selected.l === sq.l && this.selected.t === sq.t && this.selected.x === sq.x && this.selected.y === sq.y) return this.deselect();
    if (!this.board.isInteresting(sq)) return this.deselect();
    let targets = [];
    try {
      targets = s.rules.call({ cmd: 'targets', node: s.node(), draft: this.draft, from }).targets;
    } catch (e) {
      toast(e.message, { error: true });
    }
    this.selected = { ...sq };
    this.targets = targets;
    this.board.setOverlay({ selected: this.selected, targets });
  }

  deselect() {
    this.selected = null;
    this.targets = [];
    this.board.setOverlay({ selected: null, targets: [] });
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
      hint.textContent = 'Ready: submit, or keep playing on other boards you may move on.';
      hint.classList.remove('bad');
    } else {
      hint.textContent = v.submitError ? cap(v.submitError) : '';
      hint.classList.toggle('bad', !!v.submitError && !/every board on the present/.test(v.submitError));
      if (/every board on the present/.test(v.submitError || '')) hint.textContent = 'Play a move on every highlighted board on the present, then submit.';
    }
  }
}

function cap(s) { return s ? s[0].toUpperCase() + s.slice(1) : s; }

const files = 'abcdefgh';
export function lanText(m) {
  const sq = (p) => files[p[2]] + (p[3] + 1);
  const a = `(${m.from[0]}T${m.from[1]})${sq(m.from)}`;
  const same = m.from[0] === m.to[0] && m.from[1] === m.to[1];
  return same ? a + sq(m.to) : `${a}(${m.to[0]}T${m.to[1]})${sq(m.to)}`;
}
