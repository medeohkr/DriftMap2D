// eulerGrid.js
// Eulerian grid for concentration visualization
// No arbitrary radius, deterministic, scientific

export class EulerianGrid {
    constructor(lonMin, lonMax, latMin, latMax, cellSizeDeg = 0.2) {
        this.lonMin = lonMin;
        this.lonMax = lonMax;
        this.latMin = latMin;
        this.latMax = latMax;
        this.cellSize = cellSizeDeg;
        
        // Calculate grid dimensions
        this.nx = Math.ceil((lonMax - lonMin) / cellSizeDeg);
        this.ny = Math.ceil((latMax - latMin) / cellSizeDeg);
        
        // Concentration grid (flattened array)
        this.grid = new Float32Array(this.nx * this.ny);
        
        // For smoothing
        this.smoothKernel = [
            1/16, 2/16, 1/16,
            2/16, 4/16, 2/16,
            1/16, 2/16, 1/16
        ];
    }
    
    // Clear all grid cells
    clear() {
        this.grid.fill(0);
    }
    
    // Add particle concentration to grid
    addParticle(lon, lat, concentration = 1.0) {
        const ix = Math.floor((lon - this.lonMin) / this.cellSize);
        const iy = Math.floor((lat - this.latMin) / this.cellSize);
        
        if (ix >= 0 && ix < this.nx && iy >= 0 && iy < this.ny) {
            const idx = iy * this.nx + ix;
            this.grid[idx] += concentration;
        }
    }
    
    // Add multiple particles (batch for performance)
    addParticles(positions, concentrations = null) {
        if (concentrations && positions.length !== concentrations.length * 2) {
            console.warn('Positions and concentrations length mismatch');
            return;
        }
        
        for (let i = 0; i < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];
            const conc = concentrations ? concentrations[i / 2] : 1.0;
            
            const ix = Math.floor((lon - this.lonMin) / this.cellSize);
            const iy = Math.floor((lat - this.latMin) / this.cellSize);
            
            if (ix >= 0 && ix < this.nx && iy >= 0 && iy < this.ny) {
                const idx = iy * this.nx + ix;
                this.grid[idx] += conc;
            }
        }
    }
    
    // Apply Gaussian smoothing (reduces noise, creates smooth gradient)
    smooth() {
        const smoothed = new Float32Array(this.grid.length);
        
        for (let iy = 1; iy < this.ny - 1; iy++) {
            for (let ix = 1; ix < this.nx - 1; ix++) {
                let sum = 0;
                for (let ky = -1; ky <= 1; ky++) {
                    for (let kx = -1; kx <= 1; kx++) {
                        const gridIdx = (iy + ky) * this.nx + (ix + kx);
                        const kernelIdx = (ky + 1) * 3 + (kx + 1);
                        sum += this.grid[gridIdx] * this.smoothKernel[kernelIdx];
                    }
                }
                const smoothedIdx = iy * this.nx + ix;
                smoothed[smoothedIdx] = sum;
            }
        }
        
        // Copy back (edges remain unchanged)
        for (let iy = 1; iy < this.ny - 1; iy++) {
            for (let ix = 1; ix < this.nx - 1; ix++) {
                const idx = iy * this.nx + ix;
                this.grid[idx] = smoothed[idx];
            }
        }
    }
    
    // Apply log scaling for visualization (concentrations span many orders of magnitude)
    applyLogScale(minValue = 1e-6) {
        for (let i = 0; i < this.grid.length; i++) {
            if (this.grid[i] > 0) {
                this.grid[i] = Math.log10(Math.max(this.grid[i], minValue));
            }
        }
    }
    
    // Normalize to 0-1 range for visualization
    normalize(minVal, maxVal) {
        const range = maxVal - minVal;
        if (range <= 0) return;
        
        for (let i = 0; i < this.grid.length; i++) {
            this.grid[i] = (this.grid[i] - minVal) / range;
            this.grid[i] = Math.min(1.0, Math.max(0.0, this.grid[i]));
        }
    }
    
    // Get min and max concentration (for color scaling)
    getRange() {
        let minVal = Infinity;
        let maxVal = -Infinity;
        
        for (let i = 0; i < this.grid.length; i++) {
            if (this.grid[i] > 0) {
                minVal = Math.min(minVal, this.grid[i]);
                maxVal = Math.max(maxVal, this.grid[i]);
            }
        }
        
        return { min: minVal === Infinity ? 0 : minVal, max: maxVal === -Infinity ? 1 : maxVal };
    }
    
    // Get GeoJSON features for MapLibre fill layer
    toGeoJSON() {
        const features = [];
        
        for (let iy = 0; iy < this.ny; iy++) {
            for (let ix = 0; ix < this.nx; ix++) {
                const val = this.grid[iy * this.nx + ix];
                if (val === 0) continue;
                
                const lon = this.lonMin + ix * this.cellSize;
                const lat = this.latMin + iy * this.cellSize;
                
                features.push({
                    type: 'Feature',
                    geometry: {
                        type: 'Polygon',
                        coordinates: [[
                            [lon, lat],
                            [lon + this.cellSize, lat],
                            [lon + this.cellSize, lat + this.cellSize],
                            [lon, lat + this.cellSize],
                            [lon, lat]
                        ]]
                    },
                    properties: {
                        concentration: val
                    }
                });
            }
        }
        
        return {
            type: 'FeatureCollection',
            features: features
        };
    }
    
    // Get canvas-ready data array (for custom canvas rendering)
    toCanvasData(width, height, bounds) {
        const data = new Uint8Array(width * height * 4);
        const lonRange = bounds.east - bounds.west;
        const latRange = bounds.north - bounds.south;
        
        for (let iy = 0; iy < this.ny; iy++) {
            const lat = this.latMin + iy * this.cellSize;
            const y = ((bounds.north - lat) / latRange) * height;
            
            for (let ix = 0; ix < this.nx; ix++) {
                const val = this.grid[iy * this.nx + ix];
                if (val === 0) continue;
                
                const lon = this.lonMin + ix * this.cellSize;
                const x = ((lon - bounds.west) / lonRange) * width;
                
                const w = width * this.cellSize / lonRange;
                const h = height * this.cellSize / latRange;
                
                // Color mapping (concentration to RGB)
                const intensity = Math.min(1, val);
                const r = Math.floor(255 * intensity);
                const g = Math.floor(255 * (1 - intensity) * 0.5);
                const b = Math.floor(255 * (1 - intensity));
                const a = 200;
                
                // Fill rectangle in canvas
                // (This is simplified; full implementation would need canvas context)
            }
        }
        
        return data;
    }
    
    // Get total mass/concentration in grid (for debugging)
    getTotalMass() {
        let total = 0;
        for (let i = 0; i < this.grid.length; i++) {
            total += this.grid[i];
        }
        return total;
    }
    
    // Resize grid (adjust resolution, preserve total mass)
    resize(newCellSizeDeg) {
        const newGrid = new EulerianGrid(
            this.lonMin, this.lonMax,
            this.latMin, this.latMax,
            newCellSizeDeg
        );
        
        // Conservative remapping (mass-preserving)
        for (let iy = 0; iy < this.ny; iy++) {
            for (let ix = 0; ix < this.nx; ix++) {
                const val = this.grid[iy * this.nx + ix];
                if (val === 0) continue;
                
                const lon = this.lonMin + ix * this.cellSize;
                const lat = this.latMin + iy * this.cellSize;
                
                newGrid.addParticle(lon + this.cellSize / 2, lat + this.cellSize / 2, val);
            }
        }
        
        return newGrid;
    }
}

