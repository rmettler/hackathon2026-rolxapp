//! Android dev helpers: boot an emulator, run the mobile app on an emulator or a phone (optionally against
//! `cargo xtask fake-server`), push a fake session, and build a release APK.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{fake_server, fake_session};
use crate::util::check_cmd_output;

/// Crate directory of the Dioxus mobile app, relative to the workspace root.
const APP_DIR: &str = "rolx-mobile";
const BOOT_TIMEOUT: Duration = Duration::from_secs(180);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Release keystore, relative to the workspace root: dx's gradle template prefixes `jks_file` with the way up from
/// `target/dx/<app>/release/android/app/app`, so it can't be absolute. Ignored by git.
const KEYSTORE: &str = "rolx-mobile/release.jks";
const KEY_ALIAS: &str = "rolx";
const KEYSTORE_PASSWORD_ENV: &str = "ROLX_KEYSTORE_PASSWORD";
/// Starts the signing section `apk` adds to Dioxus.toml for the build; it and everything after it is removed again.
const SIGNING_MARKER: &str =
    "# --- release signing added by `cargo xtask android apk`, removed after the build ---";

#[derive(clap::Subcommand)]
pub enum AndroidCommand {
    /// Boot an Android emulator and wait until it is ready.
    Emulator(EmulatorArgs),
    /// Run the mobile app with `dx serve` on the emulator (default) or a phone (`--device`).
    Run(RunArgs),
    /// List Android emulators (AVDs) and connected devices.
    Devices,
    /// Build a signed release APK to share, e.g. to install on phones without a dev setup.
    Apk(ApkArgs),
    /// Write a fake session (see `cargo xtask fake-session`) into the installed app, so it starts logged in.
    PushSession(PushSessionArgs),
}

impl AndroidCommand {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            AndroidCommand::Emulator(args) => emulator(args),
            AndroidCommand::Run(args) => run(root, args),
            AndroidCommand::Devices => devices(),
            AndroidCommand::Apk(args) => apk(root, args),
            AndroidCommand::PushSession(args) => push_session(root, args),
        }
    }
}

#[derive(clap::Args)]
pub struct EmulatorArgs {
    /// AVD to boot; defaults to the first one listed by `emulator -list-avds`.
    #[arg(long)]
    avd: Option<String>,
    /// Boot without loading the quick-boot snapshot.
    #[arg(long)]
    cold: bool,
}

#[derive(clap::Args)]
pub struct RunArgs {
    /// Run on a connected phone instead of the emulator; optionally pass its adb serial.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    device: Option<String>,
    /// AVD to boot if no emulator is running.
    #[arg(long, conflicts_with = "device")]
    avd: Option<String>,
    /// Build against `cargo xtask fake-server` on this port instead of the configured server: embeds a config with
    /// `server_url = "http://127.0.0.1:<port>"` and forwards the port to this machine with `adb reverse`.
    #[arg(long, value_name = "PORT", num_args = 0..=1, default_missing_value = FAKE_SERVER_PORT)]
    fake_server: Option<u16>,
    /// Extra arguments passed through to `dx serve`, e.g. `-- --release`.
    #[arg(last = true)]
    dx_args: Vec<String>,
}

#[derive(clap::Args)]
pub struct PushSessionArgs {
    /// adb serial of the device; default: `ANDROID_SERIAL`, else the only connected device.
    #[arg(long)]
    device: Option<String>,
}

/// Default of `run --fake-server`, as text for clap.
const FAKE_SERVER_PORT: &str = "5080";
const _: () = assert!(fake_server::DEFAULT_PORT == 5080);

#[derive(clap::Args)]
pub struct ApkArgs {
    /// Rust target to build for; the default runs on practically all phones.
    #[arg(long, default_value = "aarch64-linux-android")]
    target: String,
}

/// Boots an emulator (unless one is running) and waits until Android has finished booting.
fn emulator(args: &EmulatorArgs) -> Result<String, String> {
    let sdk = Sdk::locate()?;
    let serial = ensure_emulator(&sdk, args.avd.as_deref(), args.cold)?;
    Ok(format!("Emulator {serial} is ready"))
}

