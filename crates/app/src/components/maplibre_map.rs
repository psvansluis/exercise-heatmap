use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use js_sys::{Function, Promise, Reflect};
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

fn maplibre_api() -> Result<JsValue, JsValue> {
    Reflect::get(&js_sys::global(), &JsValue::from_str("exerciseMapLibre"))
}

fn api_function(api: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(api, &JsValue::from_str(name))?.dyn_into::<Function>()
}

fn remove_map(map: &JsValue) {
    if let Ok(api) = maplibre_api() {
        if let Ok(remove) = api_function(&api, "removeMap") {
            let _ = remove.call1(&api, map);
        }
    }
}

fn report_map_error(container: &JsValue) {
    if let Ok(api) = maplibre_api() {
        if let Ok(show_error) = api_function(&api, "showError") {
            let _ = show_error.call2(
                &api,
                container,
                &JsValue::from_str(
                    "MapLibre could not be initialized. Check your connection and reload the page.",
                ),
            );
        }
    }
}

/// Leptos owns the MapLibre instance lifetime; the JavaScript adapter provides
/// the renderer-specific calls that cross the WebAssembly boundary.
#[component]
pub fn MapLibreMap() -> impl IntoView {
    let map_container = NodeRef::<leptos::html::Div>::new();
    let cancelled = SendWrapper::new(Rc::new(Cell::new(false)));
    let map_instance = SendWrapper::new(Rc::new(RefCell::new(None::<JsValue>)));

    map_container.on_load({
        let cancelled = cancelled.clone();
        let map_instance = map_instance.clone();

        move |container| {
            let container: JsValue = container.into();
            let cancelled = cancelled.clone();
            let map_instance = map_instance.clone();

            leptos::task::spawn_local(async move {
                let result = async {
                    let api = maplibre_api()?;
                    let create_map = api_function(&api, "createMap")?;
                    let promise = create_map.call1(&api, &container)?;
                    JsFuture::from(Promise::resolve(&promise)).await
                }
                .await;

                match result {
                    Ok(map) if !map.is_null() && !map.is_undefined() => {
                        if cancelled.get() {
                            remove_map(&map);
                        } else {
                            *map_instance.borrow_mut() = Some(map);
                        }
                    }
                    Ok(_) => {}
                    Err(_) if !cancelled.get() => report_map_error(&container),
                    Err(_) => {}
                }
            });
        }
    });

    on_cleanup({
        let cancelled = cancelled.clone();
        let map_instance = map_instance.clone();

        move || {
            cancelled.set(true);
            if let Some(map) = map_instance.borrow_mut().take() {
                remove_map(&map);
            }
        }
    });

    view! {
        <section class="map-panel" aria-label="Activity map">
            <div class="map-toolbar">
                <span class="map-label">"ACTIVITY MAP"</span>
                <span class="map-status" aria-live="polite">
                    <span class="status-dot" aria-hidden="true"></span>
                    <span data-map-status-text>"Starting map"</span>
                </span>
            </div>
            <div class="map-stage">
                <div
                    node_ref=map_container
                    class="maplibre-canvas"
                    role="region"
                    aria-label="Interactive OpenStreetMap map"
                ></div>
                <div class="map-placeholder" aria-live="polite">
                    <span class="map-placeholder-icon" aria-hidden="true">"⌖"</span>
                    <p class="map-placeholder-title">"Starting the map"</p>
                    <p class="map-placeholder-copy">
                        "The map will appear here as soon as it has loaded."
                    </p>
                </div>
            </div>
        </section>
    }
}
