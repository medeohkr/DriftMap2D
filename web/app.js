import init, { Proteus, setup_panic_hook } from './pkg/proteus.js';
import { EulerianGrid, createAdaptiveGrid, updateGridFromParticles } from './eulerGrid.js';
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
    // renderWorldCopies: false,
    // maxBounds: [
    //     [-180 + (1/12), -80],
    //     [180, 85.05]
    // ]
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
const heatmapToggle = document.getElementById('heatmap-toggle');
const particleToggle = document.getElementById('particle-toggle');
// Event listeners
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

// State
let proteus = null;
let simulationRunning = false;
let stepInProgress = false;
let animationId = null;
let simulationVersion = 0;

// Grid visualization
let concentrationGrid = null;
let lastGridUpdate = 0;
const GRID_UPDATE_INTERVAL = 500; 
let visualizationMode = 'grid'; 
let rawLon = 56.54;
let rawLat = 26.74;
let kValue = 50;
let particleCount = 20000;
let spreadKm = 1;
let oilType = oilMenu ? oilMenu.value : 'arabian_light';
let startYear = 2025;
let startMonth = 5;
let startDay = 1;
let stepSize = 1/24;
let totalDays = 10;
let isError = false;
let stepCount = 0;
// Normalize longitude
function normalizeLongitude(lon) {
    lon = parseFloat(lon);
    lon = ((lon + 180) % 360 + 360) % 360 - 180;
    return lon;
}
function getTileIndicesFromPositions(positions, tileSize = 10.0) {
    const tiles = new Set();
    
    for (let i = 0; i < positions.length; i += 2) {
        const lon = positions[i];
        const lat = positions[i + 1];
        
        // Calculate tile index (same as Rust: (lon - min_lon) / tile_size).floor()
        const minLon = -180;
        const minLat = -80;
        
        const lonIdx = Math.floor((lon - minLon) / tileSize);
        const latIdx = Math.floor((lat - minLat) / tileSize);
        
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


// Initialize WASM and UI
async function initialize() {
    await init();
    setup_panic_hook();
    
    let lon = normalizeLongitude(rawLon);
    let lat = rawLat
    
    proteus = new Proteus(normalizeLongitude(lon), lat, kValue, particleCount, spreadKm, startYear, startMonth, startDay);
    
    initGridLayer();
    updateMarker();
}

function initGridLayer() {
    map.on('load', () => {
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
                    'interpolate', ['linear'], ['get', 'concentration'],
                    0, 'rgb(231, 236, 251)',
                    1, 'rgb(195, 209, 247)',
                    2, 'rgb(162, 186, 244)',
                    4, 'rgb(120, 153, 227)',
                    8, 'rgb(68, 115, 227)',
                    16, 'rgb(141, 142, 213)',
                    32, 'rgb(252, 184, 197)',
                    64, 'rgb(255, 115, 107)',
                    128, 'rgb(254, 79, 44)',
                    256, 'rgb(255, 106, 0)',
                    512, 'rgb(255, 154, 0)',
                    1024, 'rgb(255, 216, 1)',
                ],
                'fill-opacity': 0.7,
                'fill-antialias': false,
                'fill-outline-color': 'rgba(0,0,0,0)'
            }
        });
        
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
                'circle-color': '#ffffff',
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
        map.setLayoutProperty('particles-layer', 'visibility', 'none');
    } else {
        map.setLayoutProperty('concentration-fill', 'visibility', 'none');
        map.setLayoutProperty('particles-layer', 'visibility', 'visible');
    }
}

function toggleParticleMode() {
    visualizationMode = 'particles'
    toggleVisualizationMode();
    heatmapToggle.style.background = 'rgb(255, 255, 255)'
    heatmapToggle.style.color = 'rgb(0, 0, 0)'
    particleToggle.style.background = 'none'
    particleToggle.style.color = 'rgb(255, 255, 255)'
    updateParticleVisualization(proteus.get_positions());
    updateGridVisualization(proteus.get_positions());
}

function toggleHeatmapMode() {
    visualizationMode = 'grid'
    toggleVisualizationMode();
    heatmapToggle.style.background = 'none'
    heatmapToggle.style.color = 'rgb(255, 255, 255)'
    particleToggle.style.background = 'rgb(255, 255, 255)'
    particleToggle.style.color = 'rgb(0, 0, 0)'
    updateParticleVisualization(proteus.get_positions());
    updateGridVisualization(proteus.get_positions());
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
        
        const positions = proteus.get_positions();

        const currentDate = proteus.current_date_int();  // ← From Rust

        if (stepCount % (1/stepSize) === 0) {
            const currentTiles = getTileIndicesFromPositions(positions);
            const nextStepDate = addDays(currentDate, 1);
            preloader.preloadTiles(nextStepDate, currentTiles); 
        }
        
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

        dayDisplay.textContent = `Day: ${day.toFixed(1)}`;
        
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

    map.flyTo({
        center: [lon, lat],
        zoom: 5.5-(totalDays/100),  // Adjust zoom level (lower = further out, higher = closer)
        duration: 1500,  // Animation duration in milliseconds
        essential: true  // Ensures the animation happens even if user prefers reduced motion
    });
    
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
    
    simulationRunning = false;
    simulationVersion++;
    stepCount = 0;
    
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
    
    concentrationGrid = null;
    lastGridUpdate = 0;
    
    if (map.getSource('concentration')) {
        map.getSource('concentration').setData({ type: 'FeatureCollection', features: [] });
    }
    if (map.getSource('particles')) {
        map.getSource('particles').setData({ type: 'FeatureCollection', features: [] });
    }
    
    dayDisplay.textContent = "Day: 0.0";
    
    startBtn.style.display = "inline-flex";
    stopBtn.style.display = "none";
    resumeBtn.style.display = "none";

    updateMarker();
    updateTotalDays();
}

function updateSimulationDate() {
    let inputDate = startDate.value.split("-");
    startYear = inputDate[0];
    startMonth = inputDate[1];
    startDay = inputDate[2];
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