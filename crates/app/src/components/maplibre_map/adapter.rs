use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

const ADAPTER_GLOBAL: &str = "exerciseMapLibre";

/// Why the adapter (public/maplibre.js) rejected a call. The adapter names
/// these on `error.reason`; this enum is the typed Rust view of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AdapterReason {
    /// The maplibre-gl module could not be fetched from the CDN.
    Module,
    /// The map could not be constructed in this browser.
    Init,
    /// A pending load was cancelled through `removeMap`.
    Removed,
    /// Anything else the adapter reports.
    Other,
}

impl AdapterReason {
    pub(crate) fn from_js(error: &JsValue) -> Self {
        match failure_reason(error).as_deref() {
            Some("map-module") => Self::Module,
            Some("map-init") => Self::Init,
            Some("removed") => Self::Removed,
            _ => Self::Other,
        }
    }
}

pub(crate) fn maplibre_api() -> Result<JsValue, JsValue> {
    Reflect::get(&js_sys::global(), &JsValue::from_str(ADAPTER_GLOBAL))
}

fn api_function(api: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(api, &JsValue::from_str(name))?.dyn_into::<Function>()
}

pub(crate) fn remove_map(map: &JsValue) {
    if let Ok(api) = maplibre_api()
        && let Ok(remove) = api_function(&api, "removeMap")
    {
        let _ = remove.call1(&api, map);
    }
}

/// Calls adapter API `name` with one argument, wraps the returned promise,
/// and awaits it.
pub(crate) async fn await_api_1(name: &str, arg: &JsValue) -> Result<JsValue, JsValue> {
    api_promise_1(name, arg)?.await
}

/// Calls adapter API `name` with two arguments, wraps the returned promise,
/// and awaits it.
pub(crate) async fn await_api_2(
    name: &str,
    first: &JsValue,
    second: &JsValue,
) -> Result<JsValue, JsValue> {
    api_promise_2(name, first, second)?.await
}

/// Calls adapter API `name` with one argument and wraps the returned promise.
pub(crate) fn api_promise_1(name: &str, arg: &JsValue) -> Result<JsFuture, JsValue> {
    let api = maplibre_api()?;
    let function = api_function(&api, name)?;
    Ok(JsFuture::from(Promise::resolve(
        &function.call1(&api, arg)?,
    )))
}

/// Calls adapter API `name` with two arguments and wraps the returned promise.
pub(crate) fn api_promise_2(
    name: &str,
    first: &JsValue,
    second: &JsValue,
) -> Result<JsFuture, JsValue> {
    let api = maplibre_api()?;
    let function = api_function(&api, name)?;
    Ok(JsFuture::from(Promise::resolve(
        &function.call2(&api, first, second)?,
    )))
}

/// The machine reason the adapter attached to a rejection
/// (set on `error.reason` by public/maplibre.js).
fn failure_reason(error: &JsValue) -> Option<String> {
    Reflect::get(error, &JsValue::from_str("reason"))
        .ok()
        .and_then(|value| value.as_string())
}

/// One-shot timeout future backing the load watchdog. Uses `setTimeout` from
/// the global object so no extra timer crate is needed.
pub(crate) fn timeout_future(ms: u32) -> JsFuture {
    JsFuture::from(Promise::new(&mut |resolve, _reject| {
        let set_timeout = Reflect::get(&js_sys::global(), &JsValue::from_str("setTimeout"))
            .ok()
            .and_then(|f| f.dyn_into::<Function>().ok());
        if let Some(set_timeout) = set_timeout {
            let fire = Closure::once_into_js(move || {
                let _ = resolve.call0(&JsValue::NULL);
            });
            let _ = set_timeout.call2(&js_sys::global(), &fire, &ms.into());
        }
    }))
}
