use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use leptos::prelude::*;
use send_wrapper::SendWrapper;

use super::{
    adapter::{Map, timeout_future},
    status::{MSG_TILES, MapStatus},
};

/// The component's two pieces of mutable shared state: whether the component
/// was torn down, and the live map handed over by the renderer.
#[derive(Clone)]
pub(crate) struct MapHandle {
    cancelled: SendWrapper<Rc<Cell<bool>>>,
    map_instance: SendWrapper<Rc<RefCell<Option<Map>>>>,
}

impl MapHandle {
    pub(crate) fn new() -> Self {
        Self {
            cancelled: SendWrapper::new(Rc::new(Cell::new(false))),
            map_instance: SendWrapper::new(Rc::new(RefCell::new(None))),
        }
    }

    /// Records the live map handed over by the renderer.
    pub(crate) fn store(&self, map: Map) {
        *self.map_instance.borrow_mut() = Some(map);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }

    /// Cancels pending work and drops the stored map through the adapter.
    pub(crate) fn dispose(&self) {
        self.cancelled.set(true);
        self.remove();
    }

    /// Drops the stored map (if any) through the adapter.
    pub(crate) fn remove(&self) {
        if let Some(map) = self.map_instance.borrow_mut().take() {
            map.remove();
        }
    }
}

impl Default for MapHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolves the status to `Ready` once the renderer fires "load"; a watchdog
/// turns a hung load into an error and tears the map down.
pub(crate) fn watch_map_load(
    map: Map,
    status: RwSignal<MapStatus>,
    handle: MapHandle,
    timeout_ms: u32,
) {
    let loaded = SendWrapper::new(Rc::new(Cell::new(false)));
    let timed_out = SendWrapper::new(Rc::new(Cell::new(false)));

    leptos::task::spawn_local({
        let loaded = loaded.clone();
        let timed_out = timed_out.clone();
        let handle = handle.clone();

        async move {
            let _ = timeout_future(timeout_ms).await;
            if !loaded.get() && !handle.is_cancelled() {
                timed_out.set(true);
                status.set(MapStatus::Unavailable(MSG_TILES.to_string()));
                handle.remove();
            }
        }
    });

    leptos::task::spawn_local({
        let loaded = loaded.clone();
        let timed_out = timed_out.clone();
        let handle = handle.clone();
        let map = map;

        async move {
            match map.loaded().await {
                Ok(_) => {
                    loaded.set(true);
                    status.set(MapStatus::Ready);
                }
                Err(_) if !handle.is_cancelled() && !loaded.get() && !timed_out.get() => {
                    status.set(MapStatus::Unavailable(MSG_TILES.to_string()));
                    map.remove();
                }
                Err(_) => {}
            }
        }
    });
}
