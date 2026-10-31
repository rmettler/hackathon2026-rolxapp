# Presentation

The hackathon talk: an HTML deck in the Cudos design system that OBS composes with the live screens (the desktop
app shared from another PC, the Android app shared from the phone) and a recorded video of the terminal app, and
sends into the call as a virtual camera. Everybody uses their own RolX account on the real server, so whatever is
stamped during the demo is real.

```
Desktop PC ── VDO.Ninja screen share ──┐
Phone ─────── VDO.Ninja Android app ───┤
media/terminal.mp4 ────────────────────┼──► OBS ── Virtual Camera ──► Teams / Zoom
Deck (browser source) ─────────────────┘     ▲
                                             │ obs-websocket: the scene follows the slides
Presenter view (+ clicker) ── server.mjs ────┘
```

Slides: title with the talk's stamp card, the claim, "One core, every screen" (Terminal, Desktop, Android, RISC-V
around rolx-core, then the server), the desktop app, the phone, the terminal video, all three screens, the Rust
question, questions. The stamp card is drawn like the stamp panel of the apps: it is stamped in on the first
click and stamped out on the last slide.

## Files

- `server.mjs`: serves the deck and the presenter view, holds the slide state, switches the OBS scene.
  `node presentation/server.mjs setup-obs` creates or updates the OBS scenes and sources (and removes old ones).
- `web/slides.mjs`: the slides (order, clicks per slide, notes) and where each feed sits per layout.
- `web/deck.html`, `deck.css`, `deck.js`: the slides. The colours and type come from `rolx-ui/src/tokens.css`.
- `web/presenter.html`: current slide, next click, notes, timer, the share links.
- `config.json`: the talk's name, presenter names, and where each feed comes from (`vdo` stream id, `window`
  capture, or `video` file).
- `SHARING.md`: the steps for the people who share their screen. Send it to them.
- `media/`: the recorded terminal app video (git-ignored, put it there yourself). `terminal-voice.mp4` is the
  recording with the voice cleaned up and brought to -16 LUFS, `terminal-still.jpg` a frame of it
  (`ffmpeg -ss 30 -i terminal-voice.mp4 -frames:v 1 -q:v 2 terminal-still.jpg`):

  ```sh
  ffmpeg -i terminal-original.mp4 -map 0:v:0 -map 0:a:0 -c:v copy     -af "highpass=f=90,lowpass=f=9000,afftdn=nr=14:nf=-60:tn=1,acompressor=threshold=-38dB:ratio=3:attack=10:release=200:makeup=6,loudnorm=I=-16:TP=-1.5:LRA=9"     -c:a aac -b:a 192k -movflags +faststart terminal-voice.mp4
  ```

## One-time setup on the presenting laptop

1. Install OBS Studio and scrcpy: `winget install OBSProject.OBSStudio` and `winget install Genymobile.scrcpy`.
2. Install the Move transition plugin (smooth feed movement): unpack
   `move-transition-<version>-windows-programdata.zip` from github.com/exeldro/obs-move-transition/releases into
   `C:\ProgramData\obs-studio\plugins` and add a transition of type "Move" in OBS's Scene Transitions dock.
   `setup-obs` selects it (600 ms). Without it the scenes fade.
3. OBS > Tools > WebSocket Server Settings: enable the server, keep the password, and put it into
   `presentation/.obs-password` (git-ignored) or `OBS_PASSWORD`.
4. Put the names into `config.json` (`people`). Empty names show only the app.
5. Start the server and run `node presentation/server.mjs setup-obs` while OBS is open. Run it again after changing
   `config.json` or the layouts.
6. The KMR Apparat font: installed on the machine, or in `design-system/public/fonts` (`cargo xtask
   fetch-design-system`), or in `ROLX_FONT_DIR`. Otherwise the deck falls back to Helvetica Neue / Arial.

## Run

```sh
node presentation/server.mjs          # http://127.0.0.1:5180/presenter.html
```

Run it in your own terminal for the talk: it keeps the slide state in memory only.

- Presenter view: `http://127.0.0.1:5180/presenter.html`. Next: right arrow, space, page down (clickers send
  that). Back: left arrow, page up. `S` stamps the talk in early. "Share links" has the desktop link and the
  phone's stream id.
- Deck in a browser window without OBS: `http://127.0.0.1:5180/deck.html`, `F` for full screen. Same keys.
- OBS: Start Virtual Camera, then choose "OBS Virtual Camera" as the camera in Teams or Zoom. The live feeds are
  muted: everybody talks through the call.
- The terminal video is played by the deck itself, in OBS as in the browser: once from the start with sound on
  its slide, then its last frame stays. Everywhere else the still (`media/terminal-still.jpg`) shows. In OBS its
  sound comes with the deck source and plays on this laptop's speakers; the virtual camera carries no sound, so
  for people in the call share the computer sound in Teams ("Include computer sound"). The browser deck only plays
  sound after a click or key press in its window: press `F` for full screen there before the talk.
- Use either OBS or the browser deck for the talk, not both: each live screen and the video sound would run twice.

## Before the talk

- Everybody logged in on their device the same morning (the session lasts a day).
- Check what the screens show: activity names and the week view are real data.
- Both sharers test their share once (`SHARING.md`, "Test before the talk").
- Record each live part once as a fallback (OBS > Start Recording) and keep the files at hand.
- Rehearse twice, including the hand-overs. Delete rehearsal entries afterwards (undo or `entry delete`).
