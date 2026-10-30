mod platform;

use dioxus::prelude::*;
use rolx_ui::{App, PlatformHandle};

use platform::WebPlatform;

fn main() {
    dioxus::launch(Root);
}

#[component]
fn Root() -> Element {
    let platform = use_hook(|| PlatformHandle::new(WebPlatform::default()));
    let config = use_resource(platform::load_config);
    if let Some(config) = &*config.read() {
        tracing::debug!(
            has_google_client = config.google_client_id.is_some(),
            "web config"
        );
    }
    rsx! { App { platform } }
}
