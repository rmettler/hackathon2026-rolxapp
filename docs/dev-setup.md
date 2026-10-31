# Development setup

## Mobile app (Android)

The Dioxus app in `rolx-mobile/` is run on Android via `cargo xtask android`, which wraps the
Android SDK tools and `dx serve`. A first-time setup takes about 30 minutes, mostly downloads.

### 1. Install the tools

- **Rust** via [rustup](https://rustup.rs), plus the Android targets. `x86_64` is used by the
  emulator, `aarch64` by phones (and the emulator on Apple Silicon):

  ```sh
  rustup target add x86_64-linux-android aarch64-linux-android
  ```

- **Dioxus CLI 0.7** (`dx`). Stay on 0.7.x, the app uses Dioxus 0.7 and the xtask warns otherwise:

  ```sh
  cargo install cargo-binstall   # if you don't have it yet
  cargo binstall dioxus-cli@0.7.10
  ```

- **[Android Studio](https://developer.android.com/studio)**, for the SDK and the emulator.
- **JDK 17 or newer** for Gradle. Android Studio's bundled JBR is enough; a separate JDK works too.

### 2. Install the SDK components

In Android Studio open *Settings > Languages & Frameworks > Android SDK* (or *More Actions > SDK
Manager* on the welcome screen):

- *SDK Platforms*: **Android 14 (API 34)**, the version dx compiles against.
- *SDK Tools* (tick *Show Package Details* to pick versions):
  - **NDK (Side by side) 27.x**, the team's reference version
  - Android SDK Build-Tools
  - Android SDK Platform-Tools (`adb`)
  - Android Emulator
  - CMake

### 3. Set the environment variables

| Variable       | Value                                                     | Needed?                                                             |
| -------------- | --------------------------------------------------------- | ------------------------------------------------------------------- |
| `ANDROID_HOME` | the SDK directory (see below)                             | **Yes.** The xtask finds the default location, but `dx` does not.   |
| `NDK_HOME`     | `$ANDROID_HOME/ndk/<version>`                             | Optional. Without it `dx` uses the newest NDK under `ndk/`.         |
| `JAVA_HOME`    | a JDK 17+                                                 | Optional on Windows/macOS if Android Studio is installed in its default location (its JBR is used). Required on Linux. |

Default SDK locations: Windows `%LOCALAPPDATA%\Android\Sdk`, macOS `~/Library/Android/sdk`,
Linux `~/Android/Sdk`. Adding `$ANDROID_HOME/platform-tools` to `PATH` lets you call `adb`
directly, which is handy for debugging but not required.

**Windows** (PowerShell, then open a new terminal):

```powershell
[Environment]::SetEnvironmentVariable('ANDROID_HOME', "$env:LOCALAPPDATA\Android\Sdk", 'User')
# optional: pin the NDK (adjust the version to what's installed)
[Environment]::SetEnvironmentVariable('NDK_HOME', "$env:LOCALAPPDATA\Android\Sdk\ndk\27.0.12077973", 'User')
# optional: use Android Studio's JDK
[Environment]::SetEnvironmentVariable('JAVA_HOME', 'C:\Program Files\Android\Android Studio\jbr', 'User')
```

**macOS / Linux** (in `~/.zshrc` or `~/.bashrc`):

```sh
export ANDROID_HOME="$HOME/Library/Android/sdk"   # Linux: $HOME/Android/Sdk
export NDK_HOME="$ANDROID_HOME/ndk/27.0.12077973" # optional, adjust the version
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"  # Linux: your JDK
export PATH="$PATH:$ANDROID_HOME/platform-tools"
```

### 4. Create an emulator

In Android Studio open *Device Manager > Create Virtual Device*, pick a phone (e.g. Pixel 8) and
a system image with API 34 or newer. Use an **x86_64** image on Intel/AMD machines and an
**arm64-v8a** image on Apple Silicon.

The emulator needs hardware virtualization: on Windows enable *Windows Hypervisor Platform*
(*Turn Windows features on or off*), on Linux make sure KVM is available (`/dev/kvm`).

### 5. Check the setup

```sh
dx --version                       # dioxus 0.7.x
java -version                      # 17 or newer
rustup target list --installed     # includes the *-linux-android targets
cargo xtask android devices        # lists your AVD
```

### 6. Run the app

```sh
cargo xtask android devices              # list emulators (AVDs) and connected devices
cargo xtask android emulator             # boot the first AVD and wait until it is ready
cargo xtask android emulator --avd <AVD> --cold   # boot a specific AVD without snapshot
cargo xtask android run                  # run on the emulator, booting one if needed
cargo xtask android run --device         # run on the only connected phone
cargo xtask android run --device <SERIAL>         # run on a specific phone (see `devices`)
cargo xtask android run -- --release     # pass extra arguments to `dx serve`
```

`run` uses `dx serve`, so changes are hot-reloaded while it runs. The first build is slow:
`dx` downloads a nightly Rust toolchain (used only to plan the build), Gradle downloads its
dependencies, and ~490 crates are compiled for Android. Expect 15–30 minutes on a slow connection;
later builds take seconds.

**Config.** A phone has no `config.toml` to edit, so the build embeds one into the app: the file
at `ROLX_CONFIG` if set, else your CLI/desktop config (`cargo run -- config path`). Without one
the build warns and the app shows an error. The embedded config, including the Google client
secrets, ends up in the APK, so don't distribute builds outside the team.

**Login.** *Log in with Google* opens the phone's browser; after the Google login the browser
redirects to the app on `127.0.0.1` and shows *Back to RolX*, which opens the app via the
`ch.cudos.rolxmobile` URL scheme. Android blocks the network of apps in the background, so the app
finishes the login once it is back in the foreground. *Log in with a code* uses the
device flow (`google_device_client`) instead: open the shown link and enter the code. The session
is stored in the app's private files dir and survives restarts.

**Logs** of the app appear in logcat under the tag `RustStdoutStderr`
(`adb logcat -s RustStdoutStderr`).

### 7. Run on a phone

1. Enable *Developer options*: *Settings > About phone*, tap *Build number* seven times.
2. In *Developer options* enable **USB debugging**.
3. Connect the phone via USB, unlock it and accept the *Allow USB debugging* prompt (tick
   *Always allow from this computer*).
4. `cargo xtask android devices` should list it as `device` / `phone`, then run
   `cargo xtask android run --device`.

Wireless instead of USB (Android 11+): enable *Wireless debugging* in *Developer options*, tap
*Pair device with pairing code* and run `adb pair <ip>:<pairing-port>`, then
`adb connect <ip>:<port>` with the port shown on the *Wireless debugging* screen. Run with the
serial shown by `devices`.

### 8. Build an APK to share

```sh
$env:ROLX_KEYSTORE_PASSWORD = '<password>'   # macOS / Linux: export ROLX_KEYSTORE_PASSWORD=...
cargo xtask android apk                      # writes target/apk/rolx-mobile-<version>.apk
```

This builds a signed release APK for arm64 phones (`--target` picks another Rust target). It
installs without a dev setup: send the file to the phone, open it and allow installing from that
source. The emulator on Intel/AMD machines needs `--target x86_64-linux-android`.

The APK is signed with the keystore `rolx-mobile/release.jks` (ignored by git), created with
`ROLX_KEYSTORE_PASSWORD` on the first run. **Keep the keystore and its password**: Android only
installs an update over an existing app if it is signed with the same key, otherwise the app must
be uninstalled first, which deletes its session and local stamps. To build on another machine,
copy the keystore there.

While building, the signing config (including the password) is added to `rolx-mobile/Dioxus.toml`
and removed afterwards; if a build is killed, the next one cleans it up, but don't commit it.

As noted under *Config* above, the APK contains the embedded config with the Google client
secrets: share it within the team only.

### Native features

`rolx-mobile/src/native.rs` and `src/links.rs` add what a phone app needs beyond the web view; the
JNI calls are in `src/android.rs`, and on other targets everything is a no-op. The only Java is
`StampLinkActivity` in the `android/plugin` Gradle module (with the icons), as dx generates the
app's own activity:

- **Stamped-in notification.** `use_native_stamp_sync(app.stamp_state)` (called once in the root
  component) keeps an ongoing, silent notification in line with the stamp state: "Stamped in since
  08:12" with the activity and a timer the system keeps running without the app; "On break since …"
  without a timer; removed when stamped out. It also runs at start, so it is back after the app was
  killed. Tapping it opens the app. Channel "Stamped in" (low importance), icon
  `ic_rolx_notification` in the `plugin` module. Its buttons "Break" / "End break" and "Stamp
  out" are stamp links (`break-start`, `break-end`, `out`, so a stale notification can't undo a
  change); they open the app briefly.
- **Live Update** (Android 16 QPR2+, i.e. API 36.1; detected by `EXTRA_REQUEST_PROMOTED_ONGOING`,
  as `SDK_INT` is 36 for every Android 16): the notification asks to be promoted, so the status
  bar shows a chip with the running timer ("Break" on a break), and HyperOS 3.1+ shows it in its
  island. Needs `POST_PROMOTED_NOTIFICATIONS` (manifest, granted at install); the user can turn it
  off per app, the settings sheet shows "Timer in the status bar" with a link to that page. On
  plain Android 16 (API 36.0, e.g. the current emulator images) nothing changes.
- **Notification permission** (Android 13+, `POST_NOTIFICATIONS`): asked once per app start, at the
  first stamp-in (or at start when stamped in). Without it nothing is posted; once granted (within
  two minutes) the notification appears within a second.
- **Vibration**: a short pulse on stamp-in, a double pulse on stamp-out (not at start).
- **Share sheet**: `share_text(subject, text)` opens Android's chooser; `day_share` and
  `week_share` build the texts for a day and a week.
- **Stamp links** `rolx://stamp?action=<action>&activity=<activity>` stamp from outside the app: a
  scanned QR code, Tasker, a routine that opens a link. Actions: `in` (default; with the activity,
  else the one used last; when stamped in it switches to the activity or ends the break), `out`,
  `break` (start or end), `break-start`, `break-end`, `toggle`. `activity` is an id, a full number
  (`%231001.001.01`) or words matching one activity (`Meetings`). `StampLinkActivity` queues the
  link in `<files dir>/stamp-links.txt` and opens the app, which applies it like a tap (refused
  while a forgotten session waits; links older than two minutes are dropped) and shows the stamp
  tab. Any app can send these links, as Tasker and QR scanners need to.
- **App shortcuts** (long press on the icon, Android 7.1+): "Stamp in: <activity>" for the first
  two favourites or recent activities (as the quick start tiles; "Stamp in" without any), "Stamp
  out" and "Break". They are stamp links, set at start and when the list changes, so they appear
  after the first start; pinned to the home screen they keep working.

To check them on an emulator, besides looking at the shade:

```sh
adb shell am start -a android.intent.action.VIEW -d "'rolx://stamp?action=in&activity=Meetings'"   # a stamp link
adb shell dumpsys shortcut | grep -A3 "ShortcutInfo {id=" | grep -o "id=[^,]*\|shortLabel=[^,]*"  # the shortcuts
adb shell dumpsys notification --noredact | grep -A40 pkg=ch.cudos.RolxMobile   # title, text, showChronometer, when
adb shell dumpsys vibrator_manager | grep -A8 "Recent vibrations"
adb shell cmd statusbar expand-notifications                                 # open the shade
adb shell pm revoke ch.cudos.RolxMobile android.permission.POST_NOTIFICATIONS   # kills the app
adb shell pm clear-permission-flags ch.cudos.RolxMobile android.permission.POST_NOTIFICATIONS user-set user-fixed  # ask again
```

Android 14+ lets the user swipe an ongoing notification away; it comes back with the next stamp
action or app start. A force-stop removes it too, until the app is started again.

### Troubleshooting

| Problem                                                    | Fix                                                                                                                 |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `Android SDK not found`                                    | Set `ANDROID_HOME` (step 3) and open a new terminal.                                                                 |
| `dx` complains about a missing NDK or linker               | Install the NDK (step 2); set `NDK_HOME` if it still isn't found.                                                    |
| `no AVD found`                                             | Create one (step 4).                                                                                                 |
| Emulator `did not finish booting within 180s`              | Retry, the first (cold) boot is slow. Check virtualization (step 4). `emulator --cold` helps with a broken snapshot. |
| Phone is `unauthorized`                                    | Unlock it and accept the USB debugging prompt. If no prompt appears, *Revoke USB debugging authorizations* and replug. |
| Phone not listed at all                                    | Use a data-capable cable, set USB mode to *File transfer*, on Windows install the vendor's USB driver.              |
| `several phones connected`                                 | Pass the serial: `run --device <SERIAL>`.                                                                           |
| Gradle fails with a Java version error                     | Point `JAVA_HOME` at a JDK 17+.                                                                                     |
| `rustc` error about a missing `*-linux-android` target     | `rustup target add` it (step 1).                                                                                    |
| Gradle `mergeDebugResources` fails with `values.xml` "path not found" (Windows) | Usually transient (a file lock, e.g. antivirus scanning): just run again. If it recurs, exclude `target\`, `%USERPROFILE%\.gradle` and `%USERPROFILE%\.cargo` from Defender real-time scanning (admin: `Add-MpPreference -ExclusionPath <dir>`), which also speeds up builds. |

## Testing against the fake server

`cargo xtask fake-server` runs an in-memory stand-in for the RolX server, so the CLI and both apps
can be tried end to end, including all write paths, without touching real data:

```sh
cargo xtask fake-server                      # http://127.0.0.1:5080, stop with Ctrl-C
cargo xtask fake-server --port 5081 --bind 0.0.0.0   # e.g. for a phone in the same Wi-Fi
```

It implements what `rolx-core` calls: `SignIn` (`info`, `POST`, `extend`), `WorkRecord`
(`range` and `PUT`), `edit-lock`, `Balance/{date}`, `Activity` (list, `suitable`, by id) and
`activity/favourite` (`GET`, `PUT`, `DELETE`), with the shapes of the real server. Any bearer token and any user id are
accepted, and all users share the same data. Writes are checked like the real server does it
(unknown or closed activity, `begin + duration + pause` beyond the day, overlapping entries, days
before the edit lock) and rejected with a `400` and the server's validation message.

The data is seeded relative to the day it starts and lost when it stops:

- 12 activities in 4 customers, favourites `#1001.001.01`, `#2042.001.02` and `#0001.001.01`;
  `#3107.001.03` closed last Friday, so it is only listed because it was booked last week.
- Entries for last week and the current week up to today (today only the morning), one of them
  without a time of day. 8:24 nominal time on weekdays, 0 on weekends.
- The edit lock at the Monday of last week.
- A balance of 12:00 flextime before last week; from then on the flextime follows what is booked
  against the nominal time, like on the real server (+3:30 on a Friday with the seeded entries).
  12.5 days of vacation left, 2 of them planned.

Every request is logged as one line (time, method, path, status, and the reason of a rejection).
`GET http://127.0.0.1:5080/` shows what the server holds, to check what an app changed.
On Windows the running server locks `target\debug\xtask.exe`, so stop it before changing the xtask.

`cargo xtask fake-session --out <path>` writes a `session.json` of the fake server's user (Fiona
Fake), valid until 2099 so the apps never try to extend it. It refuses to overwrite a file unless
`--force` is given, and then backs the old file up to the temp dir first.

