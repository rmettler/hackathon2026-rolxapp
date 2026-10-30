//! `cargo xtask fake-session`: a `session.json` for the fake user of `cargo xtask fake-server`, so the apps start
//! logged in without a Google login.

use std::fs;
use std::path::PathBuf;

use chrono::Local;
use serde_json::{Value, json};

/// The one user of the fake server. Any other user id works too, the fake server ignores it.
pub const USER_ID: &str = "f4ce0000-0000-4000-8000-000000000001";
pub const USER_EMAIL: &str = "fiona.fake@example.com";
pub const USER_FIRST_NAME: &str = "Fiona";
pub const USER_LAST_NAME: &str = "Fake";
/// Far enough away that the apps never try to extend the session.
const SESSION_EXPIRES: &str = "2099-12-31T23:59:59Z";

#[derive(clap::Args)]
pub struct FakeSessionArgs {
    /// Where to write the session file, e.g. a copy of the app's session dir.
    #[arg(long)]
    out: PathBuf,
    /// Overwrite an existing file (it is backed up to the temp dir first).
    #[arg(long)]
    force: bool,
}

/// The fake user as the server's `User` resource (`Users/WebApi/Resource/User.cs`).
pub fn user_json() -> Value {
    json!({
        "id": USER_ID,
        "firstName": USER_FIRST_NAME,
        "lastName": USER_LAST_NAME,
        "email": USER_EMAIL,
        "avatarUrl": "",
        "role": 1,
        "entryDate": "2020-01-01",
        "leavingDate": null,
        "isConfirmed": true,
        "partTimeSettings": [],
        "balanceCorrections": [],
        "vacationDaysSettings": [],
        "targetBillabilitySettings": [],
        "positionSettings": [],
    })
}

/// A session in the format of `rolx-core/src/session.rs` (field names of `Session`, pretty-printed like `save`).
pub fn session_json() -> String {
    // A struct rather than `json!`, which would sort the fields.
    #[derive(serde::Serialize)]
    struct Session {
        bearer_token: &'static str,
        expires: &'static str,
        user_id: &'static str,
        user_email: &'static str,
        user_first_name: &'static str,
        user_last_name: &'static str,
    }
    let session = Session {
        bearer_token: "fake-session-token",
        expires: SESSION_EXPIRES,
        user_id: USER_ID,
        user_email: USER_EMAIL,
        user_first_name: USER_FIRST_NAME,
        user_last_name: USER_LAST_NAME,
    };
    serde_json::to_string_pretty(&session).expect("plain strings always serialize")
}

/// A path in the temp dir for a backup of a session file, unique per second.
pub fn backup_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    ))
}

pub fn run(args: &FakeSessionArgs) -> Result<String, String> {
    let mut note = String::new();
    if args.out.exists() {
        if !args.force {
            return Err(format!(
                "{} exists (a real login?), pass --force to overwrite it after a backup",
                args.out.display()
            ));
        }
        let backup = backup_path("rolx-session-backup");
        fs::copy(&args.out, &backup)
            .map_err(|e| format!("cannot back up {}: {e}", args.out.display()))?;
        note = format!("\nThe previous file is backed up to {}", backup.display());
    }
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    fs::write(&args.out, session_json())
        .map_err(|e| format!("cannot write {}: {e}", args.out.display()))?;
    Ok(format!(
        "Wrote a fake session for {USER_FIRST_NAME} {USER_LAST_NAME} to {} (expires {SESSION_EXPIRES}){note}",
        args.out.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rolx_core::session::Session;

    #[test]
    fn session_parses_as_the_apps_session_and_does_not_expire() {
        let session: Session = serde_json::from_str(&session_json()).unwrap();
        assert_eq!(session.user_id, USER_ID);
        assert_eq!(session.user_first_name, USER_FIRST_NAME);
        assert!(!session.is_near_expiry());
        // `Session::save` writes it the same way.
        assert_eq!(
            serde_json::to_string_pretty(&session).unwrap(),
            session_json()
        );
    }
}
