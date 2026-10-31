// The app's Android parts that `dx` 0.7 cannot generate, added as an FFI plugin module (declared in `src/android.rs`):
// - the launcher icon (`@mipmap/ic_rolx`) and the notification icon (`@drawable/ic_rolx_notification`), since dx
//   always writes its default `ic_launcher`;
// - the icons of the app shortcuts (`@drawable/ic_shortcut_*`);
// - `StampLinkActivity`, which receives the stamp links (`rolx://stamp?...`) for the Rust app.
plugins {
    id("com.android.library")
}

android {
    namespace = "ch.cudos.rolxmobile.plugin"
    compileSdk = 34
    defaultConfig {
        minSdk = 24
    }
}
