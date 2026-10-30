use super::*;

// Fixtures are concatenated at run time so this file holds nothing that looks like a secret.
fn fake_jwt() -> String {
    ["eyJ", "aaaaaaaaaaaa", ".", "bbbbbbbbbbbb", ".", "cccc"].concat()
}

fn scanner() -> Scanner {
    Scanner::new(Vec::new())
}

fn kinds(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(|f| f.kind).collect()
}

#[test]
fn detects_a_fake_jwt() {
    let findings = scanner().scan_file("a.md", &format!("token: {}", fake_jwt()));
    assert_eq!(kinds(&findings), ["JWT"]);
}

#[test]
fn detects_a_bearer_token() {
    let text = ["Authorization: Bearer ", &"a1".repeat(10)].concat();
    assert_eq!(kinds(&scanner().scan_file("a.md", &text)), ["bearer token"]);
}

#[test]
fn detects_google_client_credentials() {
    let secret = ["GOCSPX", "-", "abcdefghijkl"].concat();
    let id = ["1234-abc", ".apps.", "googleusercontent", ".com"].concat();
    assert_eq!(
        kinds(&scanner().scan_file("a.md", &format!("{secret}\n{id}"))),
        ["Google client secret", "Google client id"]
    );
}

#[test]
fn ignores_clean_text_and_placeholders() {
    let text = "Send `Authorization: Bearer <token>`\nbearer_auth(session.bearer_token.as_str())\n\
                eyJ is how a JWT starts\nGOCSPX-short";
    assert_eq!(scanner().scan_file("a.md", text), []);
}

#[test]
fn reports_path_and_line_but_not_the_secret() {
    let findings = scanner().scan_file("dir/a.md", &format!("clean\n{}", fake_jwt()));
    assert_eq!(
        findings,
        [Finding {
            path: "dir/a.md".to_string(),
            line: 2,
            kind: "JWT",
        }]
    );
}

#[test]
fn allowlist_covers_only_the_scanner_sources_and_only_patterns() {
    let value = ["user", "-value-", "1234"].concat();
    let scanner = Scanner::new(vec![value.clone()]);
    let text = format!("{}\n{value}", fake_jwt());

    assert_eq!(
        kinds(&scanner.scan_file("xtask/src/secret_scan.tests.rs", &text)),
        ["value from the user's config or session"]
    );
    assert_eq!(scanner.scan_file("xtask/src/other.rs", &text).len(), 2);
    assert_eq!(scanner.scan_file("docs/secret_scan.md", &text).len(), 2);
}

#[test]
fn diff_scan_checks_only_added_lines_with_new_file_line_numbers() {
    let jwt = fake_jwt();
    let diff = format!(
        "diff --git a/old.md b/new.md\n\
         --- a/old.md\n\
         +++ b/new.md\n\
         @@ -1 +0,0 @@\n\
         -{jwt}\n\
         @@ -7,0 +8,2 @@ context\n\
         +clean\n\
         +{jwt}\n"
    );
    assert_eq!(
        scanner().scan_diff(&diff),
        [Finding {
            path: "new.md".to_string(),
            line: 9,
            kind: "JWT",
        }]
    );
}

#[test]
fn diff_scan_does_not_take_added_lines_for_file_headers() {
    let diff = format!(
        "diff --git a/a.md b/a.md\n\
         --- a/a.md\n\
         +++ b/a.md\n\
         @@ -0,0 +1,2 @@\n\
         +++ b/fake.md\n\
         +{}\n",
        fake_jwt()
    );
    let findings = scanner().scan_diff(&diff);
    assert_eq!(findings[0].path, "a.md");
    assert_eq!(findings[0].line, 2);
}

#[test]
fn diff_scan_tracks_paths_across_files_and_applies_the_allowlist() {
    let jwt = fake_jwt();
    let diff = format!(
        "diff --git a/xtask/src/secret_scan.rs b/xtask/src/secret_scan.rs\n\
         +++ b/xtask/src/secret_scan.rs\n\
         @@ -0,0 +1 @@\n\
         +{jwt}\n\
         diff --git a/b.md b/b.md\n\
         +++ b/b.md\n\
         @@ -0,0 +1 @@\n\
         +{jwt}\n"
    );
    let findings = scanner().scan_diff(&diff);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "b.md");
}

#[test]
fn diff_scan_still_scans_after_an_unparsable_hunk_header() {
    let diff = format!(
        "diff --git a/a.md b/a.md\n+++ b/a.md\n@@ garbage @@\n+{}\n",
        fake_jwt()
    );
    assert_eq!(kinds(&scanner().scan_diff(&diff)), ["JWT"]);
}

#[test]
fn collects_only_long_enough_strings_at_any_depth() {
    let value = serde_json::json!({
        "short": "abc",
        "number": 1234567890,
        "nested": { "secret": "long-enough-value", "list": ["another-value"] }
    });
    let mut values = Vec::new();
    collect_strings(&value, &mut values);
    values.sort();
    assert_eq!(values, ["another-value", "long-enough-value"]);
}
