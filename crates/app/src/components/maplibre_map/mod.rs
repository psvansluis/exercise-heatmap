use leptos::prelude::*;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::JsValue;

mod adapter;
mod lifecycle;
mod options;
mod status;

use adapter::{AdapterReason, await_api_2, remove_map};
pub use options::MapOptions;
use status::{MapStatus, unavailable_message};

const DEFAULT_CENTER: [f64; 2] = [4.895, 52.37];
const DEFAULT_ZOOM: f64 = 10.0;
const DEFAULT_MIN_ZOOM: f64 = 2.0;
const DEFAULT_MAX_ZOOM: f64 = 19.0;
const DEFAULT_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const DEFAULT_TILE_ATTRIBUTION: &str = "© <a href=\"https://www.openstreetmap.org/copyright\" target=\"_blank\" rel=\"noopener noreferrer\">OpenStreetMap contributors</a>";
const DEFAULT_TILE_SERVICE_LABEL: &str = "OpenStreetMap";
const DEFAULT_LOAD_TIMEOUT_MS: u32 = 15_000;

// Leptos owns the MapLibre instance lifetime, the map configuration, and all
// UI state; the JavaScript adapter provides the renderer-specific calls that
// cross the WebAssembly boundary and never touches the DOM itself.
#[component]
pub fn MapLibreMap(
    /// Center of the map as `[longitude, latitude]`.
    #[prop(default = DEFAULT_CENTER)]
    center: [f64; 2],
    #[prop(default = DEFAULT_ZOOM)] zoom: f64,
    #[prop(default = DEFAULT_MIN_ZOOM)] min_zoom: f64,
    #[prop(default = DEFAULT_MAX_ZOOM)] max_zoom: f64,
    /// Template tile URL with `{z}`, `{x}`, `{y}` placeholders.
    #[prop(default = DEFAULT_TILE_URL.to_string())]
    tile_url: String,
    /// HTML shown in the map attribution control.
    #[prop(default = DEFAULT_TILE_ATTRIBUTION.to_string())]
    tile_attribution: String,
    /// Label shown in the toolbar once the map has loaded.
    #[prop(default = DEFAULT_TILE_SERVICE_LABEL.to_string())]
    tile_service_label: String,
    /// How long to wait for tiles before declaring the map unavailable.
    #[prop(default = DEFAULT_LOAD_TIMEOUT_MS)]
    load_timeout_ms: u32,
) -> impl IntoView {
    let map_container = NodeRef::<leptos::html::Div>::new();
    let status = RwSignal::new(MapStatus::Starting);
    let handle = lifecycle::MapHandle::new();

    let options =
        MapOptions::openstreetmap(center, zoom, min_zoom, max_zoom, tile_url, tile_attribution);

    map_container.on_load({
        let handle = handle.clone();

        move |container| {
            let container: JsValue = container.into();
            let handle = handle.clone();
            let options = options.clone();

            leptos::task::spawn_local(async move {
                status.set(MapStatus::Loading);

                match create_map(&container, &options).await {
                    Ok(map) if !map.is_null() && !map.is_undefined() => {
                        if handle.is_cancelled() {
                            // The map was created after teardown; nothing was
                            // stored, so remove the fresh instance directly.
                            remove_map(&map);
                        } else {
                            handle.store(map.clone());
                            lifecycle::watch_map_load(&map, status, handle, load_timeout_ms);
                        }
                    }
                    Ok(_) => {}
                    Err(error) if !handle.is_cancelled() => {
                        let reason = AdapterReason::from_js(&error);
                        status.set(MapStatus::Unavailable(unavailable_message(reason)));
                    }
                    Err(_) => {}
                }
            });
        }
    });

    on_cleanup(move || handle.dispose());

    let ready_label = tile_service_label.clone();

    view! {
        <section class="map-panel" aria-label="Activity map">
            <div class="map-toolbar">
                <span class="map-label">"ACTIVITY MAP"</span>
                <span class="map-status" aria-live="polite">
                    <span class="status-dot" aria-hidden="true"></span>
                    <span data-map-status-text>
                        {move || match status.get() {
                            MapStatus::Starting => "Starting map".to_string(),
                            MapStatus::Loading => "Loading map".to_string(),
                            MapStatus::Ready => ready_label.clone(),
                            MapStatus::Unavailable(_) => "Map unavailable".to_string(),
                        }}
                    </span>
                </span>
            </div>
            <div class="map-stage">
                <div
                    node_ref=map_container
                    class="maplibre-canvas"
                    role="region"
                    aria-label="Interactive OpenStreetMap map"
                ></div>
                <div
                    class="map-placeholder"
                    aria-live="polite"
                    hidden=move || status.get() == MapStatus::Ready
                >
                    <span class="map-placeholder-icon" aria-hidden="true">"⌖"</span>
                    <p class="map-placeholder-title">
                        {move || match status.get() {
                            MapStatus::Starting => "Starting the map",
                            MapStatus::Loading => "Loading the map",
                            MapStatus::Ready => "",
                            MapStatus::Unavailable(_) => "The map could not load",
                        }}
                    </p>
                    <p class="map-placeholder-copy">
                        {move || match status.get() {
                            MapStatus::Starting => {
                                "The map will appear here as soon as it has loaded.".to_string()
                            }
                            MapStatus::Loading => {
                                format!("Connecting to the {tile_service_label} tile service…")
                            }
                            MapStatus::Ready => String::new(),
                            MapStatus::Unavailable(message) => message,
                        }}
                    </p>
                </div>
            </div>
        </section>
    }
}

/// Hands the container and serialized options to the JS adapter.
async fn create_map(container: &JsValue, options: &MapOptions) -> Result<JsValue, JsValue> {
    // MapLibre's style validator needs plain objects, not ES Maps.
    let serialized = options
        .serialize(&Serializer::new().serialize_maps_as_objects(true))
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    await_api_2("createMap", container, &serialized).await
}
