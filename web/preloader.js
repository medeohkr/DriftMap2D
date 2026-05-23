if (!window.__tileCache) {
    window.__tileCache = new Map();
}

export class TilePreloader {
    constructor() {
        this.pending = new Map();      // url → Promise (in-flight requests)
        this.completed = new Set();    // urls already loaded
        this.baseUrl = "https://tiles.driftmap2d.com/tiles";
        this.landmaskUrl = "https://tiles.driftmap2d.com/roaring_landmask";
    }
    
    // ========== LANDMASK METHODS (min_lat = -90°) ==========
    
    preloadLandmask(lonIdx, latIdx) {
        const url = `${this.landmaskUrl}/landmask_${String(lonIdx).padStart(3, '0')}_${String(latIdx).padStart(3, '0')}.bin`;
        
        if (this.completed.has(url) || this.pending.has(url)) return;
        
        const promise = fetch(url)
            .then(response => {
                // if (!response.ok) throw new Error(`HTTP ${response.status}`);
                return response.arrayBuffer();
            })
            .then(buffer => {
                if (!window.__tileCache) window.__tileCache = new Map();
                window.__tileCache.set(url, new Uint8Array(buffer));
                this.completed.add(url);
                this.pending.delete(url);
            })
            .catch(err => {
                console.warn(`Landmask preload failed: ${url}`, err);
                this.pending.delete(url);
            });
        
        this.pending.set(url, promise);
    }
    
    preloadLandmaskTiles(tileIndices) {
        for (const { lonIdx, latIdx } of tileIndices) {
            this.preloadLandmask(lonIdx, latIdx);
        }
    }
    
    getTileIndicesForLandmask(positions, bufferTiles = 0) {
        const tiles = new Set();
        
        for (let i = 0; i < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];
            
            // Landmask uses min_lat = -90°
            const centerLonIdx = Math.floor((lon + 180) / 10);
            const centerLatIdx = Math.floor((lat + 90) / 10);
            
            for (let dx = -bufferTiles; dx <= bufferTiles; dx++) {
                for (let dy = -bufferTiles; dy <= bufferTiles; dy++) {
                    const lonIdx = centerLonIdx + dx;
                    const latIdx = centerLatIdx + dy;
                    // 36 longitude tiles (360° / 10°), 18 latitude tiles (180° / 10°)
                    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 18) {
                        tiles.add({ lonIdx, latIdx });
                    }
                }
            }
        }
        
        return Array.from(tiles);
    }
    
    // ========== OCEAN METHODS (min_lat = -80°) ==========
    
    getUrl(date, lonIdx, latIdx) {
        const year = Math.floor(date / 10000);
        const month = Math.floor((date % 10000) / 100);
        const day = date % 100;
        const monthStr = month.toString().padStart(2, '0');
        const dayStr = day.toString().padStart(2, '0');
        return `${this.baseUrl}/${year}/${monthStr}/${dayStr}/${lonIdx.toString().padStart(3, '0')}_${latIdx.toString().padStart(3, '0')}.bin`;
    }
    
    preloadTile(date, lonIdx, latIdx) {
        const url = this.getUrl(date, lonIdx, latIdx);
        
        if (this.completed.has(url)) return;
        if (this.pending.has(url)) return;
        
        const promise = fetch(url)
            .then(response => {
                // if (!response.ok) throw new Error(`HTTP ${response.status}`);
                return response.arrayBuffer();
            })
            .then(buffer => {
                const uint8 = new Uint8Array(buffer);
                if (!window.__tileCache) window.__tileCache = new Map();
                window.__tileCache.set(url, uint8);
                this.completed.add(url);
                this.pending.delete(url);
                return uint8;
            })
            .catch(err => {
                console.warn(`Preload failed: ${url}`, err);
                this.pending.delete(url);
            });
        
        this.pending.set(url, promise);
    }
    
    preloadTiles(date, tileIndices) {
        for (const { lonIdx, latIdx } of tileIndices) {
            this.preloadTile(date, lonIdx, latIdx);
        }
    }
    
    getTileIndicesForOcean(positions, bufferTiles = 0) {
        const tiles = new Set();
        
        for (let i = 0; i < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];
            
            // Ocean uses min_lat = -80°, max_lat = 85.05°
            const centerLonIdx = Math.floor((lon + 180) / 10);
            const centerLatIdx = Math.floor((lat + 80) / 10);
            
            for (let dx = -bufferTiles; dx <= bufferTiles; dx++) {
                for (let dy = -bufferTiles; dy <= bufferTiles; dy++) {
                    const lonIdx = centerLonIdx + dx;
                    const latIdx = centerLatIdx + dy;
                    // 36 longitude tiles, 17 latitude tiles (-80° to 85°)
                    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 17) {
                        tiles.add({ lonIdx, latIdx });
                    }
                }
            }
        }
        
        return Array.from(tiles);
    }
    
    // ========== FUTURE PRELOADING ==========
    
    preloadFutureSteps(currentDate, currentPositions, stepsAhead = 3, bufferTiles = 1) {
        for (let step = 1; step <= stepsAhead; step++) {
            const futureDate = this.addDays(currentDate, step);
            const futurePositions = this.predictPositions(currentPositions, step);
            
            // Preload ocean tiles for future positions
            const futureOceanTiles = this.getTileIndicesForOcean(futurePositions, bufferTiles);
            this.preloadTiles(futureDate, futureOceanTiles);
            
            // Preload landmask tiles for future positions
            const futureLandmaskTiles = this.getTileIndicesForLandmask(futurePositions, bufferTiles);
            this.preloadLandmaskTiles(futureLandmaskTiles);
        }
    }
    
    predictPositions(positions, stepsAhead) {
        // Simple linear extrapolation based on current velocity
        // For now, just return current positions (conservative — preloads nearby tiles)
        return positions;
    }
    
    addDays(dateInt, days) {
        const year = Math.floor(dateInt / 10000);
        const month = Math.floor((dateInt % 10000) / 100);
        const day = dateInt % 100;
        const date = new Date(year, month - 1, day);
        date.setDate(date.getDate() + days);
        return date.getFullYear() * 10000 + (date.getMonth() + 1) * 100 + date.getDate();
    }
}

window.getPreloadedTile = function(url) {
    if (window.__tileCache && window.__tileCache.has(url)) {
        return window.__tileCache.get(url);
    }
    return null;
};

export const preloader = new TilePreloader();