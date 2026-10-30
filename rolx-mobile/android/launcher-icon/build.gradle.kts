// Resource-only library with the launcher icon (`@mipmap/ic_rolx`) and the notification icon
// (`@drawable/ic_rolx_notification`). dx 0.7 always writes its default `ic_launcher`, so the icons come in as a plugin
// module, declared in `src/android.rs`.
plugins {
    id("com.android.library")
}

android {
    namespace = "ch.cudos.rolxmobile.launchericon"
    compileSdk = 34
    defaultConfig {
        minSdk = 24
    }
}
