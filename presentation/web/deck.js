// The deck: follows the server's slide state (or a fixed offset from it, for the presenter's previews).
//
//   deck.html             browser window: arrow keys, space and page up/down move the slides; the frames show the
//                         live screens (VDO.Ninja) and the video themselves
//   deck.html?mode=obs    OBS browser source: no keys, empty frames (OBS puts the feeds on top)
//   deck.html?offset=1    presenter preview of the next click, without transitions or live screens

import { icon } from '/icons.mjs';
import { FEEDS, FRAME_PAD, LAYOUTS, SLIDES, STAGE, stepsOf } from '/slides.mjs';

const params = new URLSearchParams(location.search);
const mode = params.get('mode') ?? 'browser';
const offset = Number(params.get('offset') ?? 0);
const animate = offset === 0 && 'startViewTransition' in document;
// Every live screen is one more viewer the sharing device has to upload to, so only the real deck shows them.
const live = mode === 'browser' && offset === 0 && params.get('live') !== '0';

const stage = document.querySelector('.stage');
const stamp = document.querySelector('.stamp');
const stampTimer = document.querySelector('.stamp-timer');
const stampStatus = document.querySelector('.stamp-status');
stage.dataset.mode = mode;

let state = null;
let clockOffset = 0; // server clock minus ours
let config = { people: {}, talk: '', sources: {} };

// ---- Static parts --------------------------------------------------------------------------------------------

function fitStage() {
  const scale = Math.min(innerWidth / STAGE.width, innerHeight / STAGE.height);
  stage.style.setProperty('--scale', scale);
}

function personHtml(key, role, iconName) {
  const name = config.people[key];
  const nameHtml = name ? `<span class="person-name">${escapeHtml(name)}</span>` : '';
  return `${icon(iconName)}<span class="person-text">${nameHtml}<span class="person-role">${role}</span></span>`;
}

const escapeHtml = (text) =>
  text.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

function buildStatic() {
  for (const el of document.querySelectorAll('.node[data-icon]')) {
    el.insertAdjacentHTML('afterbegin', icon(el.dataset.icon));
  }
  for (const el of document.querySelectorAll('.person[data-feed]')) {
    const feed = FEEDS[el.dataset.feed];
    el.innerHTML = personHtml(feed.person, feed.label, feed.icon);
  }
  for (const el of document.querySelectorAll('[data-talk]')) el.textContent = config.talk;

  const frames = document.querySelector('.frames');
  frames.innerHTML = Object.entries(FEEDS)
    .map(
      ([key, feed]) => `
      <div class="frame" data-feed="${key}" style="--frame-pad: ${FRAME_PAD}px; view-transition-name: frame-${key}" hidden>
        <div class="frame-screen"><span class="frame-hint">${icon(feed.icon)}${feed.label}</span></div>
      </div>`,
    )
    .join('');

  if (live) {
    for (const frame of frames.querySelectorAll('.frame')) {
      const source = config.sources?.[frame.dataset.feed];
      const screen = frame.querySelector('.frame-screen');
      if (source?.kind === 'vdo') {
        screen.insertAdjacentHTML('beforeend', `<iframe class="frame-live" src="${source.src}" allow="autoplay; fullscreen" title="${FEEDS[frame.dataset.feed].label}"></iframe>`);
      } else if (source?.kind === 'video') {
        screen.insertAdjacentHTML('beforeend', `<video class="frame-live" src="${source.src}" muted playsinline preload="auto"></video>`);
      }
    }
  }

  document.querySelector('.bar').innerHTML = SLIDES.map(() => '<span class="segment"><span class="segment-fill"></span></span>').join('');
}

// ---- State ----------------------------------------------------------------------------------------------------

/** The state `n` clicks after `s`. */
function advance(s, n) {
  let { index, step } = s;
  for (let i = 0; i < n; i++) {
    if (step < stepsOf(index)) step += 1;
    else if (index < SLIDES.length - 1) [index, step] = [index + 1, 0];
  }
  return { ...s, index, step };
}

function render(s) {
  const slide = SLIDES[s.index];
  stage.dataset.slide = slide.id;
  stage.dataset.step = s.step;
  stage.dataset.layout = slide.layout;
  stage.dataset.tone = slide.tone ?? 'plain';
  stamp.dataset.place = slide.tone === 'brand' ? 'hero' : 'bar';
  for (const section of document.querySelectorAll('.slide')) {
    section.toggleAttribute('data-active', section.dataset.id === slide.id);
  }

  const slots = LAYOUTS[slide.layout];
  for (const frame of document.querySelectorAll('.frame')) {
    const rect = slots[frame.dataset.feed];
    frame.hidden = !rect;
    if (!rect) continue;
    Object.assign(frame.style, {
      left: `${rect.x - FRAME_PAD}px`,
      top: `${rect.y - FRAME_PAD}px`,
      width: `${rect.w + 2 * FRAME_PAD}px`,
      height: `${rect.h + 2 * FRAME_PAD}px`,
    });
  }

  document.querySelectorAll('.segment').forEach((segment, i) => {
    const fill = i < s.index ? 1 : i > s.index ? 0 : (s.step + 1) / (stepsOf(i) + 1);
    segment.toggleAttribute('data-current', i === s.index);
    segment.style.setProperty('--fill', fill);
  });

  tick(s);
}

