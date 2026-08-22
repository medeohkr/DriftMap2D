<script lang="ts">
    import ImportAutoZoom from './ImportAutoZoom.svelte';
    import Presets from './Presets.svelte';
    import ModelSelection from './ModelSelection.svelte';
    import SimulationTime from './SimulationTime.svelte';
    import ReleaseSettings from './ReleaseSettings.svelte';
</script>

<div class="sidebar-content">
    <div class="container-primary">
        <ImportAutoZoom />
        <Presets />
        <ModelSelection />
        <SimulationTime />
        <ReleaseSettings />
        <!-- simulation buttons container -->
        <div class="simulation-btn-container">
            <button id="start-simulation" class="btn-primary">Render Scenario</button>
            <button id="stop-simulation" class="btn-primary">Pause Render</button>
            <button id="resume-simulation" class="btn-primary">Resume Render</button>
            <button id="export-geojson" class="btn-primary">Export Scenario</button>
            <button id="reset-simulation">
                <img class="reset-btn-img" src="src/assets/images/ResetBtn.webp" alt="Reset Button">
            </button>
        </div>
    </div>
</div>