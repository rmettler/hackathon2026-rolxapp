mod platform;

use dioxus::prelude::*;
use rolx_ui::{App, PlatformHandle};

use platform::AndroidPlatform;

fn main() {
    dioxus::launch(Root);
}

#[component]
fn Root() -> Element {
    let platform = use_hook(|| PlatformHandle::new(AndroidPlatform::default()));
    rsx! { App { platform } }
}
