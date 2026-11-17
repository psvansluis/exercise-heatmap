use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::JsValue;

use super::{
    adapter::{await_api_1, remove_map, timeout_future},
    status::{MSG_TILES, MapStatus},
};

/// Resolves the status to `Ready` once the renderer fires "load"; a watchdog
/// turns a hung load into an error and tears the map down.
pub(crate) fn watch_map_load(
    map: &JsValue,
    status: RwSignal<MapStatus>,
    cancelled: SendWrapper<Rc<Cell<bool>>>,
    map_instance: SendWrapper<Rc<RefCell<Option<JsValue>>>>,
    timeout_ms: u32,
) {
    let loaded = SendWrapper::new(Rc::new(Cell::new(false)));
    let timed_out = SendWrapper::new(Rc::new(Cell::new(false)));

    leptos::task::spawn_local({
        let loaded = loaded.clone();
        let timed_out = timed_out.clone();
        let cancelled = cancelled.clone();
        let map_instance = map_instance.clone();
        let map = map.clone();

        async move {
            let _ = timeout_future(timeout_ms).await;
            if !loaded.get() && !cancelled.get() {
                timed_out.set(true);
                status.set(MapStatus::Unavailable(MSG_TILES.to_string()));
                *map_instance.borrow_mut() = None;
                remove_map(&map);
            }
        }
    });

    leptos::task::spawn_local({
        let loaded = loaded.clone();
        let timed_out = timed_out.clone();
        let cancelled = cancelled.clone();
        let map = map.clone();

        async move {
            match await_api_1("mapLoaded", &map).await {
                Ok(_) => {
                    loaded.set(true);
                    status.set(MapStatus::Ready);
                }
                Err(_) if !cancelled.get() && !loaded.get() && !timed_out.get() => {
                    status.set(MapStatus::Unavailable(MSG_TILES.to_string()));
                    remove_map(&map);
                }
                Err(_) => {}
            }
        }
    });
}
