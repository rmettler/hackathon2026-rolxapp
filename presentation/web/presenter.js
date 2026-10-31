// Presenter view: drives the shared slide state. A clicker sends page up/down, which work here as well.

import { SLIDES } from '/slides.mjs';

const $ = (selector) => document.querySelector(selector);
const pad = (n) => String(n).padStart(2, '0');

let state = null;
let clockOffset = 0;

const LINK_NAMES = { trainScreen: 'Train, screen', trainCamera: 'Train, camera' };
const SLIDE_NAMES = {
  title: 'Title',
  claim: 'Claim',
  core: 'Core',
  phone: 'Phone',
  desk: 'Desk',
  train: 'Train',
  all: 'All',
  rust: 'Rust',
  end: 'End',
};

async function post(path, body) {
  try {
    await fetch(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  } catch {
    // The status shows that the server is gone.
  }
}

function setStatus(name, ok, text) {
  const el = $(`[data-status='${name}']`);
  el.dataset.ok = ok;
  el.textContent = text;
}

function render() {
  const slide = SLIDES[state.index];
  const steps = slide.steps ?? 0;
  $('.position').textContent =
    `${state.index + 1} of ${SLIDES.length}` + (steps ? `, step ${state.step + 1} of ${steps + 1}` : '');
  $('.notes').innerHTML = (slide.notes ?? []).map((note) => `<li>${note}</li>`).join('');
  document.querySelectorAll('.slides button').forEach((button, i) => {
    if (i === state.index) button.setAttribute('aria-current', 'step');
    else button.removeAttribute('aria-current');
  });
  setStatus('obs', state.obs.connected, state.obs.connected ? `OBS ${state.obs.message}` : `OBS: ${state.obs.message}`);
  tick();
}

function tick() {
  if (!state) return;
  let elapsed = 0;
  let status = 'idle';
  if (state.stoppedElapsed !== null) [elapsed, status] = [state.stoppedElapsed, 'stopped'];
  else if (state.startedAt !== null) [elapsed, status] = [Date.now() + clockOffset - state.startedAt, 'running'];
  const s = Math.max(0, Math.floor(elapsed / 1000));
  $('.clock-time').textContent = `${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}`;
  $('.clock').dataset.clock = status;
}

async function main() {
  $('.slides').innerHTML = SLIDES.map(
    (slide, i) => `<li style="display: contents"><button type="button" data-goto="${i}">${i + 1} ${SLIDE_NAMES[slide.id] ?? slide.id}</button></li>`,
  ).join('');

  document.addEventListener('click', (event) => {
    const button = event.target.closest('button');
    if (!button) return;
    if (button.dataset.go) post('/api/go', { action: button.dataset.go });
    if (button.dataset.goto) post('/api/go', { action: 'goto', index: Number(button.dataset.goto) });
    if (button.dataset.timer === 'start') post('/api/timer', { action: 'start' });
    if (button.dataset.timer === 'reset' && confirm('Reset the talk timer to 00:00:00?')) post('/api/timer', { action: 'reset' });
    if (button.dataset.copy) {
      navigator.clipboard.writeText(button.dataset.copy);
      button.textContent = 'Copied';
      setTimeout(() => (button.textContent = 'Copy'), 1500);
    }
    button.blur();
  });

  const keys = { ArrowRight: 'next', PageDown: 'next', ' ': 'next', ArrowLeft: 'prev', PageUp: 'prev' };
  addEventListener('keydown', (event) => {
    if (event.repeat || event.ctrlKey || event.altKey || event.metaKey) return;
    if (event.target.closest('summary')) return;
    if (keys[event.key]) {
      event.preventDefault();
      post('/api/go', { action: keys[event.key] });
    } else if (event.key === 's' || event.key === 'S') {
      post('/api/timer', { action: 'start' });
    }
  });

  const { links } = await (await fetch('/api/config')).json();
  $('.link-list').innerHTML = Object.entries(links)
    .map(
      ([key, link]) =>
        `<span>${LINK_NAMES[key] ?? key}</span><code title="${link.push}">${link.push}</code><button type="button" data-copy="${link.push}">Copy</button>`,
    )
    .join('');

  setStatus('server', false, 'Server: connecting');
  const events = new EventSource('/api/events');
  events.onopen = () => setStatus('server', true, 'Server connected');
  events.onerror = () => setStatus('server', false, 'Server: not reachable');
  events.onmessage = (event) => {
    state = JSON.parse(event.data);
    clockOffset = state.serverNow - Date.now();
    render();
  };
  setInterval(tick, 250);
}

main();