/// Runs `dx serve --android` for the mobile app on the emulator or a phone.
fn run(root: &Path, args: &RunArgs) -> Result<String, String> {
    let sdk = Sdk::locate()?;
    check_dx()?;

    let serial = match args.device.as_deref() {
        None => ensure_emulator(&sdk, args.avd.as_deref(), false)?,
        Some(wanted) => pick_phone(&sdk, wanted)?,
    };

    println!("Running {APP_DIR} on {serial}");
    let mut cmd = Command::new("dx");
    cmd.args(["serve", "--android"]);
    if let Some(port) = args.fake_server {
        let config = write_fake_server_config(root, port)?;
        // 127.0.0.1 is the one host dx's network security config allows plain HTTP to (for its dev server).
        let tcp = format!("tcp:{port}");
        sdk.adb_output(&["-s", &serial, "reverse", &tcp, &tcp])?;
        println!(
            "Embedding {} (fake server on port {port}, forwarded with `adb reverse`)",
            config.display()
        );
        println!(
            "Start the server with `cargo xtask fake-server --port {port}`, \
             log in with `cargo xtask android push-session`"
        );
        cmd.env("ROLX_CONFIG", &config);
    }
    if args.device.is_some() {
        // Makes dx build for aarch64, the architecture of real phones.
        cmd.arg("--device");
    }
    let status = cmd
        .args(&args.dx_args)
        .current_dir(root.join(APP_DIR))
        // adb honours ANDROID_SERIAL, so dx's adb calls target this device even if several are attached.
        .env("ANDROID_SERIAL", &serial)
        .status()
        .map_err(|e| format!("failed to run `dx`: {e}"))?;

    status
        .success()
        .then(|| "dx serve finished".to_string())
        .ok_or_else(|| format!("`dx serve` failed (exit {:?})", status.code()))
}

/// Writes the config `run --fake-server` embeds into the app; under `target/`, so it is never committed.
fn write_fake_server_config(root: &Path, port: u16) -> Result<PathBuf, String> {
    let dir = root.join("target").join("fake-server");
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join("rolx-mobile-config.toml");
    let text = format!(
        "# Written by `cargo xtask android run --fake-server`: the app talks to `cargo xtask fake-server` via\n\
         # `adb reverse`. No Google clients: log in with `cargo xtask android push-session` instead.\n\
         server_url = \"http://127.0.0.1:{port}\"\n"
    );
    fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// The config `rolx-mobile/build.rs` embeds: `ROLX_CONFIG`, else the CLI/desktop config.
fn embedded_config_path() -> Option<PathBuf> {
    env::var_os("ROLX_CONFIG")
        .map(PathBuf::from)
        .or_else(|| dirs::config_dir().map(|d| d.join("rolx").join("config.toml")))
}

/// Refuses to build a release APK whose embedded config points at a local (e.g. the fake) or unencrypted server.
fn check_release_config() -> Result<(), String> {
    // A missing config is left to build.rs, which warns about it.
    let Some(text) = embedded_config_path().and_then(|p| fs::read_to_string(p).ok()) else {
        return Ok(());
    };
    let url = text
        .parse::<toml::Table>()
        .ok()
        .and_then(|t| t.get("server_url")?.as_str().map(str::to_lowercase))
        .unwrap_or_default();
    if let Some(problem) = release_url_problem(&url) {
        return Err(format!(
            "the config to embed ({}) has server_url `{url}`, {problem}; point ROLX_CONFIG at the real config \
             or unset it",
            embedded_config_path().unwrap_or_default().display()
        ));
    }
    Ok(())
}

/// Why `url` must not end up in a release APK, if it must not.
fn release_url_problem(url: &str) -> Option<&'static str> {
    let host = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    if ["localhost", "10.0.2.2", "0.0.0.0", "["].contains(&host) || host.starts_with("127.") {
        Some("a local server (the fake server?)")
    } else if !url.starts_with("https://") {
        Some("not HTTPS")
    } else {
        None
    }
}

