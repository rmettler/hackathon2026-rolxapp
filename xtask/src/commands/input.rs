use enigo::{Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};

use super::screenshot::find_window;

#[derive(clap::Subcommand)]
pub enum InputAction {
    /// Click at a position relative to the window's top left corner.
    Click {
        #[arg(long)]
        window: String,
        #[arg(long)]
        x: i32,
        #[arg(long)]
        y: i32,
    },
    /// Type text into the focused control.
    Type { text: String },
    /// Scroll the mouse wheel at the current position; positive is down.
    Scroll { lines: i32 },
}

/// Drives the desktop for GUI checks. Needs an unlocked, awake desktop.
pub fn run(action: &InputAction) -> Result<String, String> {
    // Makes coordinates physical pixels, as in the screenshots. A failure only matters on scaled
    // displays and is reported by the click landing elsewhere.
    let _ = enigo::set_dpi_awareness();
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("no input: {e}"))?;
    match action {
        InputAction::Click { window, x, y } => {
            let window = find_window(window)?;
            let left = window.x().map_err(|e| e.to_string())?;
            let top = window.y().map_err(|e| e.to_string())?;
            enigo
                .move_mouse(left + x, top + y, Coordinate::Abs)
                .map_err(|e| e.to_string())?;
            enigo
                .button(Button::Left, Direction::Click)
                .map_err(|e| e.to_string())?;
            Ok(format!("clicked at {x},{y}"))
        }
        InputAction::Type { text } => {
            enigo.text(text).map_err(|e| e.to_string())?;
            Ok("typed".to_string())
        }
        InputAction::Scroll { lines } => {
            enigo
                .scroll(*lines, enigo::Axis::Vertical)
                .map_err(|e| e.to_string())?;
            Ok(format!("scrolled {lines}"))
        }
    }
}
