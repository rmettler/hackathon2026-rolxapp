// Presentation server: serves the deck and the presenter view, keeps the shared slide state, and switches the
// OBS scene with the slides. No dependencies (Node 22+).
//
//   node presentation/server.mjs             serve on http://127.0.0.1:<port>
//   node presentation/server.mjs setup-obs   create or update the OBS scenes and sources, then exit
//
// The obs-websocket password (OBS > Tools > WebSocket Server Settings): OBS_PASSWORD, else presentation/.obs-password.

import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { createReadStream, existsSync, readFileSync } from 'node:fs';
import { dirname, extname, join, normalize, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import { Obs } from './obs.mjs';
import { FEEDS, LAYOUTS, SCENES, SLIDES, STAGE, stepsOf } from './web/slides.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const WEB = join(HERE, 'web');
const REPO = resolve(HERE, '..');
const TOKENS = join(REPO, 'rolx-ui', 'src', 'tokens.css');
const FONTS = process.env.ROLX_FONT_DIR ?? join(REPO, 'design-system', 'public', 'fonts');

const loadConfig = () => JSON.parse(readFileSync(join(HERE, 'config.json'), 'utf8'));
const config = loadConfig();
const PORT = Number(process.env.PORT ?? config.port ?? 5180);
const OBS_URL = process.env.OBS_URL ?? config.obsUrl ?? 'ws://127.0.0.1:4455';
const PASSWORD_FILE = join(HERE, '.obs-password');
const OBS_PASSWORD =
  process.env.OBS_PASSWORD ?? (existsSync(PASSWORD_FILE) ? readFileSync(PASSWORD_FILE, 'utf8').trim() : '');

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
};

/** VDO.Ninja links (docs.vdo.ninja, "How to screen share in 1080p"): `view` goes into OBS, `push` is what the
 * presenting PC opens. A phone shares with the VDO.Ninja Android app under the same stream id instead. */
function vdoLinks(feed) {
  return {
    view: `https://vdo.ninja/?view=${feed.id}&videobitrate=10000&scale=100&cleanoutput&noaudio`,
    push: `https://vdo.ninja/?push=${feed.id}&screenshare&quality=0&audiodevice=0`,
  };
}

function publicConfig() {
  const { people = {}, feeds = {}, talk = '' } = loadConfig();
  const links = Object.fromEntries(
    Object.entries(feeds)
      .filter(([, feed]) => feed.kind === 'vdo')
      .map(([key, feed]) => [key, { id: feed.id, device: feed.device ?? 'pc', ...vdoLinks(feed) }]),
  );
  // What the deck itself shows in each frame when it runs in a browser (in OBS, OBS puts the feeds on top).
  const sources = Object.fromEntries(
    Object.entries(feeds).map(([key, feed]) => {
      if (feed.kind === 'vdo') return [key, { kind: 'vdo', src: vdoLinks(feed).view }];
      if (feed.kind === 'video') return [key, { kind: 'video', src: `/media/${key}` }];
      return [key, { kind: feed.kind }];
    }),
  );
  return { people, talk, links, sources };
}

/** A video feed's file, with byte ranges so the browser can loop and seek. */
async function sendVideo(req, res, key) {
  const feed = loadConfig().feeds?.[key];
  if (feed?.kind !== 'video') return res.writeHead(404).end();
  const path = resolve(HERE, feed.file);
  let size;
  try {
    size = (await stat(path)).size;
  } catch {
    return res.writeHead(404).end();
  }
  const type = TYPES[extname(path)] ?? 'application/octet-stream';
  const range = /bytes=(\d*)-(\d*)/.exec(req.headers.range ?? '');
  if (!range) {
    res.writeHead(200, { 'Content-Type': type, 'Content-Length': size, 'Accept-Ranges': 'bytes' });
    return createReadStream(path).pipe(res);
  }
  const start = range[1] ? Number(range[1]) : Math.max(0, size - Number(range[2]));
  const end = range[1] && range[2] ? Math.min(Number(range[2]), size - 1) : size - 1;
  if (start >= size || start > end) return res.writeHead(416, { 'Content-Range': `bytes */${size}` }).end();
  res.writeHead(206, {
    'Content-Type': type,
    'Content-Length': end - start + 1,
    'Content-Range': `bytes ${start}-${end}/${size}`,
    'Accept-Ranges': 'bytes',
  });
  createReadStream(path, { start, end }).pipe(res);
}

// ---------------------------------------------------------------------------------------------------------------
// Slide state

