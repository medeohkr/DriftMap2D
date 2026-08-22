import { state } from './stores'
import { dom } from './ui';
import { map, updatePositionFromFields, zoom } from './map';
import { updateSimulationDate, updateTotalDays, updateReleaseAmount, updateReleaseDuration, updateReleaseRadius } from './simulation';
import { updateConcentrationLayer, updateLegend } from './visualization';
import { showTimeline } from './timeline';

export interface GeoJsonConfig {
  release_lon: number;
  release_lat: number;
  release_amount_tons: number;
  release_duration_days: number;
  release_radius_km: number;
  start_date: string;
  total_days: number;
  particle_count: number;
  cs_value: number;
}

export interface GeoJsonProperties {
  model: string;
  version: string;
  date: string;
  includes_heatmaps: boolean;
  config: GeoJsonConfig;
}

export interface GeoJsonData {
  type: "FeatureCollection";
  properties: GeoJsonProperties;
  features: any[];
}
// === IMPORT/EXPORT ===

export function exportScenario() {
  const exportData: GeoJsonData = {
    type: "FeatureCollection",
    properties: {
      model: "DriftMap",
      version: "1.0",
      date: new Date().toISOString(),
      includes_heatmaps: true,
      config: {
        release_lon: state.rawLon,
        release_lat: state.rawLat,
        release_amount_tons: state.releaseAmount,
        release_duration_days: state.releaseDuration,
        release_radius_km: state.spreadKm,
        start_date: `${state.startYear}-${String(state.startMonth).padStart(2, "0")}-${String(state.startDay).padStart(2, "0")}`,
        total_days: state.totalDays,
        particle_count: state.particleCount,
        cs_value: state.csValue,
      },
    },
    features: state.simulationHistory.map((snapshot) => ({
      type: "Feature",
      properties: {
        day: snapshot.day,
        date: snapshot.dateStr,
        stats: snapshot.stats,
        unstranded_particles: snapshot.unstrandedGeojson.features.length,
        stranded_particles: snapshot.strandedGeojson.features.length,
      },
      geometry: {
        type: "GeometryCollection",
        geometries: [
          {
            type: "MultiPoint",
            coordinates: snapshot.unstrandedGeojson.features.map(
              (f: any) => f.geometry.coordinates,
            ),
          },
          {
            type: "MultiPoint",
            coordinates: snapshot.strandedGeojson.features.map(
              (f: any) => f.geometry.coordinates,
            ),
          },
          snapshot.heatmapGeojson?.features
            ? {
                type: "FeatureCollection",
                features: snapshot.heatmapGeojson.features,
              }
            : null,
        ].filter(Boolean),
      },
    })),
  };

  const blob = new Blob([JSON.stringify(exportData)], {
    type: "application/json",
  });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `driftmap-full-${state.startYear}-${state.startMonth}-${state.startDay}.geojson`;
  a.click();
  URL.revokeObjectURL(a.href);
}

export function loadGeoJsonResults(data: GeoJsonData) {
  if (!data.features || data.features.length === 0) {
    alert("No simulation data found in file");
    return;
  }

  state.simulationRunning = false;
  if (state.animationId) {
    cancelAnimationFrame(state.animationId);
    state.animationId = null;
  }

  const hasHeatmaps = data.properties.includes_heatmaps;

  state.simulationHistory = data.features.map((feature: any) => {
    const geometries = feature.geometry.geometries;

    const snapshot = {
      day: feature.properties.day,
      dateStr: feature.properties.date,
      stats: feature.properties.stats,
      unstrandedGeojson: {
        type: "FeatureCollection",
        features: geometries[0].coordinates.map((coord) => ({
          type: "Feature",
          geometry: { type: "Point", coordinates: coord },
        })),
      },
      strandedGeojson: {
        type: "FeatureCollection",
        features: geometries[1].coordinates.map((coord) => ({
          type: "Feature",
          geometry: { type: "Point", coordinates: coord },
        })),
      },
      heatmapGeojson: null,
    };

    if (hasHeatmaps && geometries.length > 2 && geometries[2]) {
      const heatmapFeatures =
        geometries[2].features || geometries[2].geometries || [];
      snapshot.heatmapGeojson = {
        type: "FeatureCollection",
        features: heatmapFeatures.map((f) => ({
          type: "Feature",
          geometry: f.geometry,
          properties: {
            concentration: f.properties?.concentration || 1,
          },
        })),
      };
    }

    return snapshot;
  });

  dom.startBtn.style.display = "none";
  dom.stopBtn.style.display = "none";
  dom.resumeBtn.style.display = "none";
  dom.exportGeojsonBtn.style.display = "inline-block";
  dom.latField.value = `${data.properties.config.release_lat}`;
  dom.lonField.value = `${data.properties.config.release_lon}`;
  dom.releaseAmountField.value = `${data.properties.config.release_amount_tons}`;
  dom.releaseDurationField.value = `${data.properties.config.release_duration_days}`;
  dom.releaseRadiusField.value = `${data.properties.config.release_radius_km}`;
  dom.startDate.value = `${data.properties.config.start_date}`;
  dom.totalDaysField.value = `${data.properties.config.total_days}`;
  dom.statsDisplay.style.display = "flex";

  updatePositionFromFields();
  updateSimulationDate();
  updateTotalDays();
  updateReleaseAmount();
  updateReleaseDuration();
  updateReleaseRadius();
  showTimeline();
  updateConcentrationLayer();
  updateLegend();
  zoom();

  map.setPaintProperty("overlay-layer", "raster-opacity", 0.05);

  state.playbackMode = true;

  if (state.currentMarker) {
    state.currentMarker.remove();
  }
}