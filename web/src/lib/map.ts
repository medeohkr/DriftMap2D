// @ts-expect-error
import * as maplibregl from 'https://unpkg.com/maplibre-gl@^6.6.0/dist/maplibre-gl.mjs';
import { simulation, config, visualization } from "./stores.svelte";
import { preloader } from "./preloader";

export let map: any;

export function initMap() {
    map = new maplibregl.Map({
        container: "map",
        style: {
            version: 8,
            sources: {
                "carto-dark": {
                    type: "raster",
                    tiles: [
                        "https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png?key=cb1_2i6g_1_acb8c049139549a1b56e3610",
                    ],
                    tileSize: 256,
                    attribution:
                        '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
                },
            },
            layers: [
                {
                    id: "carto-dark-layer",
                    type: "raster",
                    source: "carto-dark",
                },
            ],
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

    map.on("click", (e: any) => {
        if (!simulation.simulationActive) {
            config.lon = parseFloat(e.lngLat.lng.toFixed(2));
            config.lat = parseFloat(e.lngLat.lat.toFixed(2));
            updateMarker();
        }
    });

    map.on("mousemove", (e: any) => {
        visualization.mapLon = normalizeLongitude(e.lngLat.lng).toFixed(2);
        visualization.mapLat = e.lngLat.lat.toFixed(2);
    });
}

export async function updateMarker() {
    if (!simulation.simulationActive && visualization.currentMarker) {
        visualization.currentMarker.remove();
    }
    if (!simulation.simulationActive) {
        visualization.currentMarker = new maplibregl.Marker({
            color: "#244886",
            scale: 0.9,
        })
            .setLngLat([config.lon, config.lat])
            .addTo(map);
    }
    const currentDate = parseInt(config.startDate.replace(/-/g, ""));
    const positions = new Float32Array([
        normalizeLongitude(config.lon),
        config.lat,
    ]);
    const oceanTile = preloader.getTileIndicesForOcean(positions);
    preloader.preloadTiles(currentDate, oceanTile);
    simulation.landmaskPromise =
        simulation.proteus?.init_landmask(
            normalizeLongitude(config.lon),
            config.lat,
        ) ?? null;
    await simulation.landmaskPromise;
    simulation.landmaskPromise = null;
}

export function zoom() {
    if (!config.autoZoom) return;
    if (map.getZoom() < 6 - config.totalDays / 100) {
        map.flyTo({
            center: [config.lon, config.lat],
            zoom: 6 - config.totalDays / 100,
            duration: 2000,
        });
    } else {
        map.flyTo({
            center: [config.lon, config.lat],
            zoom: map.getZoom(),
            duration: 2000,
        });
    }
}

export function normalizeLongitude(lon: number) {
    return ((((lon + 180) % 360) + 360) % 360) - 180;
}
