// Thin renderer adapter for MapLibre. All lifecycle, state, and messaging
// lives in Rust (crates/app/src/components/maplibre_map.rs); this file only
// knows how to construct and destroy a map instance.
const maplibreModuleUrl =
  "https://unpkg.com/maplibre-gl@6.18.0/dist/maplibre-gl.mjs";

// Map instance -> reject function of the outstanding mapLoaded() promise.
// removeMap() settles these so a Rust-side timeout can break the load wait.
const loadWaiters = new WeakMap();

function mapError(reason) {
  const error = new Error(`maplibre adapter failure: ${reason}`);
  error.reason = reason;
  return error;
}

// options is a plain object produced in Rust (center, zoom, style, ...).
async function createMap(container, options) {
  let maplibregl;
  try {
    maplibregl = await import(maplibreModuleUrl);
  } catch {
    throw mapError("map-module");
  }

  try {
    const map = new maplibregl.Map({ ...options, container });
    map.addControl(
      new maplibregl.NavigationControl({ showCompass: false }),
      "top-right",
    );
    return map;
  } catch {
    throw mapError("map-init");
  }
}

// Resolves on the map's "load" event; rejects if removeMap() is called first.
function mapLoaded(map) {
  return new Promise((resolve, reject) => {
    if (map.loaded()) {
      resolve();
      return;
    }
    loadWaiters.set(map, reject);
    map.once("load", () => {
      if (loadWaiters.get(map) === reject) loadWaiters.delete(map);
      resolve();
    });
  });
}

function removeMap(map) {
  const reject = loadWaiters.get(map);
  if (reject !== undefined) {
    loadWaiters.delete(map);
    reject(mapError("removed"));
  }
  map.remove();
}

const TRACKS_SOURCE_ID = "exercise-tracks";
const TRACKS_LAYER_ID = "exercise-tracks-line";

function setTracks(map, geojsonData) {
  const source = map.getSource(TRACKS_SOURCE_ID);
  if (source) {
    source.setData(geojsonData);
  } else {
    map.addSource(TRACKS_SOURCE_ID, {
      type: "geojson",
      data: geojsonData,
    });
    map.addLayer({
      id: TRACKS_LAYER_ID,
      type: "line",
      source: TRACKS_SOURCE_ID,
      layout: {
        "line-cap": "round",
        "line-join": "round",
      },
      paint: {
        "line-color": [
          "match",
          ["get", "activityType"],
          "Run",
          "#e0523c",
          "Ride",
          "#2564eb",
          "Walk",
          "#0d9489",
          "Hike",
          "#d97706",
          "Swim",
          "#0284c7",
          "#7c3aed",
        ],
        "line-width": 3.5,
        "line-opacity": 0.75,
      },
    });
  }
}

globalThis.exerciseMapLibre = { createMap, mapLoaded, removeMap, setTracks };