### CLI

```powershell
cargo xtask fake-server                       # in a second terminal
cargo build                                   # clippy/test don't rebuild target\debug\rolxapp.exe
$env:ROLX_SERVER = 'http://127.0.0.1:5080'
$env:ROLX_DATA_DIR = "$env:TEMP\rolx-fake-data"   # stamps, activity cache and undo stay out of your real ones
cargo run -- records
cargo run -- activities --refresh
cargo run -- stamp in 3107.001.01; cargo run -- stamp out; cargo run -- sync
cargo run -- entry add -a 2042.001.02 --begin 13:00 --end 15:30
cargo run -- entry undo
```

The CLI and the desktop app always use the session in `%LOCALAPPDATA%\rolx\session.json`, your
**real** login; there is no override for it. Against the fake server that is harmless as long as it
is not rewritten, but it can be:

- Within 10 minutes of its expiry the apps extend it, and the fake server answers with a fake
  token that is then saved over your real one.
- `login` against the fake server stores the fake user's session; `logout` deletes the real one.

So copy `%LOCALAPPDATA%\rolx\session.json` somewhere outside the repo before a test run, compare
it afterwards (`Get-FileHash`) and put the copy back if it changed. Check the override first, as
`AGENTS.md` describes: with `ROLX_SERVER` at a closed port, `records` must fail with a connection
error.

