//! Android dev helpers: boot an emulator and run the mobile app on an emulator or a phone.

use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::util::check_cmd_output;

/// Crate directory of the Dioxus mobile app, relative to the workspace root.
const APP_DIR: &str = "rolx-mobile";
const BOOT_TIMEOUT: Duration = Duration::from_secs(180);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(clap::Subcommand)]
pub enum AndroidCommand {
    /// Boot an Android emulator and wait until it is ready.
    Emulator(EmulatorArgs),
    /// Run the mobile app with `dx serve` on the emulator (default) or a phone (`--device`).
    Run(RunArgs),
    /// List Android emulators (AVDs) and connected devices.
    Devices,
}

impl AndroidCommand {
    pub fn run(&self, root: &Path) -> Result<String, String> {
        match self {
            AndroidCommand::Emulator(args) => emulator(args),
            AndroidCommand::Run(args) => run(root, args),
            AndroidCommand::Devices => devices(),
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
    /// Extra arguments passed through to `dx serve`, e.g. `-- --release`.
    #[arg(last = true)]
    dx_args: Vec<String>,
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

/// Returns the serial of a booted emulator, booting `avd` (or the first AVD) if none is running.
fn ensure_emulator(sdk: &Sdk, avd: Option<&str>, cold: bool) -> Result<String, String> {
    if let Some(d) = sdk.devices()?.into_iter().find(Device::is_emulator) {
        println!("Emulator {} is already running", d.serial);
        return wait_for_boot(sdk);
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

    wait_for_boot(sdk)
}

/// Polls adb until an emulator reports `sys.boot_completed`, returning its serial.
fn wait_for_boot(sdk: &Sdk) -> Result<String, String> {
    let start = Instant::now();
    print!("Waiting for Android to boot");
    loop {
        if let Some(d) = sdk
            .devices()?
            .into_iter()
            .find(|d| d.is_emulator() && d.state == "device")
        {
            let booted = sdk
                .adb_output(&["-s", &d.serial, "shell", "getprop", "sys.boot_completed"])
                .is_ok_and(|out| out.trim() == "1");
            if booted {
                println!();
                return Ok(d.serial);
            }
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
