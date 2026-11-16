use leptos::prelude::*;

/// Map container scaffold. MapLibre initialization will attach to `maplibre-map`.
#[component]
pub fn MapLibreMap() -> impl IntoView {
    view! {
        <section class="map-panel" aria-label="Activity map">
            <div class="map-toolbar">
                <span class="map-label">"ACTIVITY MAP"</span>
                <span class="map-status">
                    <span class="status-dot" aria-hidden="true"></span>
                    "MapLibre scaffold"
                </span>
            </div>
            <div class="map-stage">
                <div
                    id="maplibre-map"
                    class="maplibre-canvas"
                    role="region"
                    aria-label="Interactive activity map"
                ></div>
                <div class="map-placeholder" aria-live="polite">
                    <span class="map-placeholder-icon" aria-hidden="true">"⌖"</span>
                    <p class="map-placeholder-title">"Your map will appear here"</p>
                    <p class="map-placeholder-copy">
                        "MapLibre will render the activity map in this space."
                    </p>
                </div>
            </div>
        </section>
    }
}
