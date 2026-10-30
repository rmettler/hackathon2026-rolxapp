use std::path::Path;

use super::*;

#[test]
fn override_replaces_the_default_dir() {
    let dir = resolve_dir(Some("over".into()), Some("base".into()));
    assert_eq!(dir, Some(PathBuf::from("over")));
}

#[test]
fn missing_or_empty_override_uses_rolx_below_the_default_base() {
    let expected = Some(Path::new("base").join("rolx"));
    assert_eq!(resolve_dir(None, Some("base".into())), expected);
    assert_eq!(resolve_dir(Some("".into()), Some("base".into())), expected);
}

#[test]
fn no_dir_without_override_and_default() {
    assert_eq!(resolve_dir(None, None), None);
}
