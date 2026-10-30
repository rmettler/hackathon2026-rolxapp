use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::tailwind;
use crate::secret_scan;
use crate::util::{check_cmd_output, read_cargo_metadata, run_cmd};

/// Built only for their own targets, see `WASM_PACKAGES` and the `dx` Android build.
const NOT_NATIVE: [&str; 2] = ["rolxapp-web", "rolxapp-android"];

/// Packages linted for wasm32 with their extra cargo arguments, each only once it exists.
const WASM_PACKAGES: [(&str, &[&str]); 3] = [
    ("rolx-core", &["--no-default-features"]),
    ("rolx-ui", &[]),
    ("rolxapp-web", &[]),
];

/// Runs the gate of `docs/plan.md` §3.1, stopping at the first failure. Packages are built one by
/// one so the Dioxus renderer features of different apps never unify.
pub fn run(root: &Path) -> Result<String, String> {
    let packages: Vec<String> = read_cargo_metadata(root)?
        .packages
        .into_iter()
        .map(|p| p.name)
        .collect();
    let test_dirs = TestDirs::create(root)?;

    cargo(root, &["fmt", "--all", "--", "--check"], None)?;

    println!("{}", secret_scan::run(root)?);

    for pkg in packages
        .iter()
        .filter(|p| !NOT_NATIVE.contains(&p.as_str()))
    {
        cargo(
            root,
            &["clippy", "-p", pkg, "--all-targets", "--", "-D", "warnings"],
            None,
        )?;
        cargo(root, &["test", "-p", pkg], Some(&test_dirs))?;
    }

    for (pkg, extra) in WASM_PACKAGES {
        if packages.iter().any(|p| p == pkg) {
            let mut args = vec!["clippy", "-p", pkg, "--target", "wasm32-unknown-unknown"];
            args.extend(extra);
            args.extend(["--", "-D", "warnings"]);
            cargo(root, &args, None)?;
        }
    }

    check_single_dioxus(root)?;

    if packages.iter().any(|p| p == "rolx-ui") {
        check_tailwind_fresh(root)?;
    }

    Ok("check: all gates passed".to_string())
}

/// The generated CSS is committed because `asset!` needs it at compile time, so it must be tracked
/// and match the sources. Compares with the index, so it works before the milestone commit.
fn check_tailwind_fresh(root: &Path) -> Result<(), String> {
    println!("check: tailwind CSS is fresh");
    println!("{}", tailwind::run(root)?);

    let tracked = run_cmd(
        "git",
        &["ls-files", "--error-unmatch", tailwind::CSS_FILE],
        root,
    )?;
    check_cmd_output(&tracked, "generated CSS tracked by git")?;

    let diff = run_cmd(
        "git",
        &["diff", "--exit-code", "--quiet", "--", tailwind::CSS_FILE],
        root,
    )?;
    diff.status.success().then_some(()).ok_or_else(|| {
        format!(
            "{} is stale: stage the regenerated file",
            tailwind::CSS_FILE
        )
    })
}

/// Scratch dirs handed to tests, so none of them can reach the real config or session.
struct TestDirs {
    config: PathBuf,
    cache: PathBuf,
}

impl TestDirs {
    fn create(root: &Path) -> Result<Self, String> {
        let base = root.join("target").join("xtask-check");
        if base.exists() {
            fs::remove_dir_all(&base)
                .map_err(|e| format!("cannot clear {}: {e}", base.display()))?;
        }
        let dirs = Self {
            config: base.join("config"),
            cache: base.join("cache"),
        };
        for dir in [&dirs.config, &dirs.cache] {
            fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        Ok(dirs)
    }
}

fn cargo(root: &Path, args: &[&str], test_dirs: Option<&TestDirs>) -> Result<(), String> {
    println!("check: cargo {}", args.join(" "));
    let mut command = Command::new("cargo");
    command.args(args).current_dir(root);
    if let Some(dirs) = test_dirs {
        command
            .env("ROLX_CONFIG_DIR", &dirs.config)
            .env("ROLX_CACHE_DIR", &dirs.cache);
    }
    let status = command
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("`cargo {}` failed", args.join(" ")))
}

/// Guards against a stray second copy of a Dioxus crate (other version or source).
fn check_single_dioxus(root: &Path) -> Result<(), String> {
    let args = ["tree", "-d", "--workspace", "--target", "all"];
    println!("check: cargo {}", args.join(" "));
    let out = run_cmd("cargo", &args, root)?;
    check_cmd_output(&out, "`cargo tree -d`")?;

    let duplicates = duplicate_dioxus_crates(&String::from_utf8_lossy(&out.stdout));
    if duplicates.is_empty() {
        return Ok(());
    }
    Err(format!(
        "duplicate Dioxus crates:\n{}",
        duplicates.join("\n")
    ))
}

/// The duplicates themselves are the unindented lines of `cargo tree -d`; indented ones are their
/// dependents, which may well be Dioxus crates without being duplicated. A crate listed twice with
/// the same version and source is one package built with different features, which is fine.
fn duplicate_dioxus_crates(tree: &str) -> Vec<String> {
    let mut packages: Vec<String> = tree
        .lines()
        .filter(|line| line.starts_with("dioxus"))
        .map(|line| line.replace(" (proc-macro)", "").replace(" (*)", ""))
        .collect();
    packages.sort();
    packages.dedup();

    let name = |package: &str| package.split(' ').next().unwrap_or_default().to_string();
    packages
        .iter()
        .filter(|package| packages.iter().filter(|p| name(p) == name(package)).count() > 1)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_duplicated_dioxus_crates_only_at_top_level() {
        let tree = "\
base64 v0.21.7
└── dioxus-cli-config v0.8.0

dioxus-core v0.7.0
└── other v1.0.0

dioxus-core v0.8.0 (https://github.com/DioxusLabs/dioxus#abc)
└── dioxus v0.8.0
";
        assert_eq!(
            duplicate_dioxus_crates(tree),
            [
                "dioxus-core v0.7.0",
                "dioxus-core v0.8.0 (https://github.com/DioxusLabs/dioxus#abc)"
            ]
        );
    }

    #[test]
    fn same_version_and_source_is_not_a_duplicate() {
        let tree = "\
dioxus-core-template v0.8.0-alpha.3
└── a v1.0.0

dioxus-core-template v0.8.0-alpha.3 (proc-macro)
└── b v1.0.0
";
        assert!(duplicate_dioxus_crates(tree).is_empty());
    }

    #[test]
    fn same_version_from_two_sources_is_a_duplicate() {
        let tree = "\
dioxus-html v0.8.0-alpha.3
└── a v1.0.0

dioxus-html v0.8.0-alpha.3 (https://github.com/DioxusLabs/dioxus#abc)
└── b v1.0.0
";
        assert_eq!(duplicate_dioxus_crates(tree).len(), 2);
    }

    #[test]
    fn no_duplicates_in_a_clean_tree() {
        assert!(duplicate_dioxus_crates("base64 v0.21.7\n└── other v1.0.0\n").is_empty());
    }
}
