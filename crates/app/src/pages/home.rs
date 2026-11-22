use crate::components::{
    activity_table::ActivityTable,
    import_panel::{ActivityUpdate, ImportPanel},
    maplibre_map::MapLibreMap,
};
use exercise_domain::Activity;
use leptos::prelude::*;

#[component]
pub fn Home() -> impl IntoView {
    let activities = RwSignal::new(Vec::<Activity>::new());
    let on_activity_update = Callback::new(move |update| match update {
        ActivityUpdate::Clear => activities.set(Vec::new()),
        ActivityUpdate::Append(activity) => activities.update(|items| items.push(activity)),
    });

    view! {
        <main class="page-shell">
            <header class="hero">
                <p class="eyebrow">"PERSONAL ACTIVITY EXPLORER"</p>
                <h1>"Exercise heatmap"</h1>
                <p class="hero-copy">
                    "Explore your walks and runs on a map, see the routes you have taken, and discover the streets still waiting to be explored."
                </p>
            </header>

            <ImportPanel on_activity_update=on_activity_update />

            <ActivityTable activities=activities />

            <MapLibreMap />

            <footer class="page-footer">
                "A personal project for making exercise history easier to explore."
            </footer>
        </main>
    }
}
