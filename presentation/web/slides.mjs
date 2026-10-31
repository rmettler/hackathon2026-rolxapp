// Slides and stage layouts, shared by the deck, the presenter view and the server (OBS scenes).
// Coordinates are stage pixels on the 1920 x 1080 canvas.

export const STAGE = { width: 1920, height: 1080 };

// Space the deck leaves around a live feed for its frame (the feed itself sits on top in OBS).
export const FRAME_PAD = 14;

// Where each live feed goes, per layout. A layout is one OBS scene; feeds missing from a layout are not in it.
export const LAYOUTS = {
  deck: {},
  phone: {
    phone: { x: 1300, y: 84, w: 412, h: 876 },
  },
  desk: {
    desk: { x: 680, y: 130, w: 1120, h: 700 },
  },
  train: {
    trainScreen: { x: 680, y: 130, w: 1120, h: 700 },
    trainCamera: { x: 120, y: 605, w: 400, h: 225 },
  },
  // Bottoms on one line (840), captions below.
  all: {
    phone: { x: 134, y: 240, w: 282, h: 600 },
    desk: { x: 516, y: 384, w: 730, h: 456 },
    trainScreen: { x: 1346, y: 565, w: 440, h: 275 },
    trainCamera: { x: 1346, y: 362, w: 240, h: 135 },
  },
};

// OBS scene per layout.
export const SCENES = {
  deck: 'RolX Deck',
  phone: 'RolX Phone',
  desk: 'RolX Desk',
  train: 'RolX Train',
  all: 'RolX All',
};

// Feeds: the OBS source name and who presents it (the `people` key in config.json).
export const FEEDS = {
  phone: { source: 'RolX phone', person: 'phone', label: 'Android app' },
  desk: { source: 'RolX desk', person: 'desk', label: 'Desktop app' },
  trainScreen: { source: 'RolX train screen', person: 'train', label: 'Terminal' },
  trainCamera: { source: 'RolX train camera', person: 'train', label: 'Camera' },
};

// `steps` is the number of extra clicks a slide takes before moving on. Notes show in the presenter view only.
export const SLIDES = [
  {
    id: 'title',
    layout: 'deck',
    tone: 'brand',
    notes: ['Start the timer with S, or it starts on the first click.', 'Who we are, what we built.'],
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
      'Click: the apps appear.',
      'Click: the server. Stamping works offline, the core uploads later.',
      'All in Rust, one library under every app.',
    ],
  },
  {
    id: 'phone',
    layout: 'phone',
    notes: ['Stamp in, switch activity, look at the day.'],
  },
  {
    id: 'desk',
    layout: 'desk',
    notes: ['Hotkey, command palette, tray.', 'Fix an overlap in the day view.'],
  },
  {
    id: 'train',
    layout: 'train',
    steps: 2,
    notes: ['Click when the train is offline: stamping still works.', 'Click when the upload went through.'],
  },
  {
    id: 'all',
    layout: 'all',
    notes: ['Three accounts, three devices, one core.'],
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
    notes: ['The timer stops here: that is how long the talk took.'],
  },
];

export const stepsOf = (index) => SLIDES[index]?.steps ?? 0;
