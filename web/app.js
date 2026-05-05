import init, { Proteus, setup_panic_hook, HeatmapGenerator} from './pkg/proteus.js';
import { preloader } from './preloader.js';

let map = new maplibregl.Map({
    container: 'map',
    style: {
        version: 8,
        sources: {
            'carto-dark': {
                type: 'raster',
                tiles: [
                    'https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png',
                    'https://b.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png',
                    'https://c.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png',
                    'https://d.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png'
                ],
                tileSize: 256,
                attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> &copy; <a href="https://carto.com/attributions">CARTO</a>'
            }
        },
        layers: [{
            id: 'carto-dark-layer',
            type: 'raster',
            source: 'carto-dark',
            minzoom: 0,
            maxzoom: 22
        }]
    },
    center: [0, 0],
    zoom: 0,
});


const latField = document.querySelector('.lat-field');
const lonField = document.querySelector('.lon-field');
const startBtn = document.getElementById('start-simulation');
const dayDisplay = document.getElementById('current-day');
const stepBtn = document.getElementById('step-simulation');
const stopBtn = document.getElementById('stop-simulation');
const resumeBtn = document.getElementById('resume-simulation');
const resetBtn = document.getElementById('reset-simulation');
const oilMenu = document.getElementById('oil-menu');
const startDate = document.getElementById('start-day-selector');
const totalDaysField = document.getElementById('total-day-field');
const heatmapToggle = document.getElementById('heatmap-toggle');
const particleToggle = document.getElementById('particle-toggle');

startBtn.addEventListener('click', startSimulation);
stopBtn.addEventListener('click', stopSimulation);
resumeBtn.addEventListener('click', resumeSimulation);
resetBtn.addEventListener('click', resetSimulation);
latField.addEventListener('input', updatePositionFromFields);
lonField.addEventListener('input', updatePositionFromFields);
latField.addEventListener('blur', updateFields)
lonField.addEventListener('blur', updateFields);
startDate.addEventListener('input', updateSimulationDate);
totalDaysField.addEventListener('input', updateTotalDays);
heatmapToggle.addEventListener('click', toggleHeatmapMode);
particleToggle.addEventListener('click', toggleParticleMode)

let proteus = null;
let simulationRunning = false;
let stepInProgress = false;
let animationId = null;
let simulationVersion = 0;

let heatmap = null;
let concentrationGrid = null;
let lastGridUpdate = 0;
let visualizationMode = 'particles'; 
let rawLon = 56.54;
let rawLat = 26.74;
let kValue = 20;
let particleCount = 10000;
let spreadKm = 0.1;
let oilType = oilMenu ? oilMenu.value : 'arabian_light';
let startYear = 2025;
let startMonth = 5;
let startDay = 1;
let stepSize = 1/96;
let totalDays = 10;
let isError = false;
let stepCount = 0;
let bounding_box = [];

const GRID_UPDATE_INTERVAL = 200; 
const GRID_SIZE = 0.01;
const CONCENTRATIONS = [
    particleCount/40000,
    particleCount/20000,
    particleCount/10000,
    particleCount/5000,
    particleCount/2500,
    particleCount/1250,
    particleCount/625,
    particleCount/313,
    particleCount/156,
    particleCount/78,
]
// Normalize longitude
function normalizeLongitude(lon) {
    lon = parseFloat(lon);
    lon = ((lon + 180) % 360 + 360) % 360 - 180;
    return lon;
}
function getTileIndices() {
    const positions = proteus.get_positions();
    const tiles = new Set();
    
    for (let i = 0; i < positions.length; i += 2) {
        const lon = positions[i];
        const lat = positions[i + 1];
        
        // Calculate tile index (same as Rust: (lon - min_lon) / tile_size).floor()
        const minLon = -180;
        const minLat = -80;
        
        const lonIdx = Math.floor((lon - minLon) / 10);
        const latIdx = Math.floor((lat - minLat) / 10);
        
        // Clamp to valid range
        if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 34) {
            tiles.add({ lonIdx, latIdx });
        }
    }

    return Array.from(tiles);
}

