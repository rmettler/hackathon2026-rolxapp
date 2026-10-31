//! The bridge to the Kotlin parts in the `plugin` module (`android/plugin`): the home-screen widget, the background
//! upload and the app lock's prompt.
//!
//! Kotlin calls in through `RolxNative`'s native methods, exported here as `Java_…` functions. Those may run without
//! the app's activity (the widget, a background upload, in a process the app never started in), so they get the
//! context from Kotlin, set up the storage themselves and answer with JSON, never a panic or exception. The app calls
//! out to Kotlin with [`refresh_widgets`] and friends, through the activity (`android::with_activity`).

use std::panic::{AssertUnwindSafe, catch_unwind};

use chrono::Local;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JValue};
use jni::sys::jstring;
use rolx_core::activities::{self, Activity};
use rolx_core::stamp::{StampState, Toggled};
use serde_json::{Value, json};

use crate::android;
use crate::native;
use crate::widget::widget_view;

const WIDGET: &str = "ch.cudos.rolxmobile.plugin.StampWidget";
const UPLOAD: &str = "ch.cudos.rolxmobile.plugin.BackgroundUpload";
const LOCK: &str = "ch.cudos.rolxmobile.plugin.AppLock";

/// `AppLock.status()` values.
pub const UNLOCK_PENDING: i32 = 0;
pub const UNLOCKED: i32 = 1;

/// Runs `f` for a call from Kotlin: with `context` in place of the activity, the storage in the app's files dir and
/// the embedded config, as `main` would set them up. Returns its JSON; an error or panic becomes
/// `{"result": "error", "error": …}`.
fn respond(
    env: &mut JNIEnv,
    context: &JObject,
    f: impl FnOnce() -> anyhow::Result<Value>,
) -> jstring {
    let result = catch_unwind(AssertUnwindSafe(|| {
        android::with_context(env, context, || {
            let dir = android::files_dir()?;
            // Set already when the app runs in this process; it is the same dir and config then.
            _ = rolx_core::paths::set_base_dir(dir);
            _ = rolx_ui::set_config_source(crate::config);
            f()
        })
    }));
    let value = match result {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => {
            json!({ "result": "error", "error": e.to_string(), "message": e.to_string() })
        }
        Err(_) => {
            json!({ "result": "error", "error": "the Rust library panicked", "message": "internal error" })
        }
    };
    env.new_string(value.to_string())
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}

/// `RolxNative.widgetState(context)`: [`widget_view`] of the stamps on disk.
#[unsafe(no_mangle)]
pub extern "system" fn Java_ch_cudos_rolxmobile_plugin_RolxNative_widgetState<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) -> jstring {
    respond(&mut env, &context, || {
        let state = StampState::load()?;
        let activities = activities::load_cached().unwrap_or_default();
        let view = widget_view(&state, Local::now().naive_local(), &activities);
        Ok(serde_json::to_value(view)?)
    })
}

/// `RolxNative.toggle(context)`: the widget's button. Stamps out when stamped in, else in on the activity used last
/// (like the desktop hotkey), and updates the notification. No vibration: Android ignores it from an app in the
/// background, which the app is while its widget is tapped.
/// `{"result": "in" | "out" | "stale", "upload": bool}`; `stale` means it refused to end a forgotten session.
#[unsafe(no_mangle)]
pub extern "system" fn Java_ch_cudos_rolxmobile_plugin_RolxNative_toggle<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) -> jstring {
    respond(&mut env, &context, || {
        let now = Local::now().naive_local();
        let activities = activities::load_cached().unwrap_or_default();
        let (state, toggled) = StampState::update(|s| {
            if s.stale(now).is_some() {
                return Ok(None);
            }
            let toggled = s.toggle(now, |id| {
                activities.iter().find(|a| a.id == id).map(Activity::to_ref)
            })?;
            Ok(Some(toggled))
        })?;
        let Some(toggled) = toggled else {
            return Ok(json!({ "result": "stale", "upload": false }));
        };
        // Without the permission the app would ask for it, which needs the activity; the app does that when opened.
        if android::notifications_allowed().unwrap_or(false) {
            log_error(
                "updating the stamp notification",
                native::update_stamp_notification(&state),
            );
        }
        let result = match toggled {
            Toggled::StampedIn => "in",
            Toggled::StampedOut => "out",
        };
        let upload = toggled == Toggled::StampedOut && state.uploadable_count() > 0;
        Ok(json!({ "result": result, "upload": upload }))
    })
}

