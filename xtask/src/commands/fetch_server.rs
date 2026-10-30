use std::path::Path;
use std::process::Command;

use crate::util::{check_cmd_output, read_cargo_metadata, run_cmd};

const PIN_KEY: &str = "rolx-server";

struct Pin {
    url: String,
    rev: String,
    dir: String,
}

pub fn run(root: &Path) -> Result<String, String> {
    read_pin(root).and_then(|pin| fetch_server(root, &pin))
}

fn read_pin(root: &Path) -> Result<Pin, String> {
    let meta = read_cargo_metadata(root)?;

    let pin = meta
        .metadata
        .get(PIN_KEY)
        .ok_or_else(|| format!("[workspace.metadata.{PIN_KEY}] not found"))?;

    let read_param = |param: &str| {
        pin.get(param).and_then(|v| v.as_str()).ok_or(format!(
            "missing `{param}` in [workspace.metadata.{PIN_KEY}]"
        ))
    };

    Ok(Pin {
        url: read_param("url")?.to_string(),
        rev: read_param("rev")?.to_string(),
        dir: read_param("dir")?.to_string(),
    })
}

fn fetch_server(root: &Path, pin: &Pin) -> Result<String, String> {
    let clone_dir = root.join(&pin.dir);

    if !is_git_work_tree(&clone_dir) {
        if clone_dir.is_dir() || clone_dir.exists() {
            return Err(format!(
                "{} is not a git checkout, please remove first",
                clone_dir.display()
            ));
        }

        println!("Cloning {} into {}", pin.url, clone_dir.display());
        // Stream git's output instead of capturing it so clone progress is visible.
        let status = Command::new("git")
            .args(["clone", &pin.url, &pin.dir])
            .current_dir(root)
            .status()
            .map_err(|e| format!("failed to run `git clone`: {e}"))?;
        if !status.success() {
            return Err(format!(
                "`git clone` of {} failed — check that the URL is reachable and you have credentials",
                pin.url
            ));
        }
    } else {
        ensure_origin(&clone_dir, &pin.url)?;
        ensure_clean(&clone_dir)?;
    }

    check_cmd_output(
        &run_cmd("git", &["fetch", "origin"], &clone_dir)?,
        "`git fetch origin`",
    )?;

    // Resolve to the full SHA so short revs in Cargo.toml also validate.
    let full_rev = resolve_rev(&clone_dir, &pin.rev)?;

    // Detached checkout so it doesn't disturb any branch/working state.
    check_cmd_output(
        &run_cmd(
            "git",
            &["checkout", "-q", "--detach", &full_rev],
            &clone_dir,
        )?,
        &format!("`git checkout {}`", short_rev(&full_rev)),
    )?;

    let head_out = run_cmd("git", &["rev-parse", "HEAD"], &clone_dir)?;
    check_cmd_output(&head_out, "`git rev-parse HEAD`")?;
    let head = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
    if head != full_rev {
        return Err(format!(
            "expected server HEAD {}, found {} ({})",
            full_rev,
            head,
            clone_dir.display()
        ));
    }

    Ok(format!(
        "RolX server @ {} is at rev {}",
        pin.dir,
        short_rev(&full_rev)
    ))
}

fn is_git_work_tree(dir: &Path) -> bool {
    run_cmd("git", &["rev-parse", "--is-inside-work-tree"], dir)
        .is_ok_and(|out| out.status.success())
}

/// Refuse to proceed if the checkout's origin isn't the pinned URL; a stray
/// clone would silently point at the wrong server source.
fn ensure_origin(server_dir: &Path, url: &str) -> Result<(), String> {
    let current = run_cmd("git", &["remote", "get-url", "origin"], server_dir)
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    (current == url).ok_or_else( || {
        let found = if current.is_empty() {
            "no origin".to_string()
        } else {
            format!("origin `{current}`")
        };
        format!(
            "{} has {found} but Cargo.toml pins `{url}` — delete the directory and re-run xtask, or fix the pin",
            server_dir.display()
        )
    })
}

/// Refuse to proceed if the checkout has local changes; moving HEAD would leave
/// a mixed tree at the wrong rev.
fn ensure_clean(server_dir: &Path) -> Result<(), String> {
    let out = run_cmd("git", &["status", "--porcelain"], server_dir)?;
    check_cmd_output(&out, "`git status`")?;

    let status = String::from_utf8_lossy(&out.stdout);
    if !status.trim().is_empty() {
        let preview = status.lines().take(5).collect::<Vec<_>>().join("\n");
        return Err(format!(
            "{} has local changes — stash/commit them or delete the directory and re-run xtask:\n{}",
            server_dir.display(),
            preview
        ));
    }

    Ok(())
}

/// Resolve a (possibly short) revision to its full SHA; clear error if unknown.
fn resolve_rev(server_dir: &Path, rev: &str) -> Result<String, String> {
    let out = run_cmd(
        "git",
        &["rev-parse", &format!("{rev}^{{commit}}")],
        server_dir,
    )?;
    out.status.success().ok_or_else(|| format!(
            "server revision {} not found in {} — check the rev in Cargo.toml, or that `git fetch origin` succeeded",
            short_rev(rev),
            server_dir.display()
        ))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn short_rev(rev: &str) -> &str {
    &rev[..8.min(rev.len())]
}
