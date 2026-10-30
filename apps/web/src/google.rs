//! Google Identity Services: Google's own script draws the button and hands back an id token.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js_sys::{Function, Object, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlScriptElement, Window};

const CLIENT_SCRIPT: &str = "https://accounts.google.com/gsi/client";

/// `window.google.accounts.id`, once Google's script has run.
fn identity_api(window: &Window) -> Option<JsValue> {
    let mut value: JsValue = window.into();
    for key in ["google", "accounts", "id"] {
        value = Reflect::get(&value, &JsValue::from_str(key)).ok()?;
        if value.is_undefined() {
            return None;
        }
    }
    Some(value)
}

fn call(api: &JsValue, method: &str, args: &[&JsValue]) -> Result<(), JsValue> {
    let function: Function = Reflect::get(api, &JsValue::from_str(method))?.dyn_into()?;
    match args {
        [one] => function.call1(api, one)?,
        [one, two] => function.call2(api, one, two)?,
        _ => return Err(JsValue::from_str("unsupported arguments")),
    };
    Ok(())
}

fn object(entries: &[(&str, &JsValue)]) -> Result<Object, JsValue> {
    let object = Object::new();
    for (key, value) in entries {
        Reflect::set(&object, &JsValue::from_str(key), value)?;
    }
    Ok(object)
}

type CredentialHandler = Rc<dyn Fn(String)>;

thread_local! {
    /// Who gets the next credential. `initialize` may be called only once per page, so its callback
    /// stays and this is what a new login screen replaces.
    static ON_CREDENTIAL: RefCell<Option<CredentialHandler>> = const { RefCell::new(None) };
    static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    static SCRIPT_REQUESTED: Cell<bool> = const { Cell::new(false) };
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

/// The login screen waiting for Google's script.
struct Pending {
    element: Element,
    client_id: String,
    on_credential: CredentialHandler,
    failed: Rc<dyn Fn()>,
}

fn initialize_once(api: &JsValue, client_id: &str) -> Result<(), JsValue> {
    if INITIALIZED.get() {
        return Ok(());
    }
    let callback = Closure::<dyn Fn(JsValue)>::new(move |response: JsValue| {
        let credential = Reflect::get(&response, &JsValue::from_str("credential"))
            .ok()
            .and_then(|value| value.as_string());
        let handler = ON_CREDENTIAL.with_borrow(Clone::clone);
        if let (Some(credential), Some(handler)) = (credential, handler) {
            handler(credential);
        }
    });
    let options = object(&[
        ("client_id", &JsValue::from_str(client_id)),
        ("callback", callback.as_ref()),
    ])?;
    call(api, "initialize", &[&options])?;
    // It lives as long as the page: Google keeps it.
    callback.forget();
    INITIALIZED.set(true);
    Ok(())
}

/// Sets the API up if needed, and draws the button.
fn render_button(
    window: &Window,
    element: &Element,
    client_id: &str,
    on_credential: CredentialHandler,
) -> Result<(), JsValue> {
    let api = identity_api(window).ok_or_else(|| JsValue::from_str("no Google API"))?;
    ON_CREDENTIAL.set(Some(on_credential));
    initialize_once(&api, client_id)?;

    let look = object(&[
        ("theme", &JsValue::from_str("outline")),
        ("size", &JsValue::from_str("large")),
    ])?;
    call(&api, "renderButton", &[element, &look])
}

fn load_script(
    document: &Document,
    on_load: impl FnOnce() + 'static,
    on_error: impl FnOnce() + 'static,
) -> Result<(), JsValue> {
    let script: HtmlScriptElement = document.create_element("script")?.dyn_into()?;
    script.set_src(CLIENT_SCRIPT);
    script.set_async(true);
    script.set_onload(Some(
        Closure::once_into_js(on_load).unchecked_ref::<Function>(),
    ));
    script.set_onerror(Some(
        Closure::once_into_js(on_error).unchecked_ref::<Function>(),
    ));
    document
        .head()
        .ok_or_else(|| JsValue::from_str("no document head"))?
        .append_child(&script)?;
    Ok(())
}

/// Shows Google's button in the element `element_id`, loading Google's script if needed.
/// `on_error` gets a text for the user if that fails.
pub fn mount_button(
    element_id: &str,
    client_id: &str,
    on_credential: CredentialHandler,
    on_error: Rc<dyn Fn(String)>,
) {
    let failed = {
        let on_error = on_error.clone();
        move || on_error("Google's sign-in could not be loaded.".to_string())
    };
    let Some(window) = web_sys::window() else {
        return failed();
    };
    let Some(document) = window.document() else {
        return failed();
    };
    let Some(element) = document.get_element_by_id(element_id) else {
        return failed();
    };

    if identity_api(&window).is_some() {
        if render_button(&window, &element, client_id, on_credential).is_err() {
            failed();
        }
        return;
    }

    // Only one script per page. Screens that come while it loads replace the waiting one: the
    // earlier element is gone by then.
    PENDING.set(Some(Pending {
        element,
        client_id: client_id.to_string(),
        on_credential,
        failed: Rc::new(failed.clone()),
    }));
    if SCRIPT_REQUESTED.replace(true) {
        return;
    }
    let requested = load_script(
        &document,
        move || {
            if let Some(pending) = PENDING.take()
                && render_button(
                    &window,
                    &pending.element,
                    &pending.client_id,
                    pending.on_credential,
                )
                .is_err()
            {
                (pending.failed)();
            }
        },
        || {
            SCRIPT_REQUESTED.set(false);
            if let Some(pending) = PENDING.take() {
                (pending.failed)();
            }
        },
    );
    if requested.is_err() {
        SCRIPT_REQUESTED.set(false);
        PENDING.take();
        failed();
    }
}
