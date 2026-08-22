import * as maplibregl from "maplibre-gl";
import { state } from "./stores";
import { dom } from "./ui";
import { normalizeLongitude } from "./utils";
import { preloader } from "./preloader";
import { getTileIndices } from "./utils";

export let map: maplibregl.Map;

export function initMap() {
  map = new maplibregl.Map({
    container: "map",
    style: {
      version: 8,
      sources: {
        "carto-dark": {
          type: "raster",
          tiles: ["https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png"],
          tileSize: 256,
          attribution:
            '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
        },
      },
      layers: [{ id: "carto-dark-layer", type: "raster", source: "carto-dark" }],
    },
    center: [0, 40],
    zoom: 1.5,
    maxBounds: [
      [-Infinity, -75.0],
      [Infinity, 85.0],
    ],
  });

  map.addControl(
    new maplibregl.ScaleControl({ maxWidth: 100, unit: "metric" }),
    "bottom-right",
  );
  map.on("click", (e) => {
    console.log("Map clicked!");
    if (!state.simulationHistory.length) {
      state.rawLon = parseFloat(e.lngLat.lng.toFixed(2));
      state.rawLat = parseFloat(e.lngLat.lat.toFixed(2));
      updateFields();
      updateMarker();
    }
  });

  map.on("mousemove", (e) => {
    document.getElementById("coordinate-display").textContent =
      `${e.lngLat.lat.toFixed(2)}°, ${e.lngLat.lng.toFixed(2)}°`;
  });
}

export function updateMarker() {
  if (state.simulationHistory.length === 0 && state.currentMarker) {
    state.currentMarker.remove();
  }
  if (state.simulationHistory.length === 0) {
    state.currentMarker = new maplibregl.Marker({
      color: "#244886",
      scale: 0.9,
    })
      .setLngLat([state.rawLon, state.rawLat])
      .addTo(map);
  }
}



export function updatePositionFromFields() {
  if (!isNaN(parseFloat(dom.lonField.value)) && !isNaN(parseFloat(dom.latField.value))) {
    state.rawLon = parseFloat(dom.lonField.value);
    state.rawLat = parseFloat(dom.latField.value);
  }
  updateMarker();
}
export function zoom() {
  if (!dom.autoZoom.checked) return;
  if (map.getZoom() < 6 - state.totalDays / 100) {
    map.flyTo({
      center: [state.rawLon, state.rawLat],
      zoom: 6 - state.totalDays / 100,
      duration: 2000,
    });
  } else {
    map.flyTo({
      center: [state.rawLon, state.rawLat],
      zoom: map.getZoom(),
      duration: 2000,
    });
  }
}

export async function updateFields() {
  dom.lonField.value = normalizeLongitude(state.rawLon).toFixed(2);
  dom.latField.value = state.rawLat.toFixed(2);

  const currentDate = parseInt(
    `${state.startYear}${String(state.startMonth).padStart(2, "0")}${String(state.startDay).padStart(2, "0")}`,
  );
  const oceanTile = getTileIndices([normalizeLongitude(state.rawLon), state.rawLat], -80);
  preloader.preloadTiles(currentDate, oceanTile);
  state.landmaskPromise = state.proteus.init_landmask(normalizeLongitude(state.rawLon), state.rawLat);
  await state.landmaskPromise;
  state.landmaskPromise = null;
  updateMarker();
}

const coordDisplay = document.createElement("div");
coordDisplay.id = "coordinate-display";
coordDisplay.style.cssText =
  "position: absolute; bottom: 18px; right: 45px; color: rgba(255,255,255,0.7); font-family: monospace; font-size: 12px; z-index: 10; pointer-events: none;";
document.body.appendChild(coordDisplay);