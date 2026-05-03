import init, { Proteus, setup_panic_hook } from './pkg/proteus.js';
import { EulerianGrid, createAdaptiveGrid, updateGridFromParticles } from './eulerGrid.js';

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
    renderWorldCopies: false,
    maxBounds: [
        [-180 + (1/12), -80],
        [180, 85.05]
    ]
});

// DOM elements
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

// Event listeners
startBtn.addEventListener('click', startSimulation);
stopBtn.addEventListener('click', stopSimulation);
resumeBtn.addEventListener('click', resumeSimulation);
resetBtn.addEventListener('click', resetSimulation);
latField.addEventListener('input', updateMarkerFromFields);
lonField.addEventListener('input', updateMarkerFromFields);
startDate.addEventListener('input', updateSimulationDate);
totalDaysField.addEventListener('input', updateTotalDays);
// State
let proteus = null;
let simulationRunning = false;
let stepInProgress = false;
let animationId = null;
let currentPositions = [];
let simulationVersion = 0;

// Grid visualization
let concentrationGrid = null;
let lastGridUpdate = 0;
const GRID_UPDATE_INTERVAL = 1000; 
let visualizationMode = 'grid'; 
let kValue = 100;
let particleCount = 20000;
let spreadKm = 1;
let oilType = oilMenu ? oilMenu.value : 'arabian_light';
let startYear = 2025;
let startMonth = 5;
let startDay = 1;
let stepSize = 1/24;
let dayCounter = 0;
let totalDays = 10;
let isError = false;

// Normalize longitude
function normalizeLongitude(lon) {
    lon = parseFloat(lon);
    lon = ((lon + 180) % 360 + 360) % 360 - 180;
    return lon;
}

// Initialize WASM and UI
async function initialize() {
    await init();
    setup_panic_hook();
    
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    
    proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
    console.log('Proteus engine initialized');
    
    // Initialize grid layer on map
    initGridLayer();
    
    // Set initial marker
    updateMarkerFromFields();
}

// Initialize MapLibre grid layer
function initGridLayer() {
    // Add source for concentration grid
    map.addSource('concentration', {
        type: 'geojson',
        data: { type: 'FeatureCollection', features: [] }
    });
    map.addLayer({
        id: 'concentration-fill',
        type: 'fill',
        source: 'concentration',
        paint: {
            'fill-color': [
                'interpolate',
                ['linear'],
                ['get', 'concentration'],
                0, 'rgb(231, 236, 251)',
                1, 'rgb(195, 209, 247)',
                2, 'rgb(162, 186, 244)',
                4, 'rgb(120, 153, 227)',
                8, 'rgb(68, 115, 227)',
                16, 'rgb(141, 142, 213)',
                32, 'rgb(252, 184, 197)',
                64, 'rgb(255, 115, 107)',
                128, 'rgb(251, 64, 26)',
                256, 'rgb(255, 106, 0)',
                512, 'rgb(255, 154, 0)',
                1024, 'rgb(255, 216, 1)',
            ],
            'fill-opacity': 0.7,
            'fill-antialias': false,                     // ← No hairline borders
            'fill-outline-color': 'rgba(0,0,0,0)'        // ← Extra safety
        }
    });
    
    // Add particle layer (for visualization mode)
    map.addSource('particles', {
        type: 'geojson',
        data: { type: 'FeatureCollection', features: [] }
    });
    
    map.addLayer({
        id: 'particles-layer',
        type: 'circle',
        source: 'particles',
        paint: {
            'circle-radius': 2,
            'circle-color': '#ff6b6b',
            'circle-opacity': 0.7
        }
    });
    
    // Toggle layers based on visualization mode
    toggleVisualizationMode();
}

// Toggle between grid and particle visualization
function toggleVisualizationMode() {
    if (visualizationMode === 'grid') {
        map.setLayoutProperty('concentration-fill', 'visibility', 'visible');
        map.setLayoutProperty('particles-layer', 'visibility', 'none');
    } else {
        map.setLayoutProperty('concentration-fill', 'visibility', 'none');
        map.setLayoutProperty('particles-layer', 'visibility', 'visible');
    }
}

// Update marker from lat/lon fields
function updateMarkerFromFields() {
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    
    if (window.currentMarker) {
        window.currentMarker.remove();
    }
    
    window.currentMarker = new maplibregl.Marker({
        color: '#244886',
        scale: 0.9
    })
    .setLngLat([lon, lat])
    .addTo(map);
}

// Update grid visualization from particle positions
function updateGridVisualization(positions) {
    if (!positions || positions.length === 0) {
        return;
    }
    
    // Create or resize grid adaptively
    if (!concentrationGrid) {
        concentrationGrid = createAdaptiveGrid(positions, 0.1, 100.0);
    }
    
    // Update grid with current particle positions
    // For oil, use concentration = 1.0 per particle (mass-based)
    updateGridFromParticles(concentrationGrid, positions, null, true);
    
    // Apply log scaling for better visualization
    const range = concentrationGrid.getRange();
    if (range.max > 0) {
        // Store original for later, but use log values for color
        // The GeoJSON uses raw values, map style handles interpolation
    }
    
    // Update map source
    if (map.getSource('concentration')) {
        map.getSource('concentration').setData(concentrationGrid.toGeoJSON());
    }
}

