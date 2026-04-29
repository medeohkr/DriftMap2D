

import init, { Proteus, setup_panic_hook } from './pkg/proteus.js';

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
    center: [165, 25],
    zoom: 3
});


// DOM elements
const latField = document.querySelector('.lat-field');
const lonField = document.querySelector('.lon-field');
const startBtn = document.getElementById('start-simulation');
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

// Initialize WASM and UI
async function initialize() {
    await init();
    setup_panic_hook();
    proteus = new Proteus();
    console.log('Proteus engine initialized');
    
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
    
    // Update engine release location (normalized)
    proteus.set_release_location(normalizeLongitude(lon), lat);
}

// Run one simulation step
async function simulationStep() {
    if (!simulationRunning) return;
    
    // Step forward by 0.5 days
    await proteus.step(0.5);
    
    // Get positions
    const positions = proteus.get_positions();
    currentPositions = positions;
    
    // Update display
    const day = proteus.current_day();
    if (dayDisplay) {
        dayDisplay.textContent = day.toFixed(1);
    }
    
    // Update visualization
    updateParticleVisualization(positions);
    
    // Schedule next step
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
                    'circle-radius': 3,
                    'circle-color': '#ff6b6b',
                    'circle-opacity': 0.8
                }
            });
        }
    }
}

// Start simulation
function startSimulation() {
    if (simulationRunning) return;
    
    // Reset engine
    proteus.reset();
    
    // Set release location from current marker
    let lon = parseFloat(lonField.value);
    let lat = parseFloat(latField.value);
    proteus.set_release_location(normalizeLongitude(lon), lat);
    
    // Start simulation
    simulationRunning = true;
    simulationStep();
}

// Stop simulation
function stopSimulation() {
    simulationRunning = false;
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
}

// Event listeners
startBtn.addEventListener('click', startSimulation);
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