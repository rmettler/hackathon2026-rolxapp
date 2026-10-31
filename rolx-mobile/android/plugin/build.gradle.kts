// The app's Android parts that `dx` 0.7 cannot generate, added as an FFI plugin module (declared in `src/android.rs`;
// dx names the module `plugin` and supports only one without a declared type):
// - the launcher icon (`@mipmap/ic_rolx`) and the notification icon (`@drawable/ic_rolx_notification`), since dx
//   always writes its default `ic_launcher`;
// - the home-screen widget (`StampWidget`), the background upload (`UploadWorker`) and the app lock (`AppLock`), in
//   Kotlin. They call into the Rust library through `RolxNative`, see `src/bridge.rs`.
// The receivers and services they need are declared in this module's `AndroidManifest.xml`, merged into the app's.
plugins {
    id("com.android.library")
    // Its version comes from the buildscript classpath of dx's Gradle project.
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "ch.cudos.rolxmobile.plugin"
    compileSdk = 34
    defaultConfig {
        minSdk = 24
        // Release builds shrink the app; these rules keep what only Rust calls through JNI.
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // Newer versions need compileSdk 35, which dx 0.7 does not use.
    implementation("androidx.work:work-runtime:2.9.1")
    implementation("androidx.biometric:biometric:1.1.0")
    implementation("androidx.core:core:1.13.1")
}
