//! Android specifics, called via JNI.
//!
//! Every call goes through [`with_activity`], which attaches the thread, frees the local references the call creates
//! and turns a Java exception into an error (clearing it, so the next JNI call works).

use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};
use jni::JNIEnv;
use jni::JavaVM;
use jni::objects::{JObject, JString, JValue};

// Bundles the Gradle module with the launcher icon (`@mipmap/ic_rolx`, set in `AndroidManifest.xml`) and the
// notification icon: dx 0.7 has no icon setting for Android and overwrites its default `ic_launcher`, but adds the
// modules of FFI plugins to the app.
#[manganis::ffi("android/launcher-icon")]
extern "Kotlin" {}

/// Local references a call may hold at once; the frame frees them all when the call returns.
const LOCAL_FRAME: i32 = 32;

/// `NotificationManager.notify` id of the stamp notification; there is only one.
const STAMP_NOTIFICATION_ID: i32 = 1;
const STAMP_CHANNEL_ID: &str = "stamp";
const STAMP_CHANNEL_NAME: &str = "Stamped in";
const STAMP_CHANNEL_DESCRIPTION: &str = "Shows the running session while you are stamped in";
/// Drawable in the `launcher-icon` module, looked up by name since there is no generated `R` class to refer to.
const NOTIFICATION_ICON: &str = "ic_rolx_notification";
const POST_NOTIFICATIONS: &str = "android.permission.POST_NOTIFICATIONS";
/// Request code of the permission request; nothing listens for the result, the app polls the permission instead.
const PERMISSION_REQUEST_CODE: i32 = 4711;

// Android constants (API levels and flag values of the SDK).
const API_O: i32 = 26;
const API_TIRAMISU: i32 = 33;
const IMPORTANCE_LOW: i32 = 2;
const PRIORITY_LOW: i32 = -1;
const PERMISSION_GRANTED: i32 = 0;
const FLAG_UPDATE_CURRENT: i32 = 0x0800_0000;
const FLAG_IMMUTABLE: i32 = 0x0400_0000;

const BUILDER: &str = "android/app/Notification$Builder";

/// Runs `f` with the JNI env of the current thread (attached if needed) and the app's activity, in a local reference
/// frame that frees the references `f` creates. A Java exception thrown meanwhile is cleared and returned as error.
fn with_activity<T>(
    f: impl FnOnce(&mut JNIEnv, &JObject) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let ctx = ndk_context::android_context();
    // SAFETY: ndk-context holds the VM and the activity for the whole app lifetime.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };

    env.with_local_frame(LOCAL_FRAME, |env| {
        let result = f(env, &activity);
        match take_exception(env) {
            None => result,
            Some(exception) => Err(match result {
                Err(e) => anyhow!("{exception}").context(e.to_string()),
                Ok(_) => anyhow!("{exception}"),
            }),
        }
    })
}

/// Clears a pending Java exception and describes it (`Throwable.toString()`), if there is one.
fn take_exception(env: &mut JNIEnv) -> Option<String> {
    if !env.exception_check().unwrap_or(true) {
        return None;
    }
    let throwable = env.exception_occurred();
    _ = env.exception_clear();
    let text = throwable.ok().and_then(|t| to_string(env, &t).ok());
    // `toString` itself may have thrown.
    _ = env.exception_clear();
    Some(text.unwrap_or_else(|| "unknown Java exception".into()))
}

/// `obj.toString()`.
fn to_string(env: &mut JNIEnv, obj: &JObject) -> anyhow::Result<String> {
    let text = env
        .call_method(obj, "toString", "()Ljava/lang/String;", &[])?
        .l()?;
    Ok(env.get_string(&JString::from(text))?.into())
}

/// `Build.VERSION.SDK_INT`, the API level of the device.
fn sdk_int(env: &mut JNIEnv) -> anyhow::Result<i32> {
    Ok(env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()?)
}

/// `ctx.getSystemService(name)`; fails if the service does not exist.
fn system_service<'local>(
    env: &mut JNIEnv<'local>,
    ctx: &JObject,
    name: &str,
) -> anyhow::Result<JObject<'local>> {
    let name_obj = env.new_string(name)?;
    let service = env
        .call_method(
            ctx,
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&name_obj)],
        )?
        .l()?;
    if service.is_null() {
        bail!("no system service `{name}`");
    }
    Ok(service)
}

/// `ctx.getPackageName()`.
fn package_name<'local>(
    env: &mut JNIEnv<'local>,
    ctx: &JObject,
) -> anyhow::Result<JObject<'local>> {
    Ok(env
        .call_method(ctx, "getPackageName", "()Ljava/lang/String;", &[])?
        .l()?)
}