### Desktop app

Same environment as the CLI (`ROLX_SERVER`, `ROLX_DATA_DIR`) and the same session caveat. WebView2
keeps one data folder per exe name, so to not mix with a desktop app you use for real, run a copy
under another name:

```powershell
cargo build -p rolxapp-dioxus
Copy-Item target\debug\rolxapp-dioxus.exe target\debug\rolxapp-dioxus-fake.exe
$env:ROLX_SERVER = 'http://127.0.0.1:5080'; $env:ROLX_DATA_DIR = "$env:TEMP\rolx-fake-data"
target\debug\rolxapp-dioxus-fake.exe
```

To drive it from a script, add `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`
and use the DevTools protocol; `ROLX_IDLE_SECS` and `ROLX_FAKE_IDLE_FILE` fake the away detection
(see `AGENTS.md`).

### Android

```sh
cargo xtask fake-server                      # in a second terminal
cargo xtask android run --fake-server        # or --fake-server <PORT>
cargo xtask android push-session             # once the app is installed; then restart the app
```

`run --fake-server` embeds `target/fake-server/rolx-mobile-config.toml` (via `ROLX_CONFIG`, see
*Config* above) with `server_url = "http://127.0.0.1:5080"` and forwards that port from the device
to this machine with `adb reverse`. This works on the emulator and on a USB phone, and needs no
change to the app: the network security config dx generates allows plain HTTP to `127.0.0.1` only.
The forward is lost when the emulator or adb restarts; `adb reverse tcp:5080 tcp:5080` or another
`run --fake-server` restores it. To use a config of your own instead, set `ROLX_CONFIG` and run
without `--fake-server`.

