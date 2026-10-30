//! Asks the operating system how long ago the last keyboard or mouse input was.

use std::time::Duration;

/// Time since the last input. `ROLX_FAKE_IDLE_FILE` names a file holding a number of seconds to
/// report instead, so the away detection can be tried without sitting still.
pub fn idle_time() -> Option<Duration> {
    if let Some(path) = std::env::var_os("ROLX_FAKE_IDLE_FILE") {
        let secs: f64 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
        return Duration::try_from_secs_f64(secs).ok();
    }
    system_idle_time()
}

#[cfg(windows)]
fn system_idle_time() -> Option<Duration> {
    #[repr(C)]
    struct LastInputInfo {
        size: u32,
        time: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetLastInputInfo(info: *mut LastInputInfo) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTickCount() -> u32;
    }

    let mut info = LastInputInfo {
        size: size_of::<LastInputInfo>() as u32,
        time: 0,
    };
    // SAFETY: `info` is a valid LASTINPUTINFO with its size field set, as the API requires.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // SAFETY: takes no arguments and only reads the system tick counter.
    let now = unsafe { GetTickCount() };
    Some(Duration::from_millis(u64::from(
        now.wrapping_sub(info.time),
    )))
}

/// Away detection is only implemented for Windows so far.
#[cfg(not(windows))]
fn system_idle_time() -> Option<Duration> {
    None
}
