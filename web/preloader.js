// Tile preloader
export class TilePreloader {
  constructor() {
    this.cache = new Map(); // url -> Promise
    this.loaded = new Set(); // urls already loaded
    this.baseUrl = "data/forecast_tiles";
  }

  // Get URL for a tile
  getUrl(date, lonIdx, latIdx) {
    const year = Math.floor(date / 10000);
    const month = Math.floor((date % 10000) / 100);
    const day = date % 100;

    // Zero-pad month and day to 2 digits
    const monthStr = month.toString().padStart(2, "0");
    const dayStr = day.toString().padStart(2, "0");

    return `${this.baseUrl}/${year}/${monthStr}/${dayStr}/${lonIdx.toString().padStart(3, "0")}_${latIdx.toString().padStart(3, "0")}.bin`;
  }

  // Preload a single tile (doesn't block)
  preloadTile(date, lonIdx, latIdx) {
    const url = this.getUrl(date, lonIdx, latIdx);

    // Already loaded or loading
    if (this.loaded.has(url)) return;
    if (this.cache.has(url)) return;

    // Start loading
    const promise = fetch(url)
      .then((response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        return response.arrayBuffer();
      })
      .then((data) => {
        this.loaded.add(url);
        return data;
      })
      .catch((err) => {
        console.warn(`Preload failed: ${url}`, err);
        this.cache.delete(url);
        this.loaded.delete(url);
      });

    this.cache.set(url, promise);
  }

  // Preload multiple tiles
  preloadTiles(date, tileIndices) {
    for (const { lonIdx, latIdx } of tileIndices) {
      this.preloadTile(date, lonIdx, latIdx);
    }
  }
}
export const preloader = new TilePreloader();
