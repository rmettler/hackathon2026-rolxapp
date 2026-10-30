// Resource-only library with the launcher icon (`@mipmap/ic_rolx`). dx 0.7 always writes its default
// `ic_launcher`, so the icon comes in as a plugin module, declared in `src/android.rs`.
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
