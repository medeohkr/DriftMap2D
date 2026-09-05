<script lang="ts">
    import resetBtnImg from '$lib/assets/images/ResetBtn.webp';
    import { startSimulation, stopSimulation, resetSimulation, resumeSimulation } from '$lib/simulation';
    import { timeline, sidebarState } from '$lib/stores.svelte';

    let simulationState: "inactive" | "running" | "paused" | "playback" = $state("inactive");
    let { toggleSidebar } = $props();

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

<button
    class="btn-secondary"
    onclick={toggleSidebar}
>
    {sidebarState.collapseStage === 2 ? '▼' : '▲'}
</button>
<button
    class:active={simulationState === "inactive"}
    class="btn-primary"
    onclick={start}
>
    Render Simulation
</button>
<button
    class:active={simulationState === "running"}
    class="btn-primary"
    onclick={pause}
>
    Pause Simulation
</button>
<button
    class:active={simulationState === "paused"}
    class="btn-primary"
    onclick={resume}
>
    Resume Simulation
</button>
<button
    class:active={timeline.playbackMode}
    class="btn-primary"
    id="export-geojson"
>
    Export GeoJson
</button>
<button
    class="btn-secondary"
    type="button"
    onclick={reset}
>
    <img class="reset-btn-img" src={resetBtnImg} alt="Reset Button" />
</button>



<style>

.btn-primary {
    display: none;
    flex: 1;
    height: var(--height-action-btn);
    padding: var(--spacing-xs);
    font-family: var(--font-family);
    font-size: var(--font-size-sm);
    font-weight: var(--weight-primary);
    color: var(--text-secondary);
    background-color: var(--bg-secondary);
    border: none;
    border-radius: var(--border-lg);
    cursor: pointer;
    transition: opacity var(--transition-fast);

    align-items: center;
    justify-content: center;
    white-space: nowrap;
}

.btn-primary.active {
    display: flex;
}

.btn-primary:hover {
    opacity: .8;
}

.btn-secondary {
    display: flex;
    width: var(--height-action-btn);
    height: var(--height-action-btn);
    padding: var(--spacing-xs);
    font-size: var(--font-size-lg);
    color: var(--text-secondary);
    background-color: var(--bg-secondary);
    border: none;
    border-radius: var(--border-lg);
    cursor: pointer;

    align-items: center;
    justify-content: center;
}

.btn-secondary:hover {
    opacity: .8;
}

.reset-btn-img {
    width: 100%;
    opacity: .6;
}
</style>