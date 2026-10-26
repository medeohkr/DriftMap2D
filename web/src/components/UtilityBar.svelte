<script lang="ts">
    import { exportScenario } from "$lib/io/exportScenario";
import { importScenario } from "$lib/io/importScenario";
    import { config, simulationTimeState, releaseConfig, visualization, genericOverrides, objectOverrides, oilOverrides, simulation } from "$lib/stores/index.svelte";
    import { dateOffset, resetReleaseColors, DEFAULT_SIMULATION_DAYS } from "$lib/utils";
    import { showToast } from "$lib/stores/toast.svelte";
    let fileInput = $state<HTMLInputElement | undefined>(undefined);

    function openFilePicker() {
        if (simulation.simulationActive) {
            showToast("Cannot import config/scenario when simulation active");
            return;
        }
        fileInput?.click();
    }
    
    async function handleFileChange(e: Event) {
        const input = e.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        if (file) {
            await importScenario(file);
        }
        input.value = "";
    }
    function resetConfig() {
        if (simulation.simulationActive) {
            showToast("Cannot clear settings when simulation active");
            return;
        }
        config.particleCount = 10000;
        config.timeStepMin = 15;
        config.advectionScheme = "euler";
        config.diffusionScheme = "constant";
        config.diffusionCoeffs = [10, 0.1];
        config.startDate = dateOffset(0);
        config.startTime = "00:00";
        config.endDate = dateOffset(DEFAULT_SIMULATION_DAYS);
        config.endTime = "00:00";
        config.totalDays = DEFAULT_SIMULATION_DAYS;
        simulationTimeState.version++;
        config.tracerType = "generic";
        config.forcingModel = "global";

        releaseConfig.releases = [
            {
                lat: 48.4,
                lon: -125.0,
                radius: 0.5,
                schedule: [
                    {
                        amount: 100,
                        duration: 6,
                    },
                ],
            },
        ],
        releaseConfig.activeReleaseIndex = 0;
        resetReleaseColors();

        visualization.particleRadius = 1.5;
        visualization.gridUpdateInterval = 100;
        visualization.gridSize = 0.02;
        visualization.smoothLevel = 2;
        visualization.autoZoom = true;

        genericOverrides.windFactor = 2.0;
        genericOverrides.windDeflection = 0.0;

        oilOverrides.query = "Generic Light Crude";
        oilOverrides.id = "GN00006";
        oilOverrides.windFactor = 3.5;
        oilOverrides.windDeflection = 20.0;
        oilOverrides.deflectionScheme = "samuels";
        oilOverrides.emulOnset = "oil";
        oilOverrides.bullwinkleFrac = 0;
        oilOverrides.bulltime = 0;
        
        objectOverrides.query = "Person in water, unknown state (mean values)";
        objectOverrides.id = "Person in water, unknown state (mean values)";
        objectOverrides.jibeProbability = 4;
        objectOverrides.capsizing = false;
        objectOverrides.capsizeThreshold = 30;
        objectOverrides.capsizeFraction = 40;
        objectOverrides.capsizeSigma = 5;
    }
</script>

<button class="btn-tertiary" onclick={openFilePicker}>Import...</button>
<input
    bind:this={fileInput}
    type="file"
    id="import-geojson-file"
    accept=".json,.geojson"
    onchange={handleFileChange}
/>
<button class="btn-tertiary" onclick={() => exportScenario()}>Save Scenario</button>
<button class="btn-tertiary" onclick={resetConfig}>Reset Settings</button>

<style>
.btn-tertiary {
    padding: var(--spacing-xxxs);
    font-family: var(--font-family);
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
    background-color: var(--bg-tertiary);
    border: none;
    border-radius: var(--border-md);
    cursor: pointer;
    transition: opacity var(--transition-fast)
}

.btn-tertiary:hover {
    opacity: 0.6;
}
#import-geojson-file {
    display: none;
}

</style>
