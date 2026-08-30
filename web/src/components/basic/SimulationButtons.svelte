<script lang="ts">
    import logo from '$lib/assets/images/ResetBtn.webp';
    import { startSimulation, stopSimulation, resetSimulation, resumeSimulation } from '$lib/simulation';
    import { timeline } from '$lib/stores.svelte';

    let simulationState: "inactive" | "running" | "paused" | "playback" = $state("inactive");

    $effect(() => {
        if (timeline.playbackMode) {
            simulationState = "playback";
        }
    });

    async function start(e: Event) {
        const errors = await startSimulation();
        if (!errors || errors.length === 0) {
            simulationState = "running";
        }
    }

    function pause(e: Event) {
        stopSimulation();
        simulationState = "paused";
    }

    function resume(e: Event) {
        resumeSimulation();
        simulationState = "running";
    }

    function reset(e: Event) {
        resetSimulation();
        simulationState = "inactive";
    }
</script>

<div class="simulation-btn-container">
    <button
        class:active={simulationState === "inactive"}
        class="btn-primary"
        onclick={start}
        id="start-simulation"
    >
        Render Scenario
    </button>
    <button
        class:active={simulationState === "running"}
        class="btn-primary"
        onclick={pause}
        id="stop-simulation"
    >
        Pause Render
    </button>
    <button
        class:active={simulationState === "paused"}
        class="btn-primary"
        onclick={resume}
        id="resume-simulation"
    >
        Resume Render
    </button>
    <button
        class:active={timeline.playbackMode}
        class="btn-primary"
        id="export-geojson"
    >
        Export Scenario
    </button>
    <button
        type="button"
        onclick={reset}
        id="reset-simulation"
    >
        <img class="reset-btn-img" src={logo} alt="Reset Button" />
    </button>
</div>