# rolxapp-android

The RolX Android app: Dioxus `mobile`, drawn by the system WebView. It signs in with the device flow
only; the verification page opens in the system browser, never inside the app.

## Toolchain

- Rust targets `aarch64-linux-android` and `x86_64-linux-android` (`rust-toolchain.toml`).
- `dx` 0.8.0-alpha.3.
- Android SDK with NDK and command-line tools, and a JDK. Android Studio's bundled JBR works.
- Environment variables: `ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME`.

## Settings

An Android app has no config file to edit, so settings are compiled in. Each comes from the
environment, else from the gitignored `apps/android/config.local.toml`:

| `config.local.toml` | Environment | |
|---|---|---|
| `server_url` | `ROLX_SERVER_URL` | can be changed in the app's Settings |
| `google_device_client_id` | `ROLX_GOOGLE_DEVICE_CLIENT_ID` | |
| `google_device_client_secret` | `ROLX_GOOGLE_DEVICE_CLIENT_SECRET` | ends up in the APK |
| `google_base_url` | `ROLX_ANDROID_GOOGLE_BASE_URL` | debug builds only, for the mock |

Without a client, signing in reports that the build has no Google client. A release build talks
HTTPS only.

## Build and run on the emulator

From the workspace root:

```powershell
dx build -p rolxapp-android --android --target x86_64-linux-android
Start-Process "$env:ANDROID_HOME\emulator\emulator.exe" -ArgumentList "-avd", "<your AVD>"
adb wait-for-device
adb install -r target\dx\rolxapp-android\debug\android\app\app\build\outputs\apk\debug\app-debug.apk
adb shell monkey -p ch.cudos.rolxapp -c android.intent.category.LAUNCHER 1
```

The first start in an emulator can stay white for 20 seconds or more.

To use the mock, run `cargo run -p rolx-mock` on the host. The emulator reaches it at `10.0.2.2`:

```toml
server_url = "http://10.0.2.2:8090"
google_device_client_id = "mock-client"
google_device_client_secret = "mock-secret"
google_base_url = "http://10.0.2.2:8090/google"
```

## APKs

Optimized builds, signed with the Android debug key, for installing by hand on your own devices.
They are not for a store or for handing around:

- They are **debuggable**: anyone with USB debugging on the phone can read the stored session, and
  so the bearer token, with `adb run-as`.
- A build with real settings holds the Google client secret. Do not share it beyond your team.

They talk HTTPS only, so they cannot use the mock. Build each architecture from a clean directory,
because a leftover library of the other one would end up in the APK too:

```powershell
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue target\dx\rolxapp-android\release
New-Item -ItemType Directory -Force dist\android
dx build -p rolxapp-android --android --target aarch64-linux-android --release
Copy-Item target\dx\rolxapp-android\release\android\app\app\build\outputs\apk\debug\app-debug.apk dist\android\rolx-aarch64-debug-signed.apk
```

Likewise with `x86_64-linux-android` for the emulator. `aarch64` is for phones, about 19 MB. Build
with your real settings in `config.local.toml`, or the app has no server and cannot sign in. `dist/`
is ignored by git. (`dx bundle` gives a `.aab` for a store instead, which cannot be installed
directly.)