// Update particle visualization (points)
function updateParticleVisualization(positions) {
    const geojson = {
        type: 'FeatureCollection',
        features: []
    };
    
    for (let i = 0; i < positions.length; i += 2) {
        const lon = positions[i];
        const lat = positions[i + 1];
        
        if (Math.abs(lat) <= 90 && lon >= -180 && lon <= 180) {
            geojson.features.push({
                type: 'Feature',
                geometry: {
                    type: 'Point',
                    coordinates: [lon, lat]
                },
                properties: {}
            });
        }
    }
    
    if (map.getSource('particles')) {
        map.getSource('particles').setData(geojson);
    }
}

// Run one simulation step
async function simulationStep(version) {
    if (!simulationRunning || version !== simulationVersion) {
        console.log("Step skipped: not running or version mismatch");
        return;
    }
    
    stepInProgress = true;
    console.log("Step starting, version:", version);
    dayCounter += stepSize;

    try {

        await proteus.step(stepSize);
        
        // Check if simulation was reset during step
        if (version !== simulationVersion) {
            console.log("Step aborted: version changed");
            return;
        }
        
        const positions = proteus.get_positions();
        currentPositions = positions;
        
        // Update visualization based on mode
        const now = performance.now();
        if (visualizationMode === 'grid') {
            if (now - lastGridUpdate > GRID_UPDATE_INTERVAL) {
                updateGridVisualization(positions);
                lastGridUpdate = now;
            }
        } else {
            updateParticleVisualization(positions);
        }
        
        const day = proteus.current_day();
        if (dayDisplay) {
            dayDisplay.textContent = day.toFixed(1);
        }
        
        if (simulationRunning && version === simulationVersion && dayCounter <= totalDays) {
            animationId = requestAnimationFrame(() => simulationStep(version));
        }
    } catch (error) {
        console.error("Simulation step error:", error);
        simulationRunning = false;
    } finally {
        stepInProgress = false;
    }
}

// Start simulation
function startSimulation() {
    if (simulationRunning) {
        console.log("Simulation already running");
        return;
    }
    
    // Reset visualization and state
    simulationRunning = true;
    simulationVersion++;
    lastGridUpdate = 0;
    concentrationGrid = null;
    
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    
    updateSimulationDate();
    updateTotalDays();
    proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
    
    // Update UI
    startBtn.style.display = "none";
    stopBtn.style.display = "inline-flex";
    resumeBtn.style.display = "none";
    
    // Start simulation loop
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
    console.log("Reset started");
    
    simulationRunning = false;
    simulationVersion++;
    dayCounter = 0;
    
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
    
    if (stepInProgress) {
        await new Promise(resolve => {
            const checkInterval = setInterval(() => {
                if (!stepInProgress) {
                    clearInterval(checkInterval);
                    resolve();
                }
            }, 10);
        });
    }
    
    await new Promise(resolve => setTimeout(resolve, 50));
    
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    
    proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
    
    concentrationGrid = null;
    currentPositions = [];
    lastGridUpdate = 0;
    
    if (map.getSource('concentration')) {
        map.getSource('concentration').setData({ type: 'FeatureCollection', features: [] });
    }
    if (map.getSource('particles')) {
        map.getSource('particles').setData({ type: 'FeatureCollection', features: [] });
    }
    
    if (dayDisplay) dayDisplay.textContent = "0.0";
    
    startBtn.style.display = "inline-flex";
    stopBtn.style.display = "none";
    resumeBtn.style.display = "none";
    
    console.log("Reset complete");
}

function updateSimulationDate() {
    let inputDate = startDate.value.split("-");
    startYear = inputDate[0];
    startMonth = inputDate[1];
    startDay = inputDate[2];
}

function updateTotalDays() {
    totalDays = totalDaysField.value;
}


// Oil type change handler
if (oilMenu) {
    oilMenu.addEventListener('change', () => {
        if (proteus && !simulationRunning) {
            let lon = parseFloat(lonField.value);
            let lat = parseFloat(latField.value);
            const oilType = oilMenu.value;
            proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
            console.log("Oil type changed to:", oilType);
        }
    });
}

// Map click
map.on('click', function(e) {
    let rawLon = e.lngLat.lng;
    let rawLat = e.lngLat.lat;
    
    let displayLon = rawLon.toFixed(2);
    if (rawLon < -180 || rawLon > 180) {
        displayLon = normalizeLongitude(rawLon).toFixed(2);
    }
    let displayLat = rawLat.toFixed(2);
    
    latField.value = displayLat;
    lonField.value = displayLon;
    
    updateMarkerFromFields();
});

// Initialize when page loads
initialize().catch(console.error);