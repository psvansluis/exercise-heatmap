use leptos::prelude::*;

/// MapLibre GL JS mounts into this element after Leptos adds it to the page.
#[component]
pub fn MapLibreMap() -> impl IntoView {
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
