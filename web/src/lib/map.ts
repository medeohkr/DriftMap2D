// @ts-expect-error
import * as maplibregl from 'https://unpkg.com/maplibre-gl@^6.6.0/dist/maplibre-gl.mjs';
import { simulation, config, visualization, releaseConfig } from "./stores/index.svelte";
import { preloader } from "./preloader";
import { getReleasePositions, normalizeLongitude, getReleaseBounds, getReleaseColor } from "./utils";



export let map: any;

export function initMap() {
    map = new maplibregl.Map({
        container: "map",
        style: "https://api.maptiler.com/maps/019da3a2-47a0-7f1b-b591-38d451a7c7c5/style.json?key=gkGR2Cprig9FAk9AfV3C",
        center: [-85, 40],
        zoom: 1,
        maxBounds: [
            [-Infinity, -75.0],
            [Infinity, 85.0],
        ],
    });

    map.addControl(
        new maplibregl.ScaleControl({ maxWidth: 100, unit: "metric" }),
        "bottom-right",
    );

    map.on("click", (e: any) => {
        if (!simulation.simulationActive) {
            releaseConfig.activeRelease.lon = parseFloat(e.lngLat.lng.toFixed(3));
            releaseConfig.activeRelease.lat = parseFloat(e.lngLat.lat.toFixed(3));
        }
    });

    map.on("mousemove", (e: any) => {
        visualization.mapLon = normalizeLongitude(e.lngLat.lng).toFixed(2);
        visualization.mapLat = e.lngLat.lat.toFixed(2);
    });
}

let landmaskPreloadInFlight = false;
let pendingLandmaskPreload: Float32Array | null = null;
let releaseMarkers: any[] = [];

export async function updateMarker() {
    // A reused marker can retain its previous world-copy position through
    // MapLibre's smart wrapping. Start fresh at the unwrapped release longitude.
    releaseMarkers.forEach(marker => marker.remove());
    releaseMarkers = releaseConfig.releases.map((release, index) => {
        const active = index === releaseConfig.activeReleaseIndex;
        const marker = new maplibregl.Marker({
            color: getReleaseColor(release),
            scale: 0.9,
            opacity: active ? 1 : 0.5,
        }).setLngLat([release.lon, release.lat]);
        const element = marker.getElement();
        element.style.display = simulation.simulationActive ? "none" : "";
        element.style.cursor = "pointer";
        element.setAttribute("role", "button");
        element.setAttribute("aria-label", `Edit release ${index + 1}`);
        element.setAttribute("aria-pressed", String(active));
        element.tabIndex = 0;
        const select = () => {
            if (!simulation.simulationActive) releaseConfig.activeReleaseIndex = index;
        };
        element.addEventListener("click", (event: MouseEvent) => {
            event.stopPropagation();
            select();
        });
        element.addEventListener("keydown", (event: KeyboardEvent) => {
            if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                event.stopPropagation();
                select();
            }
        });
        return marker.addTo(map);
    });
    visualization.currentMarker = releaseMarkers[releaseConfig.activeReleaseIndex] ?? null;

    const positions = new Float32Array(getReleasePositions());
    const currentDate = parseInt(config.startDate.replace(/-/g, ""));
    const oceanTiles = preloader.getTileIndicesForOcean(positions);
    preloader.preloadTiles(currentDate, oceanTiles);

    pendingLandmaskPreload = positions;
    if (!landmaskPreloadInFlight) {
        runPendingLandmaskPreload();
    }
}

async function runPendingLandmaskPreload() {
    landmaskPreloadInFlight = true;
    try {
        while (pendingLandmaskPreload) {
            const positions = pendingLandmaskPreload;
            pendingLandmaskPreload = null;

            try {
                await simulation.proteus?.load_landmask_tiles(positions);
            } catch (e) {
                console.error("Landmask preload failed:", e);
            }
        }
    } finally {
        landmaskPreloadInFlight = false;
    }
}

export function awaitLandmaskPreload(): Promise<void> {
    return new Promise(resolve => {
        const check = () => {
            if (!landmaskPreloadInFlight) {
                resolve();
            } else {
                setTimeout(check, 10);
            }
        };
        check();
    });
}

export function zoom() {
    if (!visualization.autoZoom || !map) return;
    const bounds = getReleaseBounds(releaseConfig.releases, map.getCenter().lng);
    if (!bounds) return;
    const { width, height } = map.getContainer().getBoundingClientRect();
    if (width <= 0 || height <= 0) return;
    const padding = Math.min(80, width * 0.15, height * 0.15);
    map.fitBounds(bounds, {
        padding: {
            top: padding,
            bottom: padding,
            left: width * 0.15 + padding,
            right: padding,
        },
        maxZoom: Math.max(6, map.getZoom()),
        duration: 1500,
    });
}

export function setMarkerVisible(visible: boolean) {
    releaseMarkers.forEach(marker => {
        marker.getElement().style.display = visible ? "" : "none";
    });
}
