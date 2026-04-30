import init, { Proteus, setup_panic_hook } from './pkg/proteus.js';
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
    renderWorldCopies: false,
    maxBounds: [
        [-180+(1/12), -80],
        [180, 85.05]
    ]
});


// DOM elements
const latField = document.querySelector('.lat-field');
const lonField = document.querySelector('.lon-field');
const startBtn = document.getElementById('start-simulation');
const resumeBtn = document.getElementById('resume-simulation')
const stopBtn = document.getElementById('stop-simulation');
const resetBtn = document.getElementById('reset-simulation');
const dayDisplay = document.getElementById('current-day');
// State
let proteus = null;
let simulationRunning = false;
let animationId = null;
let currentPositions = [];

// Normalize longitude
function normalizeLongitude(lon) {
    lon = parseFloat(lon);
    lon = ((lon + 180) % 360 + 360) % 360 - 180;
    return lon;
}
function getTileIndicesFromPositions(positions, tileSize = 5.0) {
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
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    proteus = new Proteus(normalizeLongitude(lon), lat);
    
    // Set initial marker
    updateMarkerFromFields();
}

// Update marker from lat/lon fields
function updateMarkerFromFields() {
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    
    if (isNaN(lon) || isNaN(lat)) return;
    
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

async function simulationStep() {
    if (!simulationRunning) return;
    
    await proteus.step(1/24);
    

    const positions = proteus.get_positions();
    // const currentDate = proteus.current_date_int();  // ← From Rust
    // const currentTiles = getTileIndicesFromPositions(positions);
    // const nextStepDate = addDays(currentDate, 1/24);
    // preloader.preloadTiles(nextStepDate, currentTiles); 
    
    const day = proteus.current_day();
    if (dayDisplay) {
        dayDisplay.textContent = day.toFixed(1);
    }
    
    updateParticleVisualization(positions);
    
    if (simulationRunning) {
        animationId = requestAnimationFrame(() => simulationStep());
    }
}
// Update map with particles
function updateParticleVisualization(positions) {
    // Remove old markers/circles
    if (window.particleLayer) {
        map.removeLayer(window.particleLayer);
    }
    
    // Create new layer
    const geojson = {
        type: 'FeatureCollection',
        features: []
    };
    
    for (let i = 0; i < positions.length; i += 2) {
        const lon = positions[i];
        const lat = positions[i + 1];
        
        if (Math.abs(lat) <= 90) {
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
    
    // Add to map (using MapLibre GL)
    if (map.loaded()) {
        if (map.getSource('particles')) {
            map.getSource('particles').setData(geojson);
        } else {
            map.addSource('particles', {
                type: 'geojson',
                data: geojson
            });
            map.addLayer({
                id: 'particles',
                type: 'circle',
                source: 'particles',
                paint: {
                    'circle-radius': 2,
                    'circle-color': '#ffffff',
                    'circle-opacity': 0.8
                }
            });
        }
    }
}

// Start simulation
function startSimulation() {
    if (simulationRunning) return;
    
    stopBtn.style.display = "inline-flex";
    resumeBtn.style.display = "none"
    startBtn.style.display = "none";

    // Set release location from current marker
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    proteus = new Proteus(normalizeLongitude(lon), lat);
    
    // Start simulation
    simulationRunning = true;
    simulationStep();
}

function resumeSimulation() {
    simulationRunning = true;
    simulationStep();
    stopBtn.style.display = "inline-flex";
    resumeBtn.style.display = "none"
    startBtn.style.display = "none";
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
}

// Stop simulation
function stopSimulation() {
    simulationRunning = false;
    stopBtn.style.display = "none";
    resumeBtn.style.display = "inline-flex"
    startBtn.style.display = "none";
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
}

function resetSimulation() {
    simulationRunning = false;
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
    
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    proteus = new Proteus(normalizeLongitude(lon), lat);
    
    updateParticleVisualization([]);  // Clear particles
    dayDisplay.textContent = "0.0";
    
    stopBtn.style.display = "none";
    resumeBtn.style.display = "none";
    startBtn.style.display = "inline-flex";
}

// Event listeners
startBtn.addEventListener('click', startSimulation);
resumeBtn.addEventListener('click', resumeSimulation);
stopBtn.addEventListener('click', stopSimulation);
resetBtn.addEventListener('click', resetSimulation);
latField.addEventListener('input', updateMarkerFromFields);
lonField.addEventListener('input', updateMarkerFromFields);

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