const last = SLIDES.length - 1;
const state = { index: 0, step: 0, startedAt: null, stoppedElapsed: null };
const obsStatus = { connected: false, message: 'not connected' };
const clients = new Set();

const snapshot = () => ({ ...state, serverNow: Date.now(), obs: { ...obsStatus } });

function broadcast() {
  const data = `data: ${JSON.stringify(snapshot())}\n\n`;
  for (const res of clients) res.write(data);
}

function moveTo(index, step) {
  const now = Date.now();
  state.index = Math.max(0, Math.min(last, index));
  state.step = Math.max(0, Math.min(stepsOf(state.index), step));
  if (state.startedAt === null && state.index > 0) state.startedAt = now;
  if (state.index === last && state.startedAt !== null && state.stoppedElapsed === null) {
    state.stoppedElapsed = now - state.startedAt;
  } else if (state.index < last && state.stoppedElapsed !== null) {
    state.startedAt = now - state.stoppedElapsed;
    state.stoppedElapsed = null;
  }
  // Every click sets the scene, so OBS follows again after someone switched it by hand.
  showScene();
  broadcast();
}

function go(action, index) {
  if (action === 'next') {
    if (state.step < stepsOf(state.index)) moveTo(state.index, state.step + 1);
    else if (state.index < last) moveTo(state.index + 1, 0);
  } else if (action === 'prev') {
    if (state.step > 0) moveTo(state.index, state.step - 1);
    else if (state.index > 0) moveTo(state.index - 1, stepsOf(state.index - 1));
  } else if (action === 'goto') {
    moveTo(Number(index) || 0, 0);
  } else if (action === 'first') {
    moveTo(0, 0);
  }
}

function timer(action) {
  if (action === 'start' && state.startedAt === null) state.startedAt = Date.now();
  if (action === 'reset') {
    state.startedAt = null;
    state.stoppedElapsed = null;
  }
  broadcast();
}

// ---------------------------------------------------------------------------------------------------------------
// OBS: follow the slides with the matching scene, reconnecting quietly while OBS is closed.

const obs = new Obs();

function setObsStatus(connected, message) {
  if (obsStatus.connected === connected && obsStatus.message === message) return;
  obsStatus.connected = connected;
  obsStatus.message = message;
  broadcast();
}

async function connectObs() {
  try {
    await obs.connect(OBS_URL, OBS_PASSWORD);
    obs.onclose = () => {
      setObsStatus(false, 'disconnected');
      setTimeout(connectObs, 3000);
    };
    setObsStatus(true, 'connected');
    showScene();
  } catch (error) {
    setObsStatus(false, error.message);
    setTimeout(connectObs, 3000);
  }
}

function showScene() {
  if (!obs.connected) return;
  const sceneName = SCENES[SLIDES[state.index].layout];
  obs
    .call('SetCurrentProgramScene', { sceneName })
    .then(() => setObsStatus(true, 'connected'))
    .catch((error) => setObsStatus(true, `${error.message} (run setup-obs)`));
}

// ---------------------------------------------------------------------------------------------------------------
// HTTP

async function readBody(req) {
  let body = '';
  for await (const chunk of req) body += chunk;
  return body ? JSON.parse(body) : {};
}