With several emulators running, name the one to use everywhere, e.g.
`cargo xtask android run --avd <AVD> --fake-server 5081 -- --port 8091 --interactive=false` (`--port` keeps `dx` off
the dev-server port of other runs) and `cargo xtask android push-session --device emulator-5556`. If `dx` keeps
"Waiting for android emulator to be ready" after the build, install
`target/dx/rolx-mobile/debug/android/app/app/build/outputs/apk/debug/app-debug.apk` with
`adb -s <serial> install -r` and start the app.

To try the forgotten-stamp banner, write a `stamps.json` with a session that started 13 hours ago into the app
while it is in the background (Git Bash: `adb exec-in "run-as ch.cudos.RolxMobile sh -c 'cat > files/stamps.json'" < stamps.json`)
and bring the app back: it reloads the stamps when it comes to the foreground. Airplane mode shows "Offline";
note that `adb reverse` keeps the fake server reachable even then.

`push-session` writes the fake session into the app's files dir with `adb shell run-as`, which
works for debug builds. A different session that is already there (a real login) is backed up to
`%TEMP%\rolx-mobile-session-backup-<time>.json` first; the command prints the path. To get the
real login back, log in again or push the backup (in Git Bash):
`adb exec-in "run-as ch.cudos.RolxMobile sh -c 'cat > files/session.json'" < <backup>`.

The next `run` without `--fake-server` embeds your real config again. `cargo xtask android apk`
refuses to build when the config to embed points at a local or plain-HTTP server, so a shared APK
can't end up talking to the fake server.