// Get next date (YYYYMMDD)
function addDays(dateInt, days) {
    const year = Math.floor(dateInt / 10000);
    const month = Math.floor((dateInt % 10000) / 100);
    const day = dateInt % 100;
    
    const date = new Date(year, month - 1, day);
    date.setDate(date.getDate() + days);
    
    return date.getFullYear() * 10000 + (date.getMonth() + 1) * 100 + date.getDate();
}

function updateBoundingBox() {
    bounding_box = proteus.get_particle_bounding_box();
}
// Initialize WASM and UI
async function initialize() {
    await init();
    setup_panic_hook();
    
    let lon = normalizeLongitude(rawLon);
    let lat = rawLat
    
    proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
    heatmap = new HeatmapGenerator(-180, 180, -80, 90, 0.2);

    initGridLayer();
    updateMarker();
    updateSimulationDate();
    updateTotalDays();
}

function initGridLayer() {
    map.on('load', () => {
        map.addSource('concentration', {
            type: 'geojson',
            data: { type: 'FeatureCollection', features: [] },
            tolerance: 0,  // Disable simplification
            buffer: 0,     // No buffer needed
            maxzoom: 24    // Keep detailed geometry at all zooms
        });
        map.addLayer({
            id: 'concentration-fill',
            type: 'fill',
            source: 'concentration',
            paint: {
                // 'fill-color': [
                //     'interpolate', ['linear'], ['get', 'concentration'],
                //     0, 'rgb(231, 236, 251)',
                //     1, 'rgb(195, 209, 247)',
                //     2, 'rgb(162, 186, 244)',
                //     4, 'rgb(120, 153, 227)',
                //     8, 'rgb(68, 115, 227)',
                //     16, 'rgb(141, 142, 213)',
                //     32, 'rgb(252, 184, 197)',
                //     64, 'rgb(255, 115, 107)',
                //     128, 'rgb(254, 79, 44)',
                //     256, 'rgb(255, 106, 0)',
                //     512, 'rgb(255, 154, 0)',
                //     1024, 'rgb(255, 216, 1)',
                // ],
                'fill-color': [
                    'interpolate', ['linear'], ['get', 'concentration'],
                    CONCENTRATIONS[0], 'rgb(60, 90, 190)',
                    CONCENTRATIONS[1], 'rgb(80, 140, 200)',
                    CONCENTRATIONS[2], 'rgb(90, 175, 195)',
                    CONCENTRATIONS[3], 'rgb(100, 190, 160)',
                    CONCENTRATIONS[4], 'rgb(140, 200, 120)',
                    CONCENTRATIONS[5], 'rgb(200, 210, 100)',
                    CONCENTRATIONS[6], 'rgb(225, 210, 100)',
                    CONCENTRATIONS[7], 'rgb(225, 170, 90)',
                    CONCENTRATIONS[8], 'rgb(215, 135, 80)',
                    CONCENTRATIONS[9], 'rgb(200, 100, 80)',
                ],
                'fill-opacity': 1.0,
                'fill-antialias': false,
                'fill-outline-color': 'rgba(0,0,0,0)'
            },
        });
        
        map.addSource('particles-active', {
            type: 'geojson',
            data: { type: 'FeatureCollection', features: [] }
        });
        map.addLayer({
            id: 'active-particles-layer',
            type: 'circle',
            source: 'particles-active',
            paint: {
                'circle-radius': 2,
                'circle-color': 'rgb(255, 255, 255)',
                'circle-opacity': 0.7
            }
        });

        map.addSource('particles-inactive', {
            type: 'geojson',
            data: { type: 'FeatureCollection', features: [] }
        });
        map.addLayer({
            id: 'inactive-particles-layer',
            type: 'circle',
            source: 'particles-inactive',
            paint: {
                'circle-radius': 2,
                'circle-color': 'rgb(255, 59, 20)',
                'circle-opacity': 0.7
            }
        });
        
        toggleVisualizationMode();
    });
}

// Toggle between grid and particle visualization
function toggleVisualizationMode() {
    if (visualizationMode === 'grid') {
        map.setLayoutProperty('concentration-fill', 'visibility', 'visible');
        map.setLayoutProperty('active-particles-layer', 'visibility', 'none');
        map.setLayoutProperty('inactive-particles-layer', 'visibility', 'none');
    } else {
        map.setLayoutProperty('concentration-fill', 'visibility', 'none');
        map.setLayoutProperty('active-particles-layer', 'visibility', 'visible');
        map.setLayoutProperty('inactive-particles-layer', 'visibility', 'visible');
    }
}

