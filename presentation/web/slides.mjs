// Slides and stage layouts, shared by the deck, the presenter view and the server (OBS scenes).
// Coordinates are stage pixels on the 1920 x 1080 canvas.

export const STAGE = { width: 1920, height: 1080 };

// Space the deck leaves around a live feed for its frame (the feed itself sits on top in OBS).
export const FRAME_PAD = 14;

// Where each feed goes, per layout: as large as the stage allows. A layout is one OBS scene; feeds missing from
// a layout are not in it. Screens are 16:9, the phone is a portrait 9:20.
export const LAYOUTS = {
  deck: {},
  desk: {
    desk: { x: 240, y: 138, w: 1440, h: 810 },
  },
  phone: {
    phone: { x: 1240, y: 46, w: 416, h: 920 },
  },
  terminal: {
    terminal: { x: 240, y: 138, w: 1440, h: 810 },
  },
  all: {
    phone: { x: 134, y: 74, w: 399, h: 880 },
    desk: { x: 609, y: 74, w: 708, h: 398 },
    terminal: { x: 609, y: 556, w: 708, h: 398 },
  },
};

// OBS scene per layout.
export const SCENES = {
  deck: 'RolX Deck',
  desk: 'RolX Desk',
  phone: 'RolX Phone',
  terminal: 'RolX Terminal',
  all: 'RolX All',
};

// Feeds: the OBS source name, who presents it (the `people` key in config.json) and what it shows.
export const FEEDS = {
  desk: { source: 'RolX desk', person: 'desk', label: 'Desktop app', icon: 'laptop' },
  phone: { source: 'RolX phone', person: 'phone', label: 'Android app', icon: 'smartphone' },
  terminal: { source: 'RolX terminal', person: 'terminal', label: 'Terminal app', icon: 'terminal' },
};

// `steps` is the number of extra clicks a slide takes before moving on. Notes show in the presenter view only.
export const SLIDES = [
  {
    id: 'title',
    layout: 'deck',
    tone: 'brand',
    notes: ['The talk is stamped in on the first click (or S).', 'Who we are, what we built.'],
  },
  {
    id: 'claim',
    layout: 'deck',
    notes: ['Time tracking happens between meetings, on the move, in the evening.', 'RolX only has the web app.'],
  },
  {
    id: 'core',
    layout: 'deck',
    steps: 2,
    notes: [
      'Click: the apps appear, down to RISC-V.',
      'Click: the server. Stamping works offline, the core uploads later.',
      'One Rust library under every app; Dioxus draws desktop and mobile.',
    ],
  },
  {
    id: 'desk',
    layout: 'desk',
    notes: ['Shared from the other PC.', 'Hotkey, command palette, tray. Fix an overlap in the day view.'],
  },
  {
    id: 'phone',
    layout: 'phone',
    notes: ['Shared from the phone.', 'Stamp in, switch activity, look at the day.'],
  },
  {
    id: 'terminal',
    layout: 'terminal',
    notes: ['The recorded terminal app video.'],
  },
  {
    id: 'all',
    layout: 'all',
    notes: ['Three screens, one core.'],
  },
  {
    id: 'rust',
    layout: 'deck',
    notes: ['What worked, what hurt.'],
  },
  {
    id: 'end',
    layout: 'deck',
    tone: 'brand',
    notes: ['Stamped out: the timer shows how long the talk took.'],
  },
];

export const stepsOf = (index) => SLIDES[index]?.steps ?? 0;
