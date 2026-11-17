const maplibreModuleUrl =
  "https://unpkg.com/maplibre-gl@6.18.0/dist/maplibre-gl.mjs";
const loadTimeouts = new WeakMap();

function panelFor(container) {
  return container.closest(".map-panel");
}

function showLoading(panel) {
  panel.querySelector("[data-map-status-text]").textContent = "Loading map";
  const placeholder = panel.querySelector(".map-placeholder");
  placeholder.hidden = false;
  placeholder.querySelector(".map-placeholder-title").textContent =
    "Loading the map";
  placeholder.querySelector(".map-placeholder-copy").textContent =
    "Connecting to the OpenStreetMap tile service…";
}

function showUnavailable(panel, message) {
  panel.querySelector("[data-map-status-text]").textContent = "Map unavailable";
  const placeholder = panel.querySelector(".map-placeholder");
  placeholder.hidden = false;
  placeholder.querySelector(".map-placeholder-title").textContent =
    "The map could not load";
  placeholder.querySelector(".map-placeholder-copy").textContent = message;
}

async function createMap(container) {
  const panel = panelFor(container);
  if (!panel) return null;
  showLoading(panel);

  let maplibregl;
  try {
    maplibregl = await import(maplibreModuleUrl);
  } catch {
    showUnavailable(
      panel,
      "MapLibre could not be loaded. Check your connection and reload the page.",
    );
    return null;
  }
  if (!container.isConnected) return null;

  try {
    const map = new maplibregl.Map({
      container,
      center: [4.895, 52.37],
      zoom: 10,
      minZoom: 2,
      maxZoom: 19,
      style: {
        version: 8,
        sources: {
          openstreetmap: {
            type: "raster",
            tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
            tileSize: 256,
            attribution:
              '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap contributors</a>',
          },
        },
        layers: [
          {
            id: "openstreetmap-raster",
            type: "raster",
            source: "openstreetmap",
            minzoom: 0,
            maxzoom: 19,
          },
        ],
      },
      attributionControl: { compact: true },
    });

    map.addControl(
      new maplibregl.NavigationControl({ showCompass: false }),
      "top-right",
    );

    const loadTimeout = setTimeout(() => {
      if (!map.loaded()) {
        showUnavailable(
          panel,
          "Map tiles have not arrived. Check your connection and reload the page.",
        );
      }
    }, 15000);
    loadTimeouts.set(map, loadTimeout);

    map.once("load", () => {
      clearTimeout(loadTimeout);
      loadTimeouts.delete(map);
      panel.querySelector("[data-map-status-text]").textContent =
        "OpenStreetMap";
      panel.querySelector(".map-placeholder").hidden = true;
    });

    return map;
  } catch {
    showUnavailable(
      panel,
      "MapLibre could not be initialized in this browser. Check your connection and reload the page.",
    );
    return null;
  }
}

function removeMap(map) {
  const loadTimeout = loadTimeouts.get(map);
  if (loadTimeout !== undefined) clearTimeout(loadTimeout);
  loadTimeouts.delete(map);
  map.remove();
}

function showError(container, message) {
  const panel = panelFor(container);
  if (panel) showUnavailable(panel, message);
}

globalThis.exerciseMapLibre = { createMap, removeMap, showError };
