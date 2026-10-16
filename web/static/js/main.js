// App shell: routing between Analysis, Play and Games, theme, shared board and engine.
import { MultiverseView } from './board.js';
import { MoveInput } from './input.js';
import { Engine, defaultEngineSettings } from './kairos.js';
import { Analysis } from './analysis.js';
import { Play } from './play.js';
import { Games } from './games.js';
import { setRules } from './static-api.js';
import { $, $$, toast, store } from './util.js';

/** Saved analysis-engine settings over the defaults. */
export function engineSettings() {
  const s = { ...defaultEngineSettings(), ...(store('engineSettings') || {}) };
  // Never more threads than logical cores (a setting saved on another machine, say).
  s.threads = Math.max(1, Math.min(s.threads | 0, navigator.hardwareConcurrency || 2));
  return s;
}

const app = {
  route: null,
  board: null,
  input: null,
  engine: new Engine(engineSettings()),
  server: null,
};

async function serverInfo() {
  try {
    const r = await fetch('api/info');
    if (r.ok) return await r.json();
  } catch (e) { /* static hosting: no server */ }
  return null;
}

function route() {
  const hash = location.hash || '#/analysis';
  const [path, qs] = hash.slice(2).split('?');
  const name = ['analysis', 'play', 'games'].includes(path) ? path : 'analysis';
  const params = new URLSearchParams(qs || '');
  if (app.route !== name) {
    const prev = app.pages[app.route];
    if (prev && prev.deactivate) prev.deactivate();
  }
  app.route = name;
  $$('.tabs a').forEach((a) => a.classList.toggle('on', a.dataset.route === name));
  $('#workspace').classList.toggle('hidden', name === 'games');
  $('#browser').classList.toggle('hidden', name !== 'games');
  $('#side-analysis').classList.toggle('hidden', name !== 'analysis');
  $('#side-play').classList.toggle('hidden', name !== 'play');
  if (name !== 'games') app.board.resize();
  app.pages[name].activate(params);
}

function setTheme(t) {
  document.documentElement.dataset.theme = t;
  try { localStorage.setItem('kairos.theme', t); } catch (e) { /* private mode */ }
  app.board.readColors();
  if (app.pages && app.pages.analysis) app.pages.analysis.renderGraph();
}

function bindKeys() {
  document.addEventListener('keydown', (e) => {
    if (e.target.closest('input, textarea, select, dialog') || e.ctrlKey || e.metaKey || e.altKey) return;
    const k = e.key;
    let handled = true;
    if (k === 'f' || k === 'F') {
      app.board.setFlipped(!app.board.flipped);
      $('#tool-flip').classList.toggle('on', app.board.flipped);
    } else if (k === 'p' || k === 'P') app.board.goPresent();
    else if (k === '0') app.board.fit();
    else if (k === '+' || k === '=') app.board.zoomAt(1.25);
    else if (k === '-' || k === '_') app.board.zoomAt(0.8);
    else if (k === '?') $('#dlg-help').showModal();
    else {
      const page = app.pages[app.route];
      handled = !!(page && page.key && page.key(e));
    }
    if (handled) e.preventDefault();
  });
}

async function main() {
  app.board = new MultiverseView($('#board'));
  app.input = new MoveInput(app.board);
  app.board.onHover = (sq) => {
    const v = app.board.view;
    if (!sq || !v) { $('#coord-chip').textContent = ''; return; }
    const tl = v.timelines.find((t) => t.l === sq.l);
    $('#coord-chip').textContent = `L${tl ? tl.label : sq.l} T${sq.t}${sq.c ? 'b' : 'w'} · ${'abcdefgh'[sq.x]}${sq.y + 1}`;
  };
  $('#tool-fit').onclick = () => app.board.fit();
  $('#tool-present').onclick = () => app.board.goPresent();
  $('#tool-zoom-in').onclick = () => app.board.zoomAt(1.25);
  $('#tool-zoom-out').onclick = () => app.board.zoomAt(0.8);
  $('#tool-flip').onclick = () => {
    app.board.setFlipped(!app.board.flipped);
    $('#tool-flip').classList.toggle('on', app.board.flipped);
  };
  $('#btn-theme').onclick = () => setTheme(document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark');
  // Board square colours: brown, blue, green (lichess themes).
  $('#btn-board-theme').onclick = () => {
    const themes = ['brown', 'blue', 'green'];
    const next = themes[(themes.indexOf(document.documentElement.dataset.board) + 1) % themes.length];
    document.documentElement.dataset.board = next;
    store('boardTheme', next);
    app.board.readColors();
    toast(`Board: ${next}`);
  };
  $('#btn-help').onclick = () => $('#dlg-help').showModal();
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    let saved = null;
    try { saved = localStorage.getItem('kairos.theme'); } catch (err) { /* ignore */ }
    if (!saved) setTheme(e.matches ? 'dark' : 'light');
  });
  bindKeys();

  app.server = await serverInfo();
  app.pages = { analysis: new Analysis(app), play: new Play(app), games: new Games(app) };
  try {
    await app.pages.analysis.init();
    setRules(app.pages.analysis.rules);
  } catch (e) {
    console.error(e);
    toast('Could not start the Kairos core: ' + e.message, { error: true, ms: 15000 });
    return;
  }
  await app.pages.play.init();
  app.pages.games.init();
  window.addEventListener('hashchange', route);
  route();
  // Warm up the engine worker in the background.
  app.engine.start().catch((e) => console.warn('engine:', e));
}

main();
window.kairosApp = app;
