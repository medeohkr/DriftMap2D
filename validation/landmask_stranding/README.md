# deterministic coastal stranding

compares Proteus generic and OpenDrift OceanDrift near 59 N, 5.1 E using NorKyst currents and wind from october 28 to 30, 2026.

both use Euler with a 15 minute timestep, 2 percent windage, zero deflection, zero release radius, and no diffusion, wave drift, or uncertainty. 9,600 particles are allocated over a 24 hour release. identical particles within each release batch follow the same trajectory, leaving release time as the source of shear.

OpenDrift receives the exact birth batches recorded by the CLI. the current CLI releases the first two batches at the initial time and finishes releasing at hour 23.5 for this timestep. this suite mirrors that existing discretization rather than changing the release engine. the CLI is run twice and particle outputs must match byte for byte.

the OpenDrift process uses the mean dilation from commit `e313d779` through a local function replacement. the installed package stays unchanged. source NetCDF is cached by the existing NorKyst leeway suite. remote tiles and cached NetCDF can still represent different forecast revisions, so this measures the complete coastal drift and stranding behavior rather than isolating landmask classification alone.

with the OpenDrift environment active, run these paths from the project root. the scripts also accept absolute paths when called from another directory.

```sh
python validation/landmask_stranding/run.py
python validation/landmask_stranding/visualize.py run_000
python validation/landmask_stranding/test_suite.py
python validation/landmask_stranding/diagnose.py run_000
python validation/landmask_stranding/matched_placement.py run_000
```

the runner builds the CLI, skips land release points, and tries nearby longitudes if either model strands every particle or none. use `--lon`, `--lat`, `--particles`, `--source`, `--tile-url`, or `--landmask-url` to override inputs. `--skip-build` uses the existing release binary.

each ignored `runs/run_NNN/` directory contains the config, CLI particle snapshots, OpenDrift NetCDF, hourly `stranding_counts.csv`, and `summary.json`. `particle_clouds.gif` and `particle_clouds_final.png` show all particles with cumulative stranded counts. black particles are stranded and colored particles remain afloat.

the initial run at 59 N, 5.1 E stranded 3,300 of 9,600 particles in each model after 48 hours. both first showed stranding at hour 14. hourly counts differed by at most 100 particles, one release batch, during hours 17 to 19. the two CLI runs produced identical particle output. repeated particles in each batch overlap visually because there is no stochastic spread.

`diagnose.py` pairs one representative from each birth batch and writes `shoreline_comparison.csv` and `shoreline_comparison.json`. it measures distance to the original GSHHG polygon boundaries in UTM zone 32N. batch statistics give equal weight to each batch rather than each duplicate particle.

the initial case has 32 stranded birth batches with no disagreement about which batches eventually strand. their last shared hourly water positions differ by a median 1.0 m and at most 3.8 m. final stranded positions differ by a median 76 m. Proteus positions are a median 1.9 m from the shoreline and at most 4.9 m away. OpenDrift positions are a median 39 m away and at most 155 m away.

the installed OpenDrift uses 0.001 degree coastline sampling and constructs a rectangular mesh of candidate points. Proteus samples along the movement segment and bisects to 0.0001 degrees. these methods can select different points even for nearly identical incoming trajectories.

one 100 particle batch released at hour 6 is a separate edge case. a 15 minute diagnostic replay shows OpenDrift stranding it at hour 16.5, while Proteus strands it at hour 19.75 about 1.93 km away. the proposed endpoints at hour 16.5 differ by only 0.65 m, but the same RoaringLandmask polygon query places the OpenDrift endpoint on land and the Proteus endpoint in water. this is sensitivity to a shoreline edge rather than evidence of an offset landmask.

`matched_placement.py` reruns only OpenDrift with Proteus's segment search and tolerance. it preserves OpenDrift's incoming coordinates and polygon classifier and restores the original function after the run. outputs go into `runs/run_NNN/matched_placement/`, leaving the original comparison intact. this is a diagnostic modification of OpenDrift, not a comparison against its standard placement algorithm.

in this controlled run, median stranded separation decreased from 76.1 m to 1.32 m and the 90th percentile decreased from 199 m to 4.00 m. every batch except the shoreline edge outlier finished within 4.09 m of its counterpart. the outlier remained about 1.90 km apart because changing final placement does not change its earlier land detection. both models still stranded 3,300 particles.
