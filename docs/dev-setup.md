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
`dx` downloads the Tailwind CLI (~130 MB) and a nightly Rust toolchain (used only to plan the
build), Gradle downloads its dependencies, and ~490 crates are compiled for Android. Expect
15–30 minutes on a slow connection; later builds take seconds.

**Config.** A phone has no `config.toml` to edit, so the build embeds one into the app: the file
at `ROLX_CONFIG` if set, else your CLI/desktop config (`cargo run -- config path`). Without one
the build warns and the app shows an error. The embedded config, including the Google client
secrets, ends up in the APK, so don't distribute builds outside the team.

**Login.** *Log in with Google* opens the phone's browser; after the Google login the browser
redirects to the app on `127.0.0.1`, then switch back to the app. *Log in with a code* uses the
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