/// Builds a release APK with `dx bundle`, signed with the keystore at `KEYSTORE` (created on first use).
fn apk(root: &Path, args: &ApkArgs) -> Result<String, String> {
    Sdk::locate()?;
    check_dx()?;
    check_release_config()?;

    let password = env::var(KEYSTORE_PASSWORD_ENV).map_err(|_| {
        format!(
            "set {KEYSTORE_PASSWORD_ENV} to the password of the release keystore `{KEYSTORE}` \
             (a new keystore is created with it if there is none)"
        )
    })?;
    // The password ends up in a Kotlin string literal, and keytool wants at least 6 characters.
    if password.len() < 6 || password.contains(['"', '\\', '$']) {
        return Err(format!(
            "{KEYSTORE_PASSWORD_ENV} must have at least 6 characters and no `\"`, `\\` or `$`"
        ));
    }

    let keystore = root.join(KEYSTORE);
    if !keystore.is_file() {
        create_keystore(&keystore, &password)?;
        println!(
            "Created the release keystore {}. Keep it and its password: updates of the app must be signed with it.",
            keystore.display()
        );
    }

    let out_dir = root.join("target").join("apk");
    let signing = SigningConfig::add(&root.join(APP_DIR).join("Dioxus.toml"), &password)?;
    let status = Command::new("dx")
        .args(["bundle", "--android", "--release", "--package-types", "apk"])
        .args(["--target", &args.target])
        .arg("--out-dir")
        .arg(&out_dir)
        .current_dir(root.join(APP_DIR))
        .status();
    signing.remove()?;
    let status = status.map_err(|e| format!("failed to run `dx`: {e}"))?;
    if !status.success() {
        return Err(format!("`dx bundle` failed (exit {:?})", status.code()));
    }

    // dx keeps gradle's name for the APK; give it one that says what it is.
    let built = out_dir.join("app-release.apk");
    let apk = out_dir.join(format!("rolx-mobile-{}.apk", app_version(root)?));
    fs::rename(&built, &apk).map_err(|e| format!("cannot rename {}: {e}", built.display()))?;
    Ok(format!("Built {}", apk.display()))
}

/// Creates a keystore with a single RSA key `KEY_ALIAS`, valid for ~27 years (Android asks for 25+).
fn create_keystore(path: &Path, password: &str) -> Result<(), String> {
    let keytool = env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin").join(exe("keytool")))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("keytool"));
    // PKCS12 keystores use the store password for the key too.
    #[rustfmt::skip]
    let args = [
        "-genkeypair", "-keystore", &path.to_string_lossy(), "-storetype", "PKCS12", "-storepass", password,
        "-alias", KEY_ALIAS, "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000", "-dname", "CN=RolX Mobile",
    ];
    output(&keytool, &args).map(drop)
}

/// The version of the mobile app from its Cargo.toml, which dx also uses as the APK's `versionName`.
fn app_version(root: &Path) -> Result<String, String> {
    crate::util::read_cargo_metadata(root)?
        .packages
        .into_iter()
        .find(|p| p.name == APP_DIR)
        .map(|p| p.version)
        .ok_or_else(|| format!("package `{APP_DIR}` not found"))
}

/// The `[bundle.android]` section that makes dx build and sign a release APK. It holds the keystore password, so it
/// is only in Dioxus.toml during the build.
struct SigningConfig {
    path: PathBuf,
    original: String,
}

impl SigningConfig {
    fn add(path: &Path, password: &str) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        // Drops a section left over from an interrupted build.
        let original = match text.find(SIGNING_MARKER) {
            Some(i) => format!("{}\n", text[..i].trim_end()),
            None => text,
        };
        let patched = format!(
            "{original}\n{SIGNING_MARKER}\n[bundle.android]\njks_file = \"{KEYSTORE}\"\n\
             jks_password = \"{password}\"\nkey_alias = \"{KEY_ALIAS}\"\nkey_password = \"{password}\"\n"
        );
        fs::write(path, patched).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            original,
        })
    }

    fn remove(self) -> Result<(), String> {
        fs::write(&self.path, &self.original).map_err(|e| {
            format!(
                "cannot restore {}, remove its signing section by hand: {e}",
                self.path.display()
            )
        })
    }
}

/// Where the app keeps its session, relative to the dir `run-as` starts in (the app's data dir); `files/` is
/// `Context.getFilesDir()`, which the app passes to `rolx_core::paths::set_base_dir`.
const APP_SESSION: &str = "files/session.json";