// ============= HELPER FUNCTIONS =============

// Create grid that covers current particle bounds
export function createAdaptiveGrid(positions, cellSizeDeg = 0.2, marginDeg = 5.0) {
    if (!positions || positions.length === 0) {
        // Return default Pacific grid
        return new EulerianGrid(-180, 180, -80, 90, cellSizeDeg);
    }
    
    let minLon = Infinity, maxLon = -Infinity;
    let minLat = Infinity, maxLat = -Infinity;
    
    for (let i = 0; i < positions.length; i += 2) {
        const lon = positions[i];
        const lat = positions[i + 1];
        minLon = Math.min(minLon, lon);
        maxLon = Math.max(maxLon, lon);
        minLat = Math.min(minLat, lat);
        maxLat = Math.max(maxLat, lat);
    }
    
    // Add margin
    minLon = Math.max(-180, minLon - marginDeg);
    maxLon = Math.min(180, maxLon + marginDeg);
    minLat = Math.max(-80, minLat - marginDeg);
    maxLat = Math.min(90, maxLat + marginDeg);
    
    return new EulerianGrid(minLon, maxLon, minLat, maxLat, cellSizeDeg);
}

// Color function for concentration values
export function getColorForConcentration(value, isLogScaled = true) {
    if (value <= 0) return 'rgba(0,0,0,0)';
    
    // Use log scale for better visual range
    let intensity;
    if (isLogScaled) {
        intensity = Math.min(1, value / 6); // log10 range ~0-6
    } else {
        intensity = Math.min(1, value);
    }
    
    // Color gradient: blue (low) -> green -> yellow -> red (high)
    if (intensity < 0.33) {
        const t = intensity / 0.33;
        return `rgba(0, ${Math.floor(100 + 155 * t)}, 255, 0.7)`;
    } else if (intensity < 0.66) {
        const t = (intensity - 0.33) / 0.33;
        return `rgba(${Math.floor(255 * t)}, 255, ${Math.floor(255 * (1 - t))}, 0.7)`;
    } else {
        const t = (intensity - 0.66) / 0.34;
        return `rgba(255, ${Math.floor(255 * (1 - t))}, 0, 0.8)`;
    }
}

// Update grid from particle positions (main loop)
export function updateGridFromParticles(grid, positions, concentrations = null, smooth = true) {
    grid.clear();
    grid.addParticles(positions, concentrations);
    
    if (smooth) {
        grid.smooth();
    }
    
    return grid;
}

// Get color ramp for legend
export function getColorRamp(stops = 10) {
    const ramp = [];
    for (let i = 0; i <= stops; i++) {
        const intensity = i / stops;
        ramp.push(getColorForConcentration(intensity * 6, true));
    }
    return ramp;
}