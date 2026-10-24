<script lang="ts">
    import resetBtnImg from '$lib/assets/images/ResetBtn.webp';
    import { startSimulation, stopSimulation, resetSimulation, resumeSimulation } from '$lib/simulation';
    import { simulation, timeline, sidebarState } from '$lib/stores/index.svelte';

    let simulationState = $derived(
        timeline.playbackMode
            ? "playback"
            : simulation.simulationRunning
              ? "running"
              : simulation.simulationActive
                ? "paused"
                : "inactive"
    );
    let { toggleSidebar } = $props();

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
    onclick={() => startSimulation()}
>
    Render Simulation
</button>
<button
    class:active={simulationState === "running"}
    class="btn-primary"
    onclick={() => stopSimulation()}
>
    Pause Simulation
</button>
<button
    class:active={simulationState === "paused"}
    class="btn-primary"
    onclick={() => resumeSimulation()}
>
    Resume Simulation
</button>
<button
    class:active={timeline.playbackMode}
    class="btn-primary"
    onclick={() => resetSimulation()}
>
    New Simulation
</button>
<button
    class="btn-secondary"
    type="button"
    onclick={() => resetSimulation()}
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