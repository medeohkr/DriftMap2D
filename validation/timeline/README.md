# timeline slider regression

run `node validation/timeline/run.mjs` after installing the web dependencies.
the test uses headless Chrome at its standard Windows installation path.
set `CHROME_PATH` to use another installation.

the actual timeline component runs with synthetic snapshots and a stub map.
the test checks that the slider, date label, selected index, and rendered
snapshot agree after repeated simulations, shorter and longer histories,
rewinding, and scrubbing. it does not download forcing or run the physics engine.