/// Returns the app's private files dir (`Context.getFilesDir()`), which survives app updates.
pub fn files_dir() -> anyhow::Result<PathBuf> {
    with_activity(|env, activity| {
        let dir = env
            .call_method(activity, "getFilesDir", "()Ljava/io/File;", &[])?
            .l()?;
        let path = env
            .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
            .l()?;
        let path: String = env
            .get_string(&JString::from(path))
            .context("reading the files dir path")?
            .into();
        Ok(PathBuf::from(path))
    })
}

/// Whether the app may post notifications: always below Android 13, else when `POST_NOTIFICATIONS` is granted.
pub fn notifications_allowed() -> anyhow::Result<bool> {
    with_activity(notifications_granted)
}

fn notifications_granted(env: &mut JNIEnv, ctx: &JObject) -> anyhow::Result<bool> {
    if sdk_int(env)? < API_TIRAMISU {
        return Ok(true);
    }
    let permission = env.new_string(POST_NOTIFICATIONS)?;
    let state = env
        .call_method(
            ctx,
            "checkSelfPermission",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&permission)],
        )?
        .i()?;
    Ok(state == PERMISSION_GRANTED)
}

/// Asks the user for `POST_NOTIFICATIONS` (`Activity.requestPermissions`) unless granted or not needed (below
/// Android 13). Returns right away; the answer is not reported, check [`notifications_allowed`] later.
pub fn request_notification_permission() -> anyhow::Result<()> {
    with_activity(|env, activity| {
        if notifications_granted(env, activity)? {
            return Ok(());
        }
        let permission = env.new_string(POST_NOTIFICATIONS)?;
        let permissions = env.new_object_array(1, "java/lang/String", &permission)?;
        env.call_method(
            activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[
                JValue::Object(&permissions),
                JValue::Int(PERMISSION_REQUEST_CODE),
            ],
        )?;
        Ok(())
    })
}

/// Shows or updates the ongoing "stamped in" notification. With `chronometer_base_ms` (epoch millis) it shows a
/// running timer counting from then, kept up by the system without the app. Does nothing without the permission;
/// returns whether it was posted.
pub fn show_stamp_notification(
    title: &str,
    text: &str,
    chronometer_base_ms: Option<i64>,
) -> anyhow::Result<bool> {
    with_activity(|env, ctx| {
        if !notifications_granted(env, ctx)? {
            return Ok(false);
        }
        let manager = system_service(env, ctx, "notification")?;
        let builder = notification_builder(env, ctx, &manager)?;

        let icon = small_icon(env, ctx)?;
        builder_set(env, &builder, "setSmallIcon", "I", &[JValue::Int(icon)])?;
        let title = env.new_string(title)?;
        builder_set(
            env,
            &builder,
            "setContentTitle",
            "Ljava/lang/CharSequence;",
            &[JValue::Object(&title)],
        )?;
        let text = env.new_string(text)?;
        builder_set(
            env,
            &builder,
            "setContentText",
            "Ljava/lang/CharSequence;",
            &[JValue::Object(&text)],
        )?;
        builder_set(env, &builder, "setOngoing", "Z", &[JValue::Bool(1)])?;
        builder_set(env, &builder, "setOnlyAlertOnce", "Z", &[JValue::Bool(1)])?;
        let category = env.new_string("status")?;
        builder_set(
            env,
            &builder,
            "setCategory",
            "Ljava/lang/String;",
            &[JValue::Object(&category)],
        )?;
        let running = chronometer_base_ms.is_some();
        builder_set(
            env,
            &builder,
            "setShowWhen",
            "Z",
            &[JValue::Bool(running.into())],
        )?;
        builder_set(
            env,
            &builder,
            "setUsesChronometer",
            "Z",
            &[JValue::Bool(running.into())],
        )?;
        if let Some(base) = chronometer_base_ms {
            builder_set(env, &builder, "setWhen", "J", &[JValue::Long(base)])?;
        }
        let intent = open_app_intent(env, ctx)?;
        builder_set(
            env,
            &builder,
            "setContentIntent",
            "Landroid/app/PendingIntent;",
            &[JValue::Object(&intent)],
        )?;

        let notification = env
            .call_method(&builder, "build", "()Landroid/app/Notification;", &[])?
            .l()?;
        env.call_method(
            &manager,
            "notify",
            "(ILandroid/app/Notification;)V",
            &[
                JValue::Int(STAMP_NOTIFICATION_ID),
                JValue::Object(&notification),
            ],
        )?;
        Ok(true)
    })
}

/// Removes the "stamped in" notification, if shown.
pub fn cancel_stamp_notification() -> anyhow::Result<()> {
    with_activity(|env, ctx| {
        let manager = system_service(env, ctx, "notification")?;
        env.call_method(
            &manager,
            "cancel",
            "(I)V",
            &[JValue::Int(STAMP_NOTIFICATION_ID)],
        )?;
        Ok(())
    })
}