/// Writes the fake server's session into the app, backing up a different session that is already there.
fn push_session(root: &Path, args: &PushSessionArgs) -> Result<String, String> {
    let sdk = Sdk::locate()?;
    let serial = pick_any_device(&sdk, args.device.as_deref())?;
    let package = installed_package(&sdk, &serial, &app_identifier(root)?)?;
    let read_session = || {
        sdk.adb_output(&[
            "-s",
            &serial,
            "exec-out",
            "run-as",
            &package,
            "cat",
            APP_SESSION,
        ])
        .unwrap_or_default()
    };

    let mut note = String::new();
    let existing = read_session();
    if existing.trim_start().starts_with('{') && !existing.contains(fake_session::USER_ID) {
        let backup = fake_session::backup_path("rolx-mobile-session-backup");
        fs::write(&backup, &existing).map_err(|e| {
            format!(
                "cannot back up the app's session to {}: {e}",
                backup.display()
            )
        })?;
        note = format!(
            "\nThe app's previous session is backed up to {}",
            backup.display()
        );
    }

    // One argument, so the redirection runs inside `run-as` (as the app) and not in adb's shell.
    let script = format!("run-as {package} sh -c 'mkdir -p files && cat > {APP_SESSION}'");
    let mut child = Command::new(sdk.adb())
        .args(["-s", &serial, "exec-in", &script])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run adb: {e}"))?;
    let sent = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(fake_session::session_json().as_bytes());
    let status = child.wait().map_err(|e| format!("adb failed: {e}"))?;
    if sent.is_err() || !status.success() || !read_session().contains(fake_session::USER_ID) {
        return Err(format!(
            "could not write {APP_SESSION} of {package} on {serial} (exit {:?}); \
             `run-as` needs a debug build of the app",
            status.code()
        ));
    }
    Ok(format!(
        "Wrote a fake session ({} {}) into {package} on {serial}; restart the app to use it.{note}",
        fake_session::USER_FIRST_NAME,
        fake_session::USER_LAST_NAME
    ))
}

/// The Android package name: `identifier` in the app's Dioxus.toml.
fn app_identifier(root: &Path) -> Result<String, String> {
    let path = root.join(APP_DIR).join("Dioxus.toml");
    let text =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("cannot parse {}: {e}", path.display()))?;
    table
        .get("bundle")
        .and_then(|b| b.get("identifier"))
        .and_then(|i| i.as_str())
        .map(String::from)
        .ok_or_else(|| format!("no [bundle] identifier in {}", path.display()))
}

/// The installed package matching `identifier`, compared case-insensitively in case dx changes the case.
fn installed_package(sdk: &Sdk, serial: &str, identifier: &str) -> Result<String, String> {
    let out = sdk.adb_output(&["-s", serial, "shell", "pm", "list", "packages"])?;
    out.lines()
        .filter_map(|l| l.trim().strip_prefix("package:"))
        .find(|p| p.eq_ignore_ascii_case(identifier))
        .map(String::from)
        .ok_or_else(|| {
            format!(
                "{identifier} is not installed on {serial}, run `cargo xtask android run` first"
            )
        })
}

