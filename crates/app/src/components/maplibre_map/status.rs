/// Panel state, owned by the component and rendered entirely from Leptos.
#[derive(Clone, PartialEq)]
pub(crate) enum MapStatus {
    Starting,
    Loading,
    Ready,
    Unavailable(String),
}

/// Copy shown when the renderer rejects with an unknown/unclassified reason.
const MSG_MODULE: &str = "MapLibre could not be loaded. Check your connection and reload the page.";
const MSG_INIT: &str =
    "MapLibre could not be initialized in this browser. Check your connection and reload the page.";
const MSG_GENERIC: &str =
    "MapLibre could not be initialized. Check your connection and reload the page.";
pub(crate) const MSG_TILES: &str =
    "Map tiles have not arrived. Check your connection and reload the page.";

/// Maps an adapter rejection reason (public/maplibre.js) to the panel's
/// failure copy.
pub(crate) fn unavailable_message(reason: Option<&str>) -> String {
    match reason {
        Some("map-module") => MSG_MODULE.to_string(),
        Some("map-init") => MSG_INIT.to_string(),
        _ => MSG_GENERIC.to_string(),
    }
}