/// `RolxNative.upload(context)`: the background upload. `{"result": "done" | "retry" | "login", "uploaded": n,
/// "waiting": n, "message": …}`; `waiting` counts the entries with an activity still queued, `login` means there is no
/// valid session to upload them with.
#[unsafe(no_mangle)]
pub extern "system" fn Java_ch_cudos_rolxmobile_plugin_RolxNative_upload<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) -> jstring {
    respond(&mut env, &context, upload)
}

fn upload() -> anyhow::Result<Value> {
    let waiting = StampState::load()?.uploadable_count();
    if waiting == 0 {
        return Ok(json!({ "result": "done", "uploaded": 0, "waiting": 0 }));
    }
    if !rolx_ui::shared::has_session() {
        return Ok(json!({ "result": "login", "uploaded": 0, "waiting": waiting }));
    }
    // Called on a WorkManager thread, which has no runtime.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let client = rolx_ui::shared::client().await?;
        rolx_core::sync::run_default(&client).await
    });
    let waiting = StampState::load().map_or(waiting, |s| s.uploadable_count());
    Ok(match result {
        Ok(report) => match report.failed.first() {
            None => json!({ "result": "done", "uploaded": report.uploaded, "waiting": waiting }),
            Some((day, reason)) => json!({
                "result": "retry",
                "uploaded": report.uploaded,
                "waiting": waiting,
                "message": format!("{day} not uploaded: {reason}"),
            }),
        },
        Err(e) => {
            json!({ "result": "retry", "uploaded": 0, "waiting": waiting, "message": e.to_string() })
        }
    })
}

fn log_error(what: &str, result: anyhow::Result<()>) {
    if let Err(e) = result {
        dioxus::logger::tracing::warn!(error = %e, "{what}");
    }
}

/// Calls the static `class.method(Context)` of the `plugin` module.
fn call_with_context(class: &str, method: &str) -> anyhow::Result<()> {
    android::with_activity(|env, activity| {
        let class = android::app_class(env, activity, class)?;
        env.call_static_method(
            &class,
            method,
            "(Landroid/content/Context;)V",
            &[JValue::Object(activity)],
        )?;
        Ok(())
    })
}

/// Redraws the home-screen widgets from the stamps on disk.
pub fn refresh_widgets() -> anyhow::Result<()> {
    call_with_context(WIDGET, "refreshAll")
}

/// Has WorkManager upload the queue once a network is there, also if the app is closed by then.
pub fn schedule_upload() -> anyhow::Result<()> {
    call_with_context(UPLOAD, "schedule")
}

/// Sets up the hourly background upload (kept if it is set up already).
pub fn start_uploads() -> anyhow::Result<()> {
    call_with_context(UPLOAD, "start")
}

/// Whether the phone has a fingerprint, face or screen lock to unlock the app with.
pub fn lock_available() -> anyhow::Result<bool> {
    android::with_activity(|env, activity| {
        let class = android::app_class(env, activity, LOCK)?;
        Ok(env
            .call_static_method(
                &class,
                "available",
                "(Landroid/app/Activity;)Z",
                &[JValue::Object(activity)],
            )?
            .z()?)
    })
}

/// Shows the system's unlock prompt; poll [`unlock_status`] for the answer.
pub fn request_unlock(title: &str, subtitle: &str) -> anyhow::Result<()> {
    android::with_activity(|env, activity| {
        let class = android::app_class(env, activity, LOCK)?;
        let title = env.new_string(title)?;
        let subtitle = env.new_string(subtitle)?;
        env.call_static_method(
            &class,
            "authenticate",
            "(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(activity),
                JValue::Object(&title),
                JValue::Object(&subtitle),
            ],
        )?;
        Ok(())
    })
}

/// [`UNLOCK_PENDING`] while the prompt is open, then [`UNLOCKED`] or another value when it was cancelled.
pub fn unlock_status() -> anyhow::Result<i32> {
    android::with_activity(|env, activity| {
        let class = android::app_class(env, activity, LOCK)?;
        Ok(env.call_static_method(&class, "status", "()I", &[])?.i()?)
    })
}
