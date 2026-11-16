const mountedMaps = new Map();
const maplibreModuleUrl = "https://unpkg.com/maplibre-gl@6.18.0/dist/maplibre-gl.mjs";

function showUnavailable(status, placeholder, message) {
  status.textContent = "Map unavailable";
  placeholder.hidden = false;
  placeholder.querySelector(".map-placeholder-title").textContent = "The map could not load";
  placeholder.querySelector(".map-placeholder-copy").textContent = message;
}

async function mountMap(container) {
  if (mountedMaps.has(container)) return;
  mountedMaps.set(container, null);

  const panel = container.closest(".map-panel");
  const status = panel?.querySelector("[data-map-status-text]");
  const placeholder = panel?.querySelector(".map-placeholder");
  if (!panel || !status || !placeholder) {
    mountedMaps.delete(container);
    return;
  }

  status.textContent = "Loading map";
  placeholder.hidden = false;
  placeholder.querySelector(".map-placeholder-title").textContent = "Loading the map";
  placeholder.querySelector(".map-placeholder-copy").textContent = "Connecting to the OpenStreetMap tile service…";

  let maplibregl;
  try {
    maplibregl = await import(maplibreModuleUrl);
  } catch {
    if (container.isConnected) {
      showUnavailable(status, placeholder, "MapLibre could not be loaded. Check your connection and reload the page.");
    }
    mountedMaps.delete(container);
    return;
  }
  if (!container.isConnected) {
    mountedMaps.delete(container);
    return;
  }

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
          attribution: '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap contributors</a>',
        },
      },
      layers: [{
        id: "openstreetmap-raster",
        type: "raster",
        source: "openstreetmap",
        minzoom: 0,
        maxzoom: 19,
      }],
    },
    attributionControl: { compact: true },
  });
  mountedMaps.set(container, map);

  map.addControl(new maplibregl.NavigationControl({ showCompass: false }), "top-right");
  const loadTimeout = setTimeout(() => {
    if (!map.loaded()) {
      showUnavailable(status, placeholder, "Map tiles have not arrived. Check your connection and reload the page.");
    }
  }, 15000);
  map.once("load", () => {
    clearTimeout(loadTimeout);
    status.textContent = "OpenStreetMap";
    placeholder.hidden = true;
  });
}

function unmountMap(container) {
  if (!mountedMaps.has(container)) return;
  mountedMaps.get(container)?.remove();
  mountedMaps.delete(container);
}

function visitAdded(node) {
  if (!(node instanceof Element)) return;
  if (node.matches(".maplibre-canvas")) mountMap(node);
  node.querySelectorAll(".maplibre-canvas").forEach(mountMap);
}

function visitRemoved(node) {
  if (!(node instanceof Element)) return;
  if (node.matches(".maplibre-canvas")) unmountMap(node);
  node.querySelectorAll(".maplibre-canvas").forEach(unmountMap);
}

const observer = new MutationObserver((records) => {
  for (const record of records) {
    record.addedNodes.forEach(visitAdded);
    record.removedNodes.forEach(visitRemoved);
  }
});

observer.observe(document.documentElement, { childList: true, subtree: true });
document.querySelectorAll(".maplibre-canvas").forEach(mountMap);
