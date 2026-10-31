# Presentation

The hackathon talk: an HTML deck in the Cudos design system that OBS composes with the live feeds (phone, desktop
app, the train's terminal and camera) and sends into the call as a virtual camera. Everybody uses their own RolX
account on the real server, so whatever is stamped during the demo is real: book the talk to the hackathon activity.

```
Phone (USB) ── scrcpy window ──┐
Desktop app window ────────────┤
Train: VDO.Ninja links ────────┼──► OBS ── Virtual Camera ──► Teams / Zoom
Deck (browser source) ─────────┘     ▲
                                     │ obs-websocket: the scene follows the slides
Presenter view (+ clicker) ── server.mjs
```

## Files

- `server.mjs`: serves the deck and the presenter view, holds the slide state, switches the OBS scene.
  `node presentation/server.mjs setup-obs` creates or updates the OBS scenes and sources.
- `web/slides.mjs`: the slides (order, clicks per slide, notes) and where each feed sits per layout.
- `web/deck.html`, `deck.css`, `deck.js`: the slides. The colours and type come from `rolx-ui/src/tokens.css`.
- `web/presenter.html`: current slide, next click, notes, timer, remote links.
- `config.json`: presenter names, the window titles OBS captures, the VDO.Ninja stream ids.

## One-time setup on the presenting laptop

1. Install OBS Studio and scrcpy: `winget install OBSProject.OBSStudio` and `winget install Genymobile.scrcpy`.
2. Install the Move transition plugin (smooth feed movement): download
   `move-transition-<version>-windows-programdata.zip` from github.com/exeldro/obs-move-transition/releases and unpack
   it into `C:\ProgramData\obs-studio\plugins`. In OBS, add a transition of type "Move" in the Scene Transitions
   dock. Without it the scenes fade.
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

- Presenter view: `http://127.0.0.1:5180/presenter.html`. Next: right arrow, space, page down (clickers send
  that). Back: left arrow, page up. `S` starts the timer (it also starts on the first click and stops on the last
  slide).
- Deck in a browser window without OBS: `http://127.0.0.1:5180/deck.html`, `F` for full screen. Same keys.
- OBS: Start Virtual Camera, then choose "OBS Virtual Camera" as the camera in Teams or Zoom. Mute the VDO.Ninja
  sources: everybody talks through the call.

### Feeds

- Phone, connected by USB (or the emulator):
  `scrcpy --window-title rolx-phone --no-audio --show-touches --stay-awake`. OBS captures the window titled
  `rolx-phone`.
- Desktop app: OBS captures the window of `rolxapp-dioxus.exe`. Start the app before the talk and size it to
  16:10 for the best fit.
- Train: the presenter view's "Remote links" has two VDO.Ninja links, one for the camera and one for the screen
  share. The person on the train opens both in a browser (no install) and shares the terminal window. The links
  carry no audio.

To present a feed from another laptop, change its entry in `config.json` to `{ "kind": "vdo", "id": "<unique id>",
"share": "screen" }` and run `setup-obs` again; its push link then shows in the presenter view.

## Before the talk

- Everybody logged in on their device the same morning (the session lasts a day).
- Check what the screens show: activity names and the week view are real data.
- Record each live part once as a fallback (OBS > Start Recording) and keep the files at hand.
- Rehearse twice, including the hand-overs. Delete rehearsal entries afterwards (undo or `entry delete`).
