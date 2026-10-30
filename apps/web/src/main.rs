mod google;
mod platform;

use dioxus::prelude::*;
use rolx_ui::{App, PlatformHandle};

use platform::{WebConfig, WebPlatform};

fn main() {
    dioxus::launch(Root);
}

#[component]
fn Root() -> Element {
    let config = use_resource(platform::load_config);
    let current = config.read().clone();
    match current {
        // The login screen depends on it, and it is a small file on the same host.
        None => rsx! {},
        Some(config) => rsx! { Loaded { config } },
    }
}

#[component]
fn Loaded(config: WebConfig) -> Element {
    let platform = use_hook(|| PlatformHandle::new(WebPlatform::new(config)));
    rsx! { App { platform } }
}
