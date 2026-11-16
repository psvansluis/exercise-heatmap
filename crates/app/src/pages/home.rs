use crate::components::maplibre_map::MapLibreMap;
use leptos::prelude::*;

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <main class="page-shell">
            <header class="hero">
                <p class="eyebrow">"PERSONAL ACTIVITY EXPLORER"</p>
                <h1>"Exercise heatmap"</h1>
                <p class="hero-copy">
                    "Explore your walks and runs on a map, see the routes you have taken, and discover the streets still waiting to be explored."
                </p>
            </header>

            <MapLibreMap />

            <footer class="page-footer">
                "A personal project for making exercise history easier to explore."
            </footer>
        </main>
    }
}
