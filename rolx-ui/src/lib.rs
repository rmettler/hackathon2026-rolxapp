//! Dioxus components shared by the desktop and the mobile app: stamping, the activity picker, the
//! day view and the week panel. The apps add the platform parts (window, tray, login, layout).

use std::sync::OnceLock;

use dioxus::prelude::*;
use rolx_core::config::AppConfig;

pub mod activity_picker;
pub mod app_state;
pub mod day_view;
pub mod lucide;
pub mod shared;
pub mod stamp_panel;
pub mod stamper;
pub mod undo;
pub mod week_panel;

/// The styles the components need: the Cudos design tokens, then the skin built on them. Render it with
/// `style { "{rolx_ui::CSS}" }` before the app's own.
pub const CSS: &str = concat!(include_str!("tokens.css"), "\n", include_str!("style.css"));

type ConfigSource = fn() -> anyhow::Result<AppConfig>;

static CONFIG_SOURCE: OnceLock<ConfigSource> = OnceLock::new();

/// Makes the components read the config from `source` instead of `AppConfig::load_with_env`, e.g.
/// a config embedded in the app. Must be called before the first component needs the server;
/// returns `source` back as error if a source was already set.
pub fn set_config_source(source: ConfigSource) -> Result<(), ConfigSource> {
    CONFIG_SOURCE.set(source)
}

/// The config, from the source set with `set_config_source` or else from the config file.
pub fn config() -> anyhow::Result<AppConfig> {
    match CONFIG_SOURCE.get() {
        Some(source) => source(),
        None => Ok(AppConfig::load_with_env()?),
    }
}

/// Provide `Compact(true)` as context to have the components use a layout for narrow screens.
/// Without it they keep the roomy desktop layout.
#[derive(Clone, Copy, PartialEq)]
pub struct Compact(pub bool);

/// Whether the narrow-screen layout was asked for (see `Compact`). A hook: call it in a component.
pub fn use_compact() -> bool {
    try_use_context::<Compact>().is_some_and(|c| c.0)
}
