use js_sys::{Function, Promise, Reflect};
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

use super::colors::ActivityColors;
use super::geojson::GeoJsonFeatureCollection;
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
        let Ok(reason) = Reflect::get(error, &JsValue::from_str("reason")) else {
            return Self::Other;
        };

        match reason {
            reason if reason == JsValue::from_str("map-module") => Self::Module,
            reason if reason == JsValue::from_str("map-init") => Self::Init,
            reason if reason == JsValue::from_str("removed") => Self::Removed,
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

    /// Sends a GeoJSON `FeatureCollection` of tracks to the JS adapter,
    /// which creates or updates the MapLibre source and layer.
    pub(crate) fn set_tracks(&self, geojson: &GeoJsonFeatureCollection, colors: &ActivityColors) {
        if let Ok(api) = maplibre_api()
            && let Ok(set_tracks) = api_function(&api, "setTracks")
            && let Ok(js_data) = serde_wasm_bindgen::to_value(geojson)
            // MapLibre needs plain objects; a map would serialize as a JS Map.
            && let Ok(js_colors) = colors
                .serialize(&Serializer::new().serialize_maps_as_objects(true))
        {
            let _ = set_tracks.call3(&api, &self.0, &js_data, &js_colors);
        }
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

#[cfg(test)]
mod tests {
    use super::AdapterReason;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn adapter_reasons_map_from_js_values() {
        let with_reason = |reason: &str| {
            let error = js_sys::Error::new(reason);
            js_sys::Reflect::set(&error, &"reason".into(), &reason.into()).unwrap();
            error.into()
        };
        assert_eq!(
            AdapterReason::from_js(&with_reason("map-module")),
            AdapterReason::Module
        );
        assert_eq!(
            AdapterReason::from_js(&with_reason("map-init")),
            AdapterReason::Init
        );
        assert_eq!(
            AdapterReason::from_js(&with_reason("removed")),
            AdapterReason::Removed
        );
        assert_eq!(
            AdapterReason::from_js(&with_reason("who-knows")),
            AdapterReason::Other
        );
        let no_reason: JsValue = js_sys::Error::new("no reason").into();
        assert_eq!(AdapterReason::from_js(&no_reason), AdapterReason::Other);
    }
}
