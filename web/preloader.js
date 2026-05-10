// preloader.js
export class TilePreloader {
    constructor() {
        this.pending = new Map();      // url → Promise (in-flight requests)
        this.completed = new Set();    // urls already loaded
        this.baseUrl = "https://tiles.driftmap2d.com/tiles";
    }
    
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
        
        // Already loaded or in-flight
        if (this.completed.has(url)) return;
        if (this.pending.has(url)) return;
        
        const promise = fetch(url)
            .then(response => {
                if (!response.ok) throw new Error(`HTTP ${response.status}`);
                return response.arrayBuffer();
            })
            .then(buffer => {
                const uint8 = new Uint8Array(buffer);
                // Store in global cache for WASM
                if (!window.__tileCache) {
                    window.__tileCache = new Map();
                }
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
    
    // Preload future steps — called after each simulation step
    preloadFutureSteps(currentDate, currentPositions, stepsAhead = 3, bufferTiles = 1) {
        for (let step = 1; step <= stepsAhead; step++) {
            const futureDate = this.addDays(currentDate, step);
            const futurePositions = this.predictPositions(currentPositions, step);
            const futureTiles = this.getTileIndices(futurePositions, bufferTiles);
            this.preloadTiles(futureDate, futureTiles);
        }
    }
    
    predictPositions(positions, stepsAhead) {
        // Simple linear extrapolation based on current velocity
        // For now, just return current positions (conservative — preloads nearby tiles)
        return positions;
    }
    
    getTileIndices(positions, bufferTiles = 0) {
        const tiles = new Set();
        
        for (let i = 0; i < positions.length; i += 2) {
            const lon = positions[i];
            const lat = positions[i + 1];
            
            const centerLonIdx = Math.floor((lon + 180) / 10);
            const centerLatIdx = Math.floor((lat + 80) / 10);
            
            for (let dx = -bufferTiles; dx <= bufferTiles; dx++) {
                for (let dy = -bufferTiles; dy <= bufferTiles; dy++) {
                    const lonIdx = centerLonIdx + dx;
                    const latIdx = centerLatIdx + dy;
                    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 34) {
                        tiles.add({ lonIdx, latIdx });
                    }
                }
            }
        }
        
        return Array.from(tiles);
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

// Expose for WASM
window.getPreloadedTile = function(url) {
    if (window.__tileCache && window.__tileCache.has(url)) {
        const data = window.__tileCache.get(url);
        window.__tileCache.delete(url);
        return data;
    }
    return null;
};

export const preloader = new TilePreloader();