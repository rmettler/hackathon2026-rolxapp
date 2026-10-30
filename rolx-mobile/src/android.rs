//! Android specifics, called via JNI.

use std::path::PathBuf;

use anyhow::Context;
use jni::JavaVM;
use jni::objects::{JObject, JString};

// Bundles the Gradle module with the launcher icon (`@mipmap/ic_rolx`, set in `AndroidManifest.xml`): dx 0.7 has no
// icon setting for Android and overwrites its default `ic_launcher`, but adds the modules of FFI plugins to the app.
#[manganis::ffi("android/launcher-icon")]
extern "Kotlin" {}

/// Returns the app's private files dir (`Context.getFilesDir()`), which survives app updates.
pub fn files_dir() -> anyhow::Result<PathBuf> {
    let ctx = ndk_context::android_context();
    // SAFETY: ndk-context holds the VM and the activity for the whole app lifetime.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    let dir = env
        .call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])?
        .l()?;
    let path = env
        .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
        .l()?;
    let path: String = env
        .get_string(&JString::from(path))
        .context("reading the files dir path")?
        .into();
    Ok(PathBuf::from(path))
}
