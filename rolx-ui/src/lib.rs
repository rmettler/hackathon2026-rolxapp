//! Dioxus components shared by the desktop and the mobile app: stamping, the activity picker and
//! the day view. The apps add the platform parts (window, tray, login, layout).

pub mod activity_picker;
pub mod day_view;
pub mod shared;
pub mod stamp_panel;
pub mod stamper;

/// The styles the components need; render it with `style { "{rolx_ui::CSS}" }` before the app's own.
pub const CSS: &str = include_str!("style.css");
