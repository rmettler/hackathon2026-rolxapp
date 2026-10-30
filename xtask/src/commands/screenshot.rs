use std::fs;
use std::path::Path;

use xcap::Window;

/// Finds the visible window whose title contains `title`.
pub fn find_window(title: &str) -> Result<Window, String> {
    let windows = Window::all().map_err(|e| format!("cannot list windows: {e}"))?;
    windows
        .into_iter()
        .find(|w| !w.is_minimized().unwrap_or(true) && w.title().is_ok_and(|t| t.contains(title)))
        .ok_or_else(|| format!("no visible window with a title containing `{title}`"))
}

/// Saves a PNG of the window, for checks by eye. Needs an unlocked, awake desktop.
pub fn run(title: &str, out: &Path) -> Result<String, String> {
    let image = find_window(title)?
        .capture_image()
        .map_err(|e| format!("cannot capture the window: {e}"))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    image
        .save(out)
        .map_err(|e| format!("cannot save {}: {e}", out.display()))?;
    Ok(format!(
        "saved {} ({}x{})",
        out.display(),
        image.width(),
        image.height()
    ))
}
