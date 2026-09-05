DriftMap is a client-side, multi-tracer Lagrangian particle tracking model that runs entirely in the browser via WebAssembly. It is primarily used to determine the fate and trajectory of simulated substances and objects such as oil and plastic in marine environments.

Driftmap performs all advection/diffusion and tracer-specific behaviour modelling completely client-side. It achieves this by streaming processed and tiled hydrodynamical data from CDN ahead of simulation time, allowing for extremely fast lagrangian simulation with real-time visualization.

Features —— Engine:

- Data parsing of current u, v, wind, and sst with bilinear interpolation
- Tile Cache system with preloading of predicted future tiles
- SoA particle field storage
- Batched Runge-Kutta 4 integration
- Windage using Samuels wind deflection parameterization
- Smagorinsky spatially-varying diffusion scheme (adapted from OceanParcels)
- Oil physical property parsing from ADIOS oil JSONs
- Oil weathering module modelling evaporation and emulsification behaviours of oils (adapted from PyGnome and OpenDrift)
- Roaring bitmaps landmask for fast and memory-efficient particle stranding (inspired from Gauteh's Roaring Landmask library)
- Heatmap contour generator using marching squares at multiple thresholds

TODO —— Engine:

- Implement tracer-specific physics for plastic and SAR objects, currently only oil modeled
- Update ReleaseManager to support multi-release points and even polygonal release areas, each with unique release schedules
- Perform temporal interpolation on forcing data (especially important for 6hourly winds and sst)
- Add full support for 3D forcing with 3D physics such as vertical diffusion, allowing for 3D tracer behaviours 

Features —— Frontend:

- Interactive OpenStreetMap from Maplibre and CartoDB maptiles
- Basic and advanced panels with wide range of configs:
    - model type, tracer type,simulation startdate and totaldays,
    release location from click of map, release radius, release duration, release amount
    oil selector from range of 1460 oil assays from ADIOS Oil Database
- Particle and Heatmap toggles for switching between visualization modes
- Legend for interpreting heatmap threshold colours scaling with release amount
- Live stats display of tracer-specific statistics (percentage particles stranded, percentage of oil emulsified, etc)
- Snapshot capture system allowing for smooth playback from saved GeoJsons via interactive timeline
- Import/Export of said GeoJsons for the saving of specific simulation scenarios


TODO —— Frontend:

- Add advanced configs such as oil overrides, multi-releases and polygonal releases, different integration schemes, etc
- Figure out a good approach to dynamically scale sidebar based on viewport height
- Make mobile UI usable

DriftMap Engine is written in Rust compiled to WASM, frontend is Svelte/Typescript