import { state } from "./state";

export function normalizeLongitude(lon) {
  lon = parseFloat(lon);
  return ((((lon + 180) % 360) + 360) % 360) - 180;
}

export function getTileIndices(positions, minLat = -80) {
  const tiles = new Set();
  for (let i = 0; i < positions.length; i += 2) {
    const lon = positions[i];
    const lat = positions[i + 1];
    const lonIdx = Math.floor((lon + 180) / 10);
    const latIdx = Math.floor((lat - minLat) / 10);
    if (lonIdx >= 0 && lonIdx < 36 && latIdx >= 0 && latIdx < 34) {
      tiles.add({ lonIdx, latIdx });
    }
  }
  return Array.from(tiles);
}

export function getShiftedBounds(positions) {
  let lonMin = state.boundingBox[0];
  let lonMax = state.boundingBox[1];

  if (lonMax - lonMin > 180) {
    // Find actual particle extent in 0-360 space
    let shiftedMin = Infinity;
    let shiftedMax = -Infinity;
    
    for (let i = 0; i < positions.length; i += 3) {
      let lon = positions[i];
      if (lon < 0) lon += 360;
      if (lon < shiftedMin) shiftedMin = lon;
      if (lon > shiftedMax) shiftedMax = lon;
    }
    
    return {
      lonMin: shiftedMin,
      lonMax: shiftedMax,
      needsShift: true,
    };
  }

  return {
    lonMin: state.boundingBox[0],
    lonMax: state.boundingBox[1],
    needsShift: false,
  };
}

export function updateBoundingBox() {
  state.boundingBox = state.proteus.get_particle_bounding_box();
}

export function get<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) { throw new Error(`element #${id} not found`) }
  return el as T
}