function toggleParticleMode() {
    visualizationMode = 'particles'
    toggleVisualizationMode();
    heatmapToggle.style.background = 'rgb(255, 255, 255)'
    heatmapToggle.style.color = 'rgb(0, 0, 0)'
    particleToggle.style.background = 'none'
    particleToggle.style.color = 'rgb(255, 255, 255)'
    updateParticleVisualization();
    updateGridVisualization();
}

function toggleHeatmapMode() {
    visualizationMode = 'grid'
    toggleVisualizationMode();
    heatmapToggle.style.background = 'none'
    heatmapToggle.style.color = 'rgb(255, 255, 255)'
    particleToggle.style.background = 'rgb(255, 255, 255)'
    particleToggle.style.color = 'rgb(0, 0, 0)'
    heatmap = new HeatmapGenerator(
        bounding_box[0]-GRID_SIZE,
        bounding_box[1]+GRID_SIZE,
        bounding_box[2]-GRID_SIZE,
        bounding_box[3]+GRID_SIZE,
        GRID_SIZE);
    updateParticleVisualization();
    updateGridVisualization();
}



function updatePositionFromFields() {
    if (!Number.isNaN(lonField.value) && !Number.isNaN(latField.value)) {
        rawLon = lonField.value;
        rawLat = latField.value;
    }
    updateMarker()
}

function updateFields() {
    let displayLon = normalizeLongitude(rawLon).toFixed(2);
    lonField.value = displayLon;
    latField.value = rawLat;
}

function updateMarker() {
    if (!simulationRunning) {
        if (window.currentMarker) {
            window.currentMarker.remove();
        }
        
        window.currentMarker = new maplibregl.Marker({
            color: '#244886',
            scale: 0.9
        })
        .setLngLat([rawLon, rawLat])
        .addTo(map);
    }
}

// Update grid visualization from particle positions
function updateGridVisualization() {
    const positions = proteus.get_positions();

    const lons = [];
    const lats = [];

    for (let i = 0; i < positions.length; i += 2) {
        lons.push(positions[i]);
        lats.push(positions[i + 1]);
    }
    
    // Update grid
    heatmap.clear();
    heatmap.add_particles(lons, lats, null);
    heatmap.smooth();
    
    // Generate contours
    const thresholds = [0.5, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];
    const geojsonStr = heatmap.to_contour_geojson(thresholds);
    const geojson = JSON.parse(geojsonStr);
    map.getSource('concentration').setData(geojson);
}

// Update particle visualization (points)
function updateParticleVisualization() {
    let activePositions = proteus.get_active_positions();
    let inactivePositions = proteus.get_inactive_positions();

    const geojsonActive = {
        type: 'FeatureCollection',
        features: []
    };

    const geojsonInactive = {
        type: 'FeatureCollection',
        features: []
    };
    
    for (let i = 0; i < activePositions.length; i += 2) {
        const lon = activePositions[i];
        const lat = activePositions[i + 1];
        
        if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
            geojsonActive.features.push({
                type: 'Feature',
                geometry: {
                    type: 'Point',
                    coordinates: [lon, lat]
                },
                properties: {}
            });
        }
    }

    for (let i = 0; i < inactivePositions.length; i += 2) {
        const lon = inactivePositions[i];
        const lat = inactivePositions[i + 1];
        
        if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
            geojsonInactive.features.push({
                type: 'Feature',
                geometry: {
                    type: 'Point',
                    coordinates: [lon, lat]
                },
                properties: {}
            });
        }
    }

    map.getSource('particles-active').setData(geojsonActive);
    map.getSource('particles-inactive').setData(geojsonInactive);
}

