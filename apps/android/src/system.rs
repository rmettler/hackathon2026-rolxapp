//! What the WebView cannot do, through the Android activity.

use std::sync::mpsc::channel;
use std::time::Duration;

use dioxus::mobile::wry::prelude::dispatch;
use jni::JNIEnv;
use jni::objects::{JObject, JValue};

/// Runs `action` on the main thread and waits for the outcome, which is safe as the UI code runs on
/// another thread. A pending Java exception, say because
/// no app can open a URL, counts as failure and is cleared, as it would otherwise crash the app.
fn on_activity(
    action: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<()> + Send + 'static,
) -> bool {
    let (tx, rx) = channel();
    dispatch(move |env: &mut JNIEnv, activity: &JObject, _webview| {
        let result = action(env, activity);
        let threw = env.exception_check().unwrap_or(true);
        if threw {
            let _ = env.exception_clear();
        }
        let _ = tx.send(result.is_ok() && !threw);
    });
    rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
}

/// Opens `url` in the system browser, outside the app's WebView. Only web addresses: the URL comes
/// from a server's answer, and another scheme would start some other app.
pub fn open_in_browser(url: &str) -> bool {
    let lowercase = url.to_ascii_lowercase();
    if !lowercase.starts_with("https://") && !lowercase.starts_with("http://") {
        return false;
    }
    let url = url.to_string();
    on_activity(move |env, activity| {
        let action = env.new_string("android.intent.action.VIEW")?;
        let url = env.new_string(url)?;
        let uri = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&url)],
            )?
            .l()?;
        let intent = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;Landroid/net/Uri;)V",
            &[JValue::Object(&action), JValue::Object(&uri)],
        )?;
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        )?;
        Ok(())
    })
}

pub fn copy_to_clipboard(text: &str) -> bool {
    let text = text.to_string();
    on_activity(move |env, activity| {
        let service = env.new_string("clipboard")?;
        let manager = env
            .call_method(
                activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&service)],
            )?
            .l()?;
        let label = env.new_string("RolX")?;
        let text = env.new_string(text)?;
        let clip = env
            .call_static_method(
                "android/content/ClipData",
                "newPlainText",
                "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
                &[JValue::Object(&label), JValue::Object(&text)],
            )?
            .l()?;
        env.call_method(
            manager,
            "setPrimaryClip",
            "(Landroid/content/ClipData;)V",
            &[JValue::Object(&clip)],
        )?;
        Ok(())
    })
}
