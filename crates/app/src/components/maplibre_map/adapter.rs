use js_sys::{Function, Promise, Reflect};
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

use super::options::MapOptions;

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

/// A live map instance owned by the renderer.
#[derive(Clone)]
pub(crate) struct Map(JsValue);

impl Map {
    fn from_value(value: JsValue) -> Option<Self> {
        (!value.is_null() && !value.is_undefined()).then_some(Self(value))
    }

    /// Builds a map in `container`; resolves once the map object exists,
    /// before its tiles finish loading.
    pub(crate) async fn create<E: Into<JsValue>>(
        container: E,
        options: &MapOptions,
    ) -> Result<Self, AdapterReason> {
        // MapLibre's style validator needs plain objects, not ES Maps.
        let options = options
            .serialize(&Serializer::new().serialize_maps_as_objects(true))
            .map_err(|_| AdapterReason::Other)?;
        let value = await_api_2("createMap", &container.into(), &options)
            .await
            .map_err(|error| AdapterReason::from_js(&error))?;
        Self::from_value(value).ok_or(AdapterReason::Other)
    }

    /// Resolves once the map fires "load"; rejects with `Removed` when
    /// `remove` settles the wait first.
    pub(crate) async fn loaded(&self) -> Result<JsValue, AdapterReason> {
        await_api_1("mapLoaded", &self.0)
            .await
            .map_err(|error| AdapterReason::from_js(&error))
    }

    /// Asks the renderer to destroy the instance and settle any pending
    /// load wait.
    pub(crate) fn remove(&self) {
        if let Ok(api) = maplibre_api()
            && let Ok(remove) = api_function(&api, "removeMap")
        {
            let _ = remove.call1(&api, &self.0);
        }
    }
}

fn maplibre_api() -> Result<JsValue, JsValue> {
    Reflect::get(&js_sys::global(), &JsValue::from_str(ADAPTER_GLOBAL))
}

fn api_function(api: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(api, &JsValue::from_str(name))?.dyn_into::<Function>()
}

fn failure_reason(error: &JsValue) -> Option<String> {
    Reflect::get(error, &JsValue::from_str("reason"))
        .ok()
        .and_then(|value| value.as_string())
}

/// Calls adapter API `name` with one argument, wraps the returned promise,
/// and awaits it.
async fn await_api_1(name: &str, arg: &JsValue) -> Result<JsValue, JsValue> {
    api_promise_1(name, arg)?.await
}

/// Calls adapter API `name` with two arguments, wraps the returned promise,
/// and awaits it.
async fn await_api_2(name: &str, first: &JsValue, second: &JsValue) -> Result<JsValue, JsValue> {
    api_promise_2(name, first, second)?.await
}

fn api_promise_1(name: &str, arg: &JsValue) -> Result<JsFuture, JsValue> {
    let api = maplibre_api()?;
    let function = api_function(&api, name)?;
    Ok(JsFuture::from(Promise::resolve(
        &function.call1(&api, arg)?,
    )))
}

fn api_promise_2(name: &str, first: &JsValue, second: &JsValue) -> Result<JsFuture, JsValue> {
    let api = maplibre_api()?;
    let function = api_function(&api, name)?;
    Ok(JsFuture::from(Promise::resolve(
        &function.call2(&api, first, second)?,
    )))
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
