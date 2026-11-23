use crate::components::{
    activity_table::ActivityTable,
    import_panel::{ActivityUpdate, ImportPanel},
    maplibre_map::MapLibreMap,
};
use exercise_domain::{ActivityEntry, ActivityId};
use indexmap::IndexMap;
use leptos::prelude::*;

#[component]
pub fn Home() -> impl IntoView {
    let activities = RwSignal::new(IndexMap::<ActivityId, ActivityEntry>::new());
    let on_activity_update = Callback::new(move |update| match update {
        ActivityUpdate::Clear => activities.set(IndexMap::new()),
        ActivityUpdate::Append(entry) => {
            activities.update(|items| {
                items.insert(entry.activity.id.clone(), entry);
            });
        }
    });

    // Entries whose tracks should be drawn on the map.
    let active_entries = Signal::derive(move || {
        activities
            .get()
            .values()
            .filter(|e| e.include_on_map && e.track.is_some())
            .cloned()
            .collect::<Vec<_>>()
    });

    // Center on the first valid GPS coordinate across all uploaded activities
    // (regardless of `include_on_map`). Returns None when no activity has a
    // track with at least one valid point.
    let first_valid_track_center = move || {
        activities.get().values().find_map(|entry| {
            entry.track.as_ref().and_then(|track| {
                track
                    .points
                    .iter()
                    .find(|p| p.has_valid_coordinates())
                    .map(|p| [p.longitude, p.latitude])
            })
        })
    };

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

            <Show when=move || first_valid_track_center().is_some()>
                {move || {
                    let center = first_valid_track_center().unwrap_or([4.895, 52.37]);
                    view! { <MapLibreMap center=center active_entries=active_entries /> }
                }}
            </Show>

            <footer class="page-footer">
                "A personal project for making exercise history easier to explore."
            </footer>
        </main>
    }
}