// Run one simulation step
async function simulationStep(version) {
    if (!simulationRunning || version !== simulationVersion) {
        return;
    }
    
    stepInProgress = true;
    stepCount++;

    try {
        await proteus.step(stepSize);
        // Check if simulation was reset during step
        if (version !== simulationVersion) {
            return;
        }

        const currentDate = proteus.current_date_int();  // ← From Rust

        if (stepCount % (1/stepSize) === 0) {
            const currentTiles = getTileIndices();
            const nextStepDate = addDays(currentDate, 1);
            preloader.preloadTiles(nextStepDate, currentTiles); 
        }
        updateBoundingBox();
        // Update visualization based on mode
        const now = performance.now();
        if (visualizationMode === 'grid') {
            if (now - lastGridUpdate > GRID_UPDATE_INTERVAL) {
                heatmap = new HeatmapGenerator(
                    bounding_box[0]-GRID_SIZE,
                    bounding_box[1]+GRID_SIZE,
                    bounding_box[2]-GRID_SIZE,
                    bounding_box[3]+GRID_SIZE,
                    GRID_SIZE);
                updateGridVisualization();
                lastGridUpdate = now;
            }
        } else {
            updateParticleVisualization();
        }
        let day = proteus.current_day();
        dayDisplay.textContent = proteus.current_date_str();
        
        if (simulationRunning && version === simulationVersion && day <= totalDays) {
            animationId = requestAnimationFrame(() => simulationStep(version));
        }
    } catch (error) {
        simulationRunning = false;
    } finally {
        stepInProgress = false;
    }
}

// Start simulation
function startSimulation() {
    if (simulationRunning) {
        return;
    }
    
    // Reset visualization and state
    simulationRunning = true;
    simulationVersion++;
    lastGridUpdate = 0;
    concentrationGrid = null;
    
    let lon = normalizeLongitude(rawLon);
    let lat = rawLat;
    
    updateSimulationDate();
    updateTotalDays();
    proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
    
    // Update UI
    startBtn.style.display = "none";
    stopBtn.style.display = "inline-flex";
    resumeBtn.style.display = "none";

    const currentZoom = map.getZoom();
    if (currentZoom < 2.5) {
        map.flyTo({
            center: [lon, lat],
            zoom: 6-(totalDays/100),  // Adjust zoom level (lower = further out, higher = closer)
            duration: 1500,  // Animation duration in milliseconds
            essential: true  // Ensures the animation happens even if user prefers reduced motion
        });
    }

    simulationStep(simulationVersion);
}

// Stop simulation
function stopSimulation() {
    simulationRunning = false;
    
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
    
    startBtn.style.display = "none";
    stopBtn.style.display = "none";
    resumeBtn.style.display = "inline-flex";
}

// Resume simulation
function resumeSimulation() {
    if (simulationRunning) return;
    simulationRunning = true;
    simulationVersion++;
    startBtn.style.display = "none";
    stopBtn.style.display = "inline-flex";
    resumeBtn.style.display = "none";
    simulationStep(simulationVersion);
}

// Reset simulation
async function resetSimulation() {
    
    simulationRunning = false;
    simulationVersion++;
    stepCount = 0;
    
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
    
    concentrationGrid = null;
    lastGridUpdate = 0;

    let lon = normalizeLongitude(rawLon);
    let lat = rawLat;

    proteus = new Proteus(lon, lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);


    map.getSource('concentration').setData({ type: 'FeatureCollection', features: [] });
    map.getSource('particles-active').setData({ type: 'FeatureCollection', features: [] });
    map.getSource('particles-inactive').setData({ type: 'FeatureCollection', features: [] });
    
    dayDisplay.textContent = proteus.current_date_str();
    
    startBtn.style.display = "inline-flex";
    stopBtn.style.display = "none";
    resumeBtn.style.display = "none";

    updateMarker();
    updateSimulationDate();
    updateTotalDays();

}

function updateSimulationDate() {
    if (!simulationRunning) {
        let inputDate = startDate.value.split("-");
        startYear = inputDate[0];
        startMonth = inputDate[1];
        startDay = inputDate[2];
    }
}

function updateTotalDays() {
    if (!simulationRunning) {
        totalDays = totalDaysField.value;
    }
}

// Map click
map.on('click', function(e) {
    if (!simulationRunning) {
        rawLon = e.lngLat.lng.toFixed(2);
        rawLat = e.lngLat.lat.toFixed(2);
        
        updateFields();
        updateMarker();
    }
});

// Initialize when page loads
initialize().catch(console.error);