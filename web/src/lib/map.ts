// @ts-expect-error
import * as maplibregl from 'https://unpkg.com/maplibre-gl@^6.6.0/dist/maplibre-gl.mjs';
import { GeoGrid } from "geogrid-maplibre-gl";
import "geogrid-maplibre-gl/dist/geogrid.css";
import { mount, unmount } from "svelte";
import MapDisplay from "../components/MapDisplay.svelte";
import { simulation, config, visualization, releaseConfig } from "./stores/index.svelte";
import { preloader } from "./preloader";
import { getReleasePositions, normalizeLongitude, getReleaseBounds, getReleaseColor, renderForcingCoverage } from "./utils";



export let map: any;

export function initMap() {
    map = new maplibregl.Map({
        container: "map",
        style: "https://api.maptiler.com/maps/019da3a2-47a0-7f1b-b591-38d451a7c7c5/style.json?key=gkGR2Cprig9FAk9AfV3C",
        center: [-85, 40],
        zoom: 1,
        attributionControl: false,
        maxBounds: [
            [-Infinity, -75.0],
            [Infinity, 85.0],
        ],
    });

    const attribution = new maplibregl.AttributionControl();

    let graticule: GeoGrid | undefined;
    let showGraticule = false;
    const toggleGraticule = (visible: boolean) => {
        if (visible === showGraticule) return;
        showGraticule = visible;
        if (graticule) {
            if (visible) graticule.add();
            else graticule.remove();
        }
    };
    map.once("load", () => {
        const beforeLayerId = map.getStyle().layers.find((layer: { type: string }) => layer.type === "symbol")?.id;
        graticule = new GeoGrid({
            map,
            beforeLayerId,
            gridStyle: { color: "rgba(220, 220, 220, 0.1)", width: 1 },
            labelStyle: {
                color: "rgb(120, 120, 120)",
                fontSize: "11px",
                fontFamily: "Montserrat, sans-serif",
                textShadow: "0 1px 3px black",
            },
        });
        // the style is already loaded so add the grid without waiting for another load event
        map.off("load", graticule.add);
        if (showGraticule) graticule.add();
    });

    const scale = new maplibregl.ScaleControl({ maxWidth: 100, unit: "metric" });
    let display: ReturnType<typeof mount>;
    let displayContainer: HTMLDivElement;
    map.addControl({
        onAdd(controlMap: any) {
            displayContainer = document.createElement("div");
            displayContainer.className = "maplibregl-ctrl";
            display = mount(MapDisplay, {
                target: displayContainer,
                props: {
                    scaleElement: scale.onAdd(controlMap),
                    attributionElement: attribution.onAdd(controlMap),
                    toggleGraticule,
                },
            });
            return displayContainer;
        },
        onRemove() {
            scale.onRemove();
            attribution.onRemove();
            void unmount(display);
            displayContainer.remove();
        },
    }, "bottom-right");
    map.getCanvas().style.cursor = simulation.simulationActive ? "" : "crosshair";
    map.on("load", updateForcingCoverage);
    map.on("dragstart", () => {
        map.getCanvas().style.cursor = "move";
    });
    map.on("dragend", () => {
        map.getCanvas().style.cursor = simulation.simulationActive ? "" : "crosshair";
    });

    map.on("click", (e: any) => {
        if (e.originalEvent.target.closest?.(".maplibregl-marker")) return;
        if (!simulation.simulationActive) {
            releaseConfig.activeRelease.lon = parseFloat(e.lngLat.lng.toFixed(3));
            releaseConfig.activeRelease.lat = parseFloat(e.lngLat.lat.toFixed(3));
        }
    });

}

export function updateForcingCoverage() {
    return renderForcingCoverage(map, config.forcingModel === "norkyst" && !simulation.simulationActive);
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
            draggable: !simulation.simulationActive,
        }).setLngLat([release.lon, release.lat]);
        const element = marker.getElement();
        element.style.display = simulation.simulationActive ? "none" : "";
        element.style.cursor = "grab";
        element.setAttribute("role", "button");
        element.setAttribute("aria-label", `Edit release ${index + 1}`);
        element.setAttribute("aria-pressed", String(active));
        element.tabIndex = 0;
        const select = () => {
            if (!simulation.simulationActive) {
                releaseConfig.activeReleaseIndex = index;
                updateMarkerSelection();
            }
        };
        marker.on("dragstart", () => {
            select();
            element.style.cursor = "grabbing";
        });
        marker.on("dragend", () => {
            element.style.cursor = "grab";
            if (simulation.simulationActive) return;
            const position = marker.getLngLat();
            release.lon = parseFloat(position.lng.toFixed(3));
            release.lat = parseFloat(position.lat.toFixed(3));
        });
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
    updateMarkerSelection();

    const positions = new Float32Array(getReleasePositions());
    const currentDate = parseInt(config.startDate.replace(/-/g, ""));
    void preloader.preloadOcean(currentDate, positions, config.forcingModel);

    pendingLandmaskPreload = positions;
    if (!landmaskPreloadInFlight) {
        runPendingLandmaskPreload();
    }
}

export function updateMarkerSelection() {
    releaseMarkers.forEach((marker, index) => {
        const active = index === releaseConfig.activeReleaseIndex;
        marker.setOpacity(active ? 1 : 0.5);
        const element = marker.getElement();
        element.setAttribute("aria-pressed", String(active));
    });
    visualization.currentMarker = releaseMarkers[releaseConfig.activeReleaseIndex] ?? null;
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
    const activeLongitude = releaseConfig.activeRelease?.lon;
    const referenceLongitude = Number.isFinite(activeLongitude)
        ? activeLongitude
        : map.getCenter().lng;
    const bounds = getReleaseBounds(releaseConfig.releases, referenceLongitude);
    if (!bounds) return;
    const { width, height } = map.getContainer().getBoundingClientRect();
    if (width <= 0 || height <= 0) return;
    const padding = Math.min(80, width * 0.15, height * 0.15);
    map.fitBounds(bounds, {
        padding: {
            top: height * 0.1 + padding,
            bottom: padding,
            left: width * 0.15 + padding,
            right: padding,
        },
        maxZoom: Math.max(6, map.getZoom()),
        duration: 1500,
    });
}

export function setMarkerVisible(visible: boolean) {
    if (map) map.getCanvas().style.cursor = simulation.simulationActive ? "" : "crosshair";
    releaseMarkers.forEach(marker => {
        const draggable = visible && !simulation.simulationActive;
        if (marker.isDraggable() !== draggable) marker.setDraggable(draggable);
        marker.getElement().style.display = visible ? "" : "none";
    });
}