async function sendFile(res, path) {
  try {
    if (!(await stat(path)).isFile()) throw new Error('not a file');
    res.writeHead(200, { 'Content-Type': TYPES[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(await readFile(path));
  } catch {
    res.writeHead(404).end();
  }
}

/** `path` below `root`, or null for anything that tries to leave it. */
function inside(root, path) {
  const full = normalize(join(root, decodeURIComponent(path)));
  return full.startsWith(root + sep) ? full : null;
}

const json = (res, value) => {
  res.writeHead(200, { 'Content-Type': TYPES['.json'], 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(value));
};

async function handle(req, res) {
  const url = new URL(req.url, 'http://localhost');
  const path = url.pathname;

  if (path === '/') return res.writeHead(302, { Location: '/presenter.html' }).end();
  if (path === '/api/state') return json(res, snapshot());
  if (path === '/api/config') return json(res, publicConfig());
  if (path === '/api/events') {
    res.writeHead(200, { 'Content-Type': 'text/event-stream', 'Cache-Control': 'no-store', Connection: 'keep-alive' });
    res.write(`retry: 1000\ndata: ${JSON.stringify(snapshot())}\n\n`);
    clients.add(res);
    req.on('close', () => clients.delete(res));
    return;
  }
  if (req.method === 'POST' && path === '/api/go') {
    const { action, index } = await readBody(req);
    go(action, index);
    return json(res, snapshot());
  }
  if (req.method === 'POST' && path === '/api/timer') {
    timer((await readBody(req)).action);
    return json(res, snapshot());
  }
  if (path.startsWith('/media/')) return sendVideo(req, res, decodeURIComponent(path.slice('/media/'.length)));
  if (path === '/tokens.css') return sendFile(res, TOKENS);
  if (path.startsWith('/fonts/')) {
    const file = inside(resolve(FONTS), path.slice('/fonts/'.length));
    return file ? sendFile(res, file) : res.writeHead(404).end();
  }
  const file = inside(WEB, path.slice(1));
  return file ? sendFile(res, file) : res.writeHead(404).end();
}

function serve() {
  const server = createServer((req, res) => {
    handle(req, res).catch((error) => {
      console.error(error);
      if (!res.headersSent) res.writeHead(500);
      res.end();
    });
  });
  server.listen(PORT, '127.0.0.1', () => {
    console.log(`Presenter view  http://127.0.0.1:${PORT}/presenter.html`);
    console.log(`Deck            http://127.0.0.1:${PORT}/deck.html`);
    console.log(`OBS             ${OBS_URL}${OBS_PASSWORD ? ' (with password)' : ''}`);
  });
  // Keep event streams alive through idle proxies and sleeping laptops.
  setInterval(() => {
    for (const res of clients) res.write(': ping\n\n');
  }, 15000);
  connectObs();
}

// ---------------------------------------------------------------------------------------------------------------
// setup-obs: scenes, sources and transition, idempotent.

const PRIORITY = { class: 0, title: 1, exe: 2 };
const MOVE_MS = 600;

function inputFor(key) {
  const feed = loadConfig().feeds[key];
  if (feed.kind === 'video') {
    return {
      inputKind: 'ffmpeg_source',
      inputSettings: {
        is_local_file: true,
        local_file: resolve(HERE, feed.file),
        looping: true,
        restart_on_activate: true,
        close_when_inactive: false,
        hw_decode: true,
      },
    };
  }
  if (feed.kind === 'window') {
    return {
      inputKind: 'window_capture',
      inputSettings: {
        window: feed.window,
        priority: PRIORITY[feed.match] ?? PRIORITY.title,
        method: 2, // Windows Graphics Capture: works for WebView2 and SDL windows
        cursor: false,
        client_area: true,
      },
    };
  }
  // A portrait browser source for a phone, so its screen is not letterboxed inside the source.
  const [width, height] = feed.device === 'android' ? [1080, 2400] : [1920, 1080];
  return {
    inputKind: 'browser_source',
    inputSettings: { url: vdoLinks(feed).view, width, height, reroute_audio: true, shutdown: false },
  };
}

async function setupObs() {
  await obs.connect(OBS_URL, OBS_PASSWORD);
  const log = (text) => console.log(`  ${text}`);

  const video = await obs.call('GetVideoSettings');
  if (video.baseWidth !== STAGE.width || video.baseHeight !== STAGE.height) {
    await obs
      .call('SetVideoSettings', {
        baseWidth: STAGE.width,
        baseHeight: STAGE.height,
        outputWidth: STAGE.width,
        outputHeight: STAGE.height,
      })
      .then(() => log('canvas set to 1920 x 1080'))
      .catch((error) => log(`could not set the canvas to 1920 x 1080: ${error.message}`));
  }

  const scenes = new Set((await obs.call('GetSceneList')).scenes.map((scene) => scene.sceneName));
  const wantedScenes = new Set(Object.values(SCENES));
  for (const sceneName of scenes) {
    if (sceneName.startsWith('RolX ') && !wantedScenes.has(sceneName)) {
      await obs.call('SetCurrentProgramScene', { sceneName: SCENES.deck }).catch(() => {});
      await obs.call('RemoveScene', { sceneName }).catch((error) => log(`could not remove ${sceneName}: ${error.message}`));
      log(`removed scene ${sceneName}`);
    }
  }
  for (const sceneName of Object.values(SCENES)) {
    if (!scenes.has(sceneName)) {
      await obs.call('CreateScene', { sceneName });
      log(`scene ${sceneName}`);
    }
  }

  const inputs = new Set((await obs.call('GetInputList')).inputs.map((input) => input.inputName));
  const deckName = 'RolX deck';
  const deckSettings = {
    url: `http://127.0.0.1:${PORT}/deck.html?mode=obs`,
    width: STAGE.width,
    height: STAGE.height,
    shutdown: false,
    restart_when_active: false,
  };
  const wanted = [
    { inputName: deckName, inputKind: 'browser_source', inputSettings: deckSettings },
    ...Object.entries(FEEDS).map(([key, feed]) => ({ inputName: feed.source, ...inputFor(key) })),
  ];
  // Sources of an earlier layout (or a feed that changed kind) go. OBS frees a removed source only once nothing
  // holds it any more, so it is renamed first to free its name for the new one.
  const wantedKinds = new Map(wanted.map((input) => [input.inputName, input.inputKind]));
  for (const { inputName, inputKind } of (await obs.call('GetInputList')).inputs) {
    if (inputName.startsWith('RolX ') && wantedKinds.get(inputName) !== inputKind) {
      const retired = `${inputName} (removed ${Date.now()})`;
      await obs.call('SetInputName', { inputName, newInputName: retired });
      await obs.call('RemoveInput', { inputName: retired });
      inputs.delete(inputName);
      log(`removed ${inputName}`);
    }
  }
  for (const input of wanted) {
    if (inputs.has(input.inputName)) {
      await obs.call('SetInputSettings', { inputName: input.inputName, inputSettings: input.inputSettings, overlay: true });
    } else {
      await obs.call('CreateInput', { sceneName: SCENES.deck, ...input, sceneItemEnabled: true });
      log(`source ${input.inputName}`);
    }
    // Everybody talks through the call; the feeds stay silent.
    if (input.inputKind !== 'window_capture') {
      await obs.call('SetInputMute', { inputName: input.inputName, inputMuted: true }).catch(() => {});
    }
  }

  const ours = new Set([deckName, ...Object.values(FEEDS).map((feed) => feed.source)]);
  for (const [layout, sceneName] of Object.entries(SCENES)) {
    const slots = LAYOUTS[layout];
    const wantedHere = new Set([deckName, ...Object.keys(slots).map((key) => FEEDS[key].source)]);
    const items = (await obs.call('GetSceneItemList', { sceneName })).sceneItems;
    const idOf = new Map(items.map((item) => [item.sourceName, item.sceneItemId]));

    for (const item of items) {
      if (ours.has(item.sourceName) && !wantedHere.has(item.sourceName)) {
        await obs.call('RemoveSceneItem', { sceneName, sceneItemId: item.sceneItemId });
      }
    }
    const ensure = async (sourceName) =>
      idOf.get(sourceName) ?? (await obs.call('CreateSceneItem', { sceneName, sourceName })).sceneItemId;

    const deckId = await ensure(deckName);
    await obs.call('SetSceneItemIndex', { sceneName, sceneItemId: deckId, sceneItemIndex: 0 });
    await obs.call('SetSceneItemTransform', {
      sceneName,
      sceneItemId: deckId,
      sceneItemTransform: {
        positionX: 0,
        positionY: 0,
        alignment: 5,
        boundsType: 'OBS_BOUNDS_STRETCH',
        boundsWidth: STAGE.width,
        boundsHeight: STAGE.height,
      },
    });
    for (const [key, rect] of Object.entries(slots)) {
      const sceneItemId = await ensure(FEEDS[key].source);
      await obs.call('SetSceneItemTransform', {
        sceneName,
        sceneItemId,
        sceneItemTransform: {
          positionX: rect.x,
          positionY: rect.y,
          alignment: 5,
          boundsType: 'OBS_BOUNDS_SCALE_INNER',
          boundsAlignment: 0,
          boundsWidth: rect.w,
          boundsHeight: rect.h,
        },
      });
      await obs.call('SetSceneItemEnabled', { sceneName, sceneItemId, sceneItemEnabled: true });
    }
    log(`layout ${sceneName}`);
  }

  const { transitions } = await obs.call('GetSceneTransitionList');
  const move = transitions.find((transition) => transition.transitionKind === 'move_transition');
  if (move) {
    await obs.call('SetCurrentSceneTransition', { transitionName: move.transitionName });
    await obs.call('SetCurrentSceneTransitionDuration', { transitionDuration: MOVE_MS });
    log(`transition ${move.transitionName}, ${MOVE_MS} ms`);
  } else {
    const fade = transitions.find((transition) => transition.transitionKind === 'fade_transition');
    if (fade) await obs.call('SetCurrentSceneTransition', { transitionName: fade.transitionName });
    log('no Move transition found: install the Move plugin and add a "Move" transition, then run setup-obs again');
  }

  // The scene stays as it is: the running server sets it with the next click.
  obs.close();
  console.log('OBS is set up.');
}

if (process.argv[2] === 'setup-obs') {
  setupObs().catch((error) => {
    console.error(`setup-obs failed: ${error.message}`);
    process.exit(1);
  });
} else {
  serve();
}
