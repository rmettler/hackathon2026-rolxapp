use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::util::{check_cmd_output, run_cmd};

/// Only the pattern check skips these paths; the user's actual values are looked for everywhere.
const PATTERN_ALLOWLIST_PREFIX: &str = "xtask/src/secret_scan";

/// Shorter config/session values (e.g. first names) would match unrelated text.
const MIN_VALUE_LEN: usize = 8;

/// A suspected secret. Holds no matched text, so reporting it cannot leak the secret.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    path: String,
    line: usize,
    kind: &'static str,
}

struct Scanner {
    patterns: Vec<(&'static str, Regex)>,
    values: Vec<String>,
}

impl Scanner {
    fn new(values: Vec<String>) -> Self {
        let patterns = [
            ("JWT", r"eyJ[\w-]{10,}\.[\w-]{10,}\."),
            ("bearer token", r"Bearer\s+[\w.~+/-]{20,}"),
            ("Google client secret", r"GOCSPX-[\w-]{10,}"),
            ("Google client id", r"\d+-\w+\.apps\.googleusercontent\.com"),
        ]
        .map(|(kind, pattern)| (kind, Regex::new(pattern).expect("valid pattern")));

        Self {
            patterns: patterns.into(),
            values,
        }
    }

    fn scan_line(&self, path: &str, line: usize, text: &str, findings: &mut Vec<Finding>) {
        let mut report = |kind| {
            findings.push(Finding {
                path: path.to_string(),
                line,
                kind,
            })
        };

        if !path.starts_with(PATTERN_ALLOWLIST_PREFIX) {
            for (kind, regex) in &self.patterns {
                if regex.is_match(text) {
                    report(*kind);
                }
            }
        }
        if self.values.iter().any(|value| text.contains(value)) {
            report("value from the user's config or session");
        }
    }

    fn scan_file(&self, path: &str, content: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (index, text) in content.lines().enumerate() {
            self.scan_line(path, index + 1, text, &mut findings);
        }
        findings
    }

    /// Scans the added lines of a `git diff -U0` output.
    fn scan_diff(&self, diff: &str) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut path = String::new();
        let mut line = 0;
        // Only outside hunks is `+++` a file header; inside it is an added line starting with `++`.
        let mut in_hunk = false;

        for text in diff.lines() {
            if text.starts_with("diff --git ") {
                in_hunk = false;
            } else if let Some(header) = text.strip_prefix("@@ ") {
                in_hunk = true;
                // An unparsable header only costs the line numbers; the lines are still scanned.
                line = hunk_start(header).unwrap_or(0);
            } else if !in_hunk {
                if let Some(header) = text.strip_prefix("+++ ") {
                    path = diff_path(header);
                }
            } else if let Some(added) = text.strip_prefix('+') {
                self.scan_line(&path, line, added, &mut findings);
                line += 1;
            }
        }
        findings
    }
}

/// Start line in the new file, from the rest of a hunk header: `-1,2 +3,4 @@ context`.
fn hunk_start(header: &str) -> Option<usize> {
    let new_range = header.split(' ').nth(1)?.strip_prefix('+')?;
    new_range.split(',').next()?.parse().ok()
}

/// File path from a `+++` header value, which git may quote and may end with a tab.
fn diff_path(header: &str) -> String {
    let path = header.trim_end_matches('\t').trim_matches('"');
    path.strip_prefix("b/").unwrap_or(path).to_string()
}

fn override_and_default(env_name: &str, default_base: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = default_base.map(|b| b.join("rolx")).into_iter().collect();
    dirs.extend(
        env::var_os(env_name)
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    dirs
}

/// Reads the values to look for from the real and the overridden config and session. Parse errors
/// are reported without their details, because those quote the file's content.
fn read_user_values() -> Result<Vec<String>, String> {
    let mut values = Vec::new();

    for dir in override_and_default("ROLX_CONFIG_DIR", dirs::config_dir()) {
        let path = dir.join("config.toml");
        if let Some(text) = read_if_exists(&path)? {
            let parsed = toml::from_str(&text).map_err(|_| cannot_parse(&path))?;
            collect_strings(&parsed, &mut values);
        }
    }
    for dir in override_and_default("ROLX_CACHE_DIR", dirs::cache_dir()) {
        let path = dir.join("session.json");
        if let Some(text) = read_if_exists(&path)? {
            let parsed = serde_json::from_str(&text).map_err(|_| cannot_parse(&path))?;
            collect_strings(&parsed, &mut values);
        }
    }

    values.sort();
    values.dedup();
    Ok(values)
}

fn cannot_parse(path: &Path) -> String {
    format!("secret-scan: cannot parse {}", path.display())
}

fn read_if_exists(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("secret-scan: cannot read {}: {e}", path.display())),
    }
}

fn collect_strings(value: &serde_json::Value, values: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) if s.len() >= MIN_VALUE_LEN => values.push(s.clone()),
        serde_json::Value::Array(items) => {
            items.iter().for_each(|item| collect_strings(item, values));
        }
        serde_json::Value::Object(map) => {
            map.values().for_each(|item| collect_strings(item, values));
        }
        _ => {}
    }
}

/// Scans the staged diff and the untracked files. Fails closed: whatever cannot be read is an error.
pub fn run(root: &Path) -> Result<String, String> {
    let scanner = Scanner::new(read_user_values()?);

    let diff = run_cmd(
        "git",
        &[
            "-c",
            "core.quotepath=off",
            "diff",
            "--cached",
            "-U0",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            // Also shows the content of binary files, which `lines()` copes with once lossy.
            "--text",
        ],
        root,
    )?;
    check_cmd_output(&diff, "`git diff --cached`")?;
    let mut findings = scanner.scan_diff(&String::from_utf8_lossy(&diff.stdout));

    let untracked = run_cmd(
        "git",
        &["ls-files", "--others", "--exclude-standard", "-z"],
        root,
    )?;
    check_cmd_output(&untracked, "`git ls-files`")?;
    let untracked = String::from_utf8_lossy(&untracked.stdout).into_owned();
    for path in untracked.split('\0').filter(|p| !p.is_empty()) {
        // Nested git repositories are listed as `dir/`; their files are not ours to commit.
        if path.ends_with('/') {
            continue;
        }
        let bytes = fs::read(root.join(path))
            .map_err(|e| format!("secret-scan: cannot read {path}: {e}"))?;
        findings.extend(scanner.scan_file(path, &String::from_utf8_lossy(&bytes)));
    }

    if findings.is_empty() {
        return Ok("secret-scan: no findings".to_string());
    }
    let report: Vec<String> = findings
        .iter()
        .map(|f| format!("{}:{}: {}", f.path, f.line, f.kind))
        .collect();
    Err(format!(
        "secret-scan: {} finding(s):\n{}",
        findings.len(),
        report.join("\n")
    ))
}

#[cfg(test)]
#[path = "secret_scan.tests.rs"]
mod tests;
