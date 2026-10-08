DriftMap2D is a client-side, multi-tracer Lagrangian particle tracking model that runs entirely in the browser via WebAssembly. It is primarily used to determine the fate and trajectory of simulated tracers (such as oil or a search and rescue object) in the ocean.

DriftMap2D performs all modelling calculations client-side. It achieves this by streaming processed and tiled hydrodynamical data from a CDN ahead of simulation time, and by using a rust engine compiled to WASM to interpolate, integrate, and apply tracer-specific physics, DriftMap2D can complete drift simulations in seconds with real-time simulation.