function apply(next) {
  const before = state;
  state = next;
  const slideChanged = !before || before.index !== next.index;
  const stepChanged = before && before.step !== next.step;

  if (slideChanged) syncVideos(SLIDES[next.index].layout);

  if (animate && before && slideChanged) {
    // Into or out of the blue: the new background wipes up instead of crossfading through lavender.
    const wipe = (SLIDES[before.index].tone ?? 'plain') !== (SLIDES[next.index].tone ?? 'plain');
    document.documentElement.classList.toggle('wipe', wipe);
    const transition = document.startViewTransition(() => render(next));
    transition.finished.finally(() => document.documentElement.classList.remove('wipe'));
  } else {
    render(next);
  }

  // The upload on the core slide: one packet travels to the server as it appears.
  if (animate && stepChanged && SLIDES[next.index].id === 'core' && next.step === 2) {
    const packet = document.querySelector('.packet');
    packet.classList.add('is-moving');
    setTimeout(() => packet.querySelector('animateMotion').beginElement(), 450);
    setTimeout(() => packet.classList.remove('is-moving'), 1400);
  }
}

// ---- Recorded videos ------------------------------------------------------------------------------------------
// As in OBS (server.mjs syncVideos): from the start with sound on their own slide, from the start without sound in
// the overview, stopped everywhere else; once through, then the last frame stays.

function syncVideos(layout) {
  for (const video of document.querySelectorAll('.frame video')) {
    const key = video.closest('.frame').dataset.feed;
    delete video.dataset.wantsSound;
    if (!LAYOUTS[layout][key]) {
      video.pause();
      continue;
    }
    video.currentTime = 0;
    video.muted = layout !== key;
    // Without a click or key press in this window first, the browser refuses sound: then play it silently and
    // turn the sound on with the next key press here.
    video.play().catch(() => {
      video.muted = true;
      video.play().catch(() => {});
      video.dataset.wantsSound = String(layout === key);
    });
  }
}

function unlockSound() {
  for (const video of document.querySelectorAll('.frame video[data-wants-sound="true"]')) {
    video.muted = false;
    delete video.dataset.wantsSound;
  }
}

// ---- Stamp card -----------------------------------------------------------------------------------------------
// The same words as the stamp panel: "Stamped out", "Since 14:02 · <activity>".

const pad = (n) => String(n).padStart(2, '0');
const hhmm = (ms) => {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
};

function tick(s = state) {
  if (!s) return;
  const hero = stamp.dataset.place === 'hero';
  let elapsed = 0;
  let clock = 'idle';
  let status = 'Stamped out';
  if (s.stoppedElapsed !== null) {
    elapsed = s.stoppedElapsed;
    clock = 'stopped';
    status = `Stamped out at ${hhmm(s.startedAt + s.stoppedElapsed)}`;
  } else if (s.startedAt !== null) {
    elapsed = Date.now() + clockOffset - s.startedAt;
    clock = 'running';
    status = `Since ${hhmm(s.startedAt)}` + (hero && config.talk ? ` · ${config.talk}` : '');
  }
  const seconds = Math.max(0, Math.floor(elapsed / 1000));
  stampTimer.textContent = `${pad(Math.floor(seconds / 3600))}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}`;
  if (stampStatus.textContent !== status) stampStatus.textContent = status;
  stage.dataset.clock = clock;
}

// ---- Wiring ---------------------------------------------------------------------------------------------------

async function post(path, body) {
  await fetch(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
}

const KEYS = {
  ArrowRight: 'next',
  ArrowDown: 'next',
  PageDown: 'next',
  ' ': 'next',
  Enter: 'next',
  ArrowLeft: 'prev',
  ArrowUp: 'prev',
  PageUp: 'prev',
  Home: 'first',
};

async function main() {
  config = await (await fetch('/api/config')).json();
  buildStatic();
  fitStage();
  addEventListener('resize', fitStage);

  if (mode === 'browser' && offset === 0) {
    addEventListener('pointerdown', unlockSound);
    addEventListener('keydown', (event) => {
      unlockSound();
      if (event.repeat || event.ctrlKey || event.altKey || event.metaKey) return;
      const action = KEYS[event.key];
      if (action) {
        event.preventDefault();
        post('/api/go', { action });
      } else if (event.key === 's' || event.key === 'S') {
        post('/api/timer', { action: 'start' });
      } else if (event.key === 'f' || event.key === 'F') {
        document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen();
      }
    });
  }

  const events = new EventSource('/api/events');
  events.onmessage = (event) => {
    const s = JSON.parse(event.data);
    clockOffset = s.serverNow - Date.now();
    apply(advance(s, offset));
  };
  setInterval(() => tick(), 250);
}

main();
