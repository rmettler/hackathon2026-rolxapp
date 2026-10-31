# Sharing your screen into the presentation

Two people share their screen from their own device. The presenting laptop picks the screens up in OBS and puts
them on the slides. You talk through the call as usual: these links send **no sound**.

Keep the stream ids to the team. Anyone who has one can watch that screen while it is shared. New ids:
change them in `config.json` and run `node presentation/server.mjs setup-obs` again.

## Desktop app, from another PC

1. Start the RolX desktop app and log in. Use a 16:9 screen (1920 x 1080 is ideal). Make the window large.
2. Turn on Do Not Disturb (Windows: Focus), so no notifications show up on the shared screen.
3. Open this link in Chrome or Edge:

   https://vdo.ninja/?push=rolx-desk-ba66a5a8&screenshare&quality=0&audiodevice=0

4. Choose what to share:
   - **Window**: the RolX window. Only the app is visible.
   - **Entire screen**: smoother according to VDO.Ninja, but the audience sees everything on that screen.
5. Click **Share** and leave the tab open until the end of the talk. Do not minimise the RolX window.

## Android app, from the phone

1. Install **VDO.Ninja** from Google Play: https://play.google.com/store/apps/details?id=flutter.vdo.ninja
2. Open it and enter this **Stream ID**:

   rolx-phone-91d41405

3. Choose **SCREEN** as the source. Leave "Capture system audio" off.
4. Tap connect and allow Android's screen capture prompt.
5. Turn on Do Not Disturb, open the RolX app and hold the phone upright.

If the phone is in the room instead, plug it into the presenting laptop by USB and run
`scrcpy --window-title rolx-phone --no-audio --show-touches --stay-awake` there. Then set the phone feed in
`config.json` to `{ "kind": "window", "window": "rolx-phone:SDL_app:scrcpy.exe", "match": "title" }` and run
`setup-obs` again. The picture is sharper that way.

## Test before the talk

Both of you share as above while the presenter steps to the Desk, Phone and "One core, three screens" slides and
watches OBS: your screen shows inside the white frame.
If a frame stays dark: refresh the source in OBS (right-click the source, Properties, "Refresh cache of current
page"), then check that the stream id matches.
