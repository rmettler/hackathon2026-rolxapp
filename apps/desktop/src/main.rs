mod platform;

use dioxus::native::{Config, WindowAttributes};
use dioxus::prelude::*;
use rolx_ui::{App, PlatformHandle};

use platform::DesktopPlatform;

fn main() {
    // `new` is falsely deprecated: its "no renderer enabled" check does not know `native`.
    #[allow(deprecated)]
    dioxus::LaunchBuilder::new()
        .with_cfg(
            Config::new().with_window_attributes(WindowAttributes::default().with_title("RolX")),
        )
        .launch(Root);
}

#[component]
fn Root() -> Element {
    let platform = use_hook(|| PlatformHandle::new(DesktopPlatform::load()));
    rsx! { App { platform } }
}
