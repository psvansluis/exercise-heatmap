use std::rc::Rc;

use leptos::prelude::*;

mod adapter;
mod lifecycle;
mod options;
mod status;

use adapter::Map;
use options::MapOptions;
use status::{MapStatus, unavailable_message};

const DEFAULT_CENTER: [f64; 2] = [4.895, 52.37];
const DEFAULT_ZOOM: f64 = 10.0;
const DEFAULT_MIN_ZOOM: f64 = 2.0;
const DEFAULT_MAX_ZOOM: f64 = 19.0;
const DEFAULT_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const DEFAULT_TILE_ATTRIBUTION: &str = "© <a href=\"https://www.openstreetmap.org/copyright\" target=\"_blank\" rel=\"noopener noreferrer\">OpenStreetMap contributors</a>";
const DEFAULT_TILE_SERVICE_LABEL: &str = "OpenStreetMap";
const DEFAULT_LOAD_TIMEOUT_MS: u32 = 15_000;

// Leptos owns the map's lifetime, configuration, and all UI state; the
// modules below own the renderer bridge and the load lifecycle.
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

    let options = Rc::new(MapOptions::openstreetmap(
        center,
        zoom,
        min_zoom,
        max_zoom,
        tile_url,
        tile_attribution,
    ));

    map_container.on_load({
        let handle = handle.clone();

        move |container| {
            let handle = handle.clone();
            let options = Rc::clone(&options);

            leptos::task::spawn_local(async move {
                status.set(MapStatus::Loading);

                match Map::create(container, &options).await {
                    Ok(map) if handle.is_cancelled() => map.remove(),
                    Ok(map) => {
                        handle.store(map.clone());
                        lifecycle::watch_map_load(map, status, handle, load_timeout_ms);
                    }
                    Err(reason) => {
                        status.set(MapStatus::Unavailable(unavailable_message(reason)));
                    }
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