/// `wanted`, else `ANDROID_SERIAL`, else the only ready device (emulator or phone).
fn pick_any_device(sdk: &Sdk, wanted: Option<&str>) -> Result<String, String> {
    if let Some(serial) = wanted
        .map(String::from)
        .or_else(|| env::var("ANDROID_SERIAL").ok().filter(|s| !s.is_empty()))
    {
        return Ok(serial);
    }
    let ready: Vec<Device> = sdk
        .devices()?
        .into_iter()
        .filter(|d| d.state == "device")
        .collect();
    match ready.as_slice() {
        [] => Err("no device found by adb, start one with `cargo xtask android emulator`".into()),
        [only] => Ok(only.serial.clone()),
        _ => Err(format!(
            "several devices connected ({}), pick one with `--device <serial>`",
            ready
                .iter()
                .map(|d| d.serial.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Lists the available AVDs and the devices adb can see.
fn devices() -> Result<String, String> {
    let sdk = Sdk::locate()?;
    let avds = sdk.list_avds()?;
    let devices = sdk.devices()?;

    let mut out = String::from("Emulators (AVDs):\n");
    if avds.is_empty() {
        out.push_str("  none, create one in Android Studio > Device Manager\n");
    }
    for avd in &avds {
        out.push_str(&format!("  {avd}\n"));
    }
    out.push_str("Connected devices (adb):\n");
    if devices.is_empty() {
        out.push_str("  none\n");
    }
    for d in &devices {
        let kind = if d.is_emulator() { "emulator" } else { "phone" };
        out.push_str(&format!("  {:<28} {:<14} {kind}\n", d.serial, d.state));
    }
    Ok(out.trim_end().to_string())
}

/// Returns the serial of a booted emulator running `avd`, booting it unless it is running. Without `avd`, any running
/// emulator is used, else the first AVD is booted.
fn ensure_emulator(sdk: &Sdk, avd: Option<&str>, cold: bool) -> Result<String, String> {
    let running = sdk
        .devices()?
        .into_iter()
        .filter(Device::is_emulator)
        .find(|d| avd.is_none() || sdk.avd_name(&d.serial).as_deref() == avd);
    if let Some(d) = running {
        println!("Emulator {} is already running", d.serial);
        if cold {
            println!("warning: --cold ignored, the emulator is already running");
        }
        return wait_for_boot(sdk, avd);
    }

    let avd = match avd {
        Some(avd) => avd.to_string(),
        None => sdk
            .list_avds()?
            .into_iter()
            .next()
            .ok_or("no AVD found, create one in Android Studio > Device Manager".to_string())?,
    };

    println!("Booting emulator `{avd}`");
    let mut cmd = Command::new(sdk.emulator());
    cmd.args(["-avd", &avd, "-netdelay", "none", "-netspeed", "full"]);
    if cold {
        cmd.arg("-no-snapshot-load");
    }
    spawn_detached(&mut cmd).map_err(|e| format!("failed to start the emulator: {e}"))?;

    wait_for_boot(sdk, Some(&avd))
}

/// Polls adb until an emulator running `avd` (or any emulator) reports `sys.boot_completed`, returning its serial.
fn wait_for_boot(sdk: &Sdk, avd: Option<&str>) -> Result<String, String> {
    let start = Instant::now();
    print!("Waiting for Android to boot");
    loop {
        let booted = sdk.devices()?.into_iter().find(|d| {
            d.is_emulator()
                && d.state == "device"
                && (avd.is_none() || sdk.avd_name(&d.serial).as_deref() == avd)
                && sdk
                    .adb_output(&["-s", &d.serial, "shell", "getprop", "sys.boot_completed"])
                    .is_ok_and(|out| out.trim() == "1")
        });
        if let Some(d) = booted {
            println!();
            return Ok(d.serial);
        }
        if start.elapsed() > BOOT_TIMEOUT {
            println!();
            return Err(format!(
                "emulator did not finish booting within {}s",
                BOOT_TIMEOUT.as_secs()
            ));
        }
        print!(".");
        let _ = std::io::stdout().flush();
        thread::sleep(POLL_INTERVAL);
    }
}

/// Picks the phone to run on: `wanted` is an adb serial, or empty to use the only connected phone.
fn pick_phone(sdk: &Sdk, wanted: &str) -> Result<String, String> {
    let phones: Vec<Device> = sdk
        .devices()?
        .into_iter()
        .filter(|d| !d.is_emulator())
        .collect();
    let serials = || {
        phones
            .iter()
            .map(|d| d.serial.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let phone = if wanted.is_empty() {
        match phones.as_slice() {
            [] => {
                return Err(
                    "no phone found by adb: enable Developer options > USB debugging, \
                     connect the phone and accept the prompt, then check with `adb devices`"
                        .into(),
                );
            }
            [phone] => phone,
            _ => {
                return Err(format!(
                    "several phones connected ({}), pick one with `--device <serial>`",
                    serials()
                ));
            }
        }
    } else {
        phones
            .iter()
            .find(|d| d.serial == wanted)
            .ok_or_else(|| format!("phone `{wanted}` not found, connected: [{}]", serials()))?
    };

    match phone.state.as_str() {
        "device" => Ok(phone.serial.clone()),
        "unauthorized" => Err(format!(
            "phone {} is unauthorized: unlock it and accept the \"Allow USB debugging\" prompt",
            phone.serial
        )),
        state => Err(format!(
            "phone {} is not ready (state: {state})",
            phone.serial
        )),
    }
}

/// Fails early with an install hint if the Dioxus CLI is missing.
fn check_dx() -> Result<(), String> {
    let out = Command::new("dx")
        .arg("--version")
        .output()
        .map_err(|_| "`dx` not found, install it with `cargo binstall dioxus-cli@0.7.10`")?;
    let version = String::from_utf8_lossy(&out.stdout);
    if !version.contains(" 0.7.") {
        println!(
            "warning: expected dioxus-cli 0.7.x, found `{}`",
            version.trim()
        );
    }
    Ok(())
}

/// Starts a process that outlives this one and doesn't receive this terminal's Ctrl-C.
fn spawn_detached(cmd: &mut Command) -> std::io::Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;

        // `cargo run` places us in a job object; break away so the emulator isn't tied to it.
        // That fails if the job forbids breakaway, so retry without it.
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    cmd.spawn().map(drop)
}

/// A device line from `adb devices`.
struct Device {
    serial: String,
    /// `device` when ready; otherwise e.g. `offline` or `unauthorized`.
    state: String,
}

impl Device {
    fn is_emulator(&self) -> bool {
        self.serial.starts_with("emulator-")
    }
}

/// The Android SDK, located via `ANDROID_HOME` / `ANDROID_SDK_ROOT` or Android Studio's default path.
struct Sdk {
    root: PathBuf,
}

impl Sdk {
    fn locate() -> Result<Self, String> {
        env::var_os("ANDROID_HOME")
            .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
            .map(PathBuf::from)
            .or_else(default_sdk_dir)
            .filter(|root| root.is_dir())
            .map(|root| Sdk { root })
            .ok_or_else(|| "Android SDK not found, set ANDROID_HOME (see docs/dev-setup.md)".into())
    }

    fn adb(&self) -> PathBuf {
        self.root.join("platform-tools").join(exe("adb"))
    }

    fn emulator(&self) -> PathBuf {
        self.root.join("emulator").join(exe("emulator"))
    }

    fn adb_output(&self, args: &[&str]) -> Result<String, String> {
        output(&self.adb(), args)
    }

    fn devices(&self) -> Result<Vec<Device>, String> {
        let out = self.adb_output(&["devices"])?;
        Ok(out
            .lines()
            .filter(|l| !l.starts_with("List of devices") && !l.starts_with('*'))
            .filter_map(|l| {
                let mut cols = l.split_whitespace();
                Some(Device {
                    serial: cols.next()?.to_string(),
                    state: cols.next()?.to_string(),
                })
            })
            .collect())
    }

    /// Returns the name of the AVD the emulator `serial` runs, or `None` if the emulator console doesn't answer.
    fn avd_name(&self, serial: &str) -> Option<String> {
        // Prints the name, then `OK`.
        let out = self
            .adb_output(&["-s", serial, "emu", "avd", "name"])
            .ok()?;
        out.lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && *l != "OK")
            .map(String::from)
    }

    fn list_avds(&self) -> Result<Vec<String>, String> {
        let out = output(&self.emulator(), &["-list-avds"])?;
        // Newer emulators also print `INFO | ...` log lines; AVD names never contain spaces.
        Ok(out
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.contains(char::is_whitespace) && !l.contains('|'))
            .map(String::from)
            .collect())
    }
}

fn default_sdk_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Android").join("Sdk"))
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Android/sdk"))
    } else {
        env::var_os("HOME").map(|h| PathBuf::from(h).join("Android/Sdk"))
    }
}

fn exe(name: &str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}

fn output(program: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run `{}`: {e}", program.display()))?;
    check_cmd_output(&out, &format!("`{}`", program.display()))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::release_url_problem;

    #[test]
    fn release_apks_only_take_remote_https_servers() {
        assert_eq!(release_url_problem("https://rolx.example.com"), None);
        assert_eq!(release_url_problem("https://rolx.example.com:8443/"), None);
        for url in [
            "http://127.0.0.1:5080",
            "http://10.0.2.2:5080",
            "https://localhost",
            "http://rolx.example.com",
            "",
        ] {
            assert!(release_url_problem(url).is_some(), "{url}");
        }
    }
}