/// A `Notification.Builder` for the stamp channel, creating the channel first where channels exist (Android 8+).
fn notification_builder<'local>(
    env: &mut JNIEnv<'local>,
    ctx: &JObject,
    manager: &JObject,
) -> anyhow::Result<JObject<'local>> {
    if sdk_int(env)? < API_O {
        let builder = env.new_object(
            BUILDER,
            "(Landroid/content/Context;)V",
            &[JValue::Object(ctx)],
        )?;
        builder_set(
            env,
            &builder,
            "setPriority",
            "I",
            &[JValue::Int(PRIORITY_LOW)],
        )?;
        return Ok(builder);
    }
    create_stamp_channel(env, manager)?;
    let channel_id = env.new_string(STAMP_CHANNEL_ID)?;
    Ok(env.new_object(
        BUILDER,
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[JValue::Object(ctx), JValue::Object(&channel_id)],
    )?)
}

/// Creates the notification channel: low importance, so no sound, no vibration and no heads-up. Creating it again
/// does nothing, and the user's changes to it are kept.
fn create_stamp_channel(env: &mut JNIEnv, manager: &JObject) -> anyhow::Result<()> {
    let id = env.new_string(STAMP_CHANNEL_ID)?;
    let name = env.new_string(STAMP_CHANNEL_NAME)?;
    let channel = env.new_object(
        "android/app/NotificationChannel",
        "(Ljava/lang/String;Ljava/lang/CharSequence;I)V",
        &[
            JValue::Object(&id),
            JValue::Object(&name),
            JValue::Int(IMPORTANCE_LOW),
        ],
    )?;
    let description = env.new_string(STAMP_CHANNEL_DESCRIPTION)?;
    env.call_method(
        &channel,
        "setDescription",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&description)],
    )?;
    env.call_method(&channel, "setShowBadge", "(Z)V", &[JValue::Bool(0)])?;
    let null = JObject::null();
    env.call_method(
        &channel,
        "setSound",
        "(Landroid/net/Uri;Landroid/media/AudioAttributes;)V",
        &[JValue::Object(&null), JValue::Object(&null)],
    )?;
    env.call_method(
        manager,
        "createNotificationChannel",
        "(Landroid/app/NotificationChannel;)V",
        &[JValue::Object(&channel)],
    )?;
    Ok(())
}

/// Calls a `Notification.Builder` setter taking `arg_sig` and drops the builder it returns (itself).
fn builder_set(
    env: &mut JNIEnv,
    builder: &JObject,
    name: &str,
    arg_sig: &str,
    args: &[JValue],
) -> anyhow::Result<()> {
    let sig = format!("({arg_sig})L{BUILDER};");
    let this = env.call_method(builder, name, sig, args)?.l()?;
    env.delete_local_ref(this)?;
    Ok(())
}

/// Resource id of the notification icon, or the app icon if it is missing.
fn small_icon(env: &mut JNIEnv, ctx: &JObject) -> anyhow::Result<i32> {
    let resources = env
        .call_method(
            ctx,
            "getResources",
            "()Landroid/content/res/Resources;",
            &[],
        )?
        .l()?;
    let name = env.new_string(NOTIFICATION_ICON)?;
    let kind = env.new_string("drawable")?;
    let package = package_name(env, ctx)?;
    let id = env
        .call_method(
            &resources,
            "getIdentifier",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)I",
            &[
                JValue::Object(&name),
                JValue::Object(&kind),
                JValue::Object(&package),
            ],
        )?
        .i()?;
    if id != 0 {
        return Ok(id);
    }
    let info = env
        .call_method(
            ctx,
            "getApplicationInfo",
            "()Landroid/content/pm/ApplicationInfo;",
            &[],
        )?
        .l()?;
    Ok(env.get_field(&info, "icon", "I")?.i()?)
}

/// A `PendingIntent` that brings the app to the front, as tapping its launcher icon does.
fn open_app_intent<'local>(
    env: &mut JNIEnv<'local>,
    ctx: &JObject,
) -> anyhow::Result<JObject<'local>> {
    let package_manager = env
        .call_method(
            ctx,
            "getPackageManager",
            "()Landroid/content/pm/PackageManager;",
            &[],
        )?
        .l()?;
    let package = package_name(env, ctx)?;
    let launch = env
        .call_method(
            &package_manager,
            "getLaunchIntentForPackage",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&package)],
        )?
        .l()?;
    if launch.is_null() {
        bail!("the app has no launch intent");
    }
    Ok(env
        .call_static_method(
            "android/app/PendingIntent",
            "getActivity",
            "(Landroid/content/Context;ILandroid/content/Intent;I)Landroid/app/PendingIntent;",
            &[
                JValue::Object(ctx),
                JValue::Int(0),
                JValue::Object(&launch),
                JValue::Int(FLAG_IMMUTABLE | FLAG_UPDATE_CURRENT),
            ],
        )?
        .l()?)
}
