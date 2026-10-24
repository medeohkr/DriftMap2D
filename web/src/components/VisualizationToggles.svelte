<script lang="ts">
    import { visualization, simulation } from "$lib/stores/index.svelte";
    import {
        toggleParticleMode,
        toggleHeatmapMode,
    } from "../lib/visualization";
</script>

<div class="visualization-toggle-container" class:simulation-active={simulation.simulationActive}>
    <button
        class="visualization-toggle"
        id="particle-toggle"
        class:active={visualization.visualizationMode === "particles"}
        onclick={toggleParticleMode}>Particles</button
    >
    <button
        class="visualization-toggle"
        id="heatmap-toggle"
        class:active={visualization.visualizationMode === "heatmap"}
        onclick={toggleHeatmapMode}>Heatmap</button
    >

</div>

<style>
    .visualization-toggle-container {
        position: absolute;
        display: flex;
        top: var(--spacing-md);
        left: 50%;
        transform: translateX(-50%);
        column-gap: var(--spacing-sm);
        font-family: var(--font-family);
        color: var(--text-primary);
        font-weight: var(--weight-secondary);
        font-size: var(--font-size-md);
        align-items: center;
        white-space: nowrap;
    }
    @media (max-width: 768px) {
        .visualization-toggle-container.simulation-active {
            display: none;
        }
        .visualization-toggle-container {
            left: auto;
            right: var(--spacing-sm);
            transform: none;
        }
    }
    .visualization-toggle {
        background: none;
        color: var(--text-primary);
        font-family: var(--font-family);
        font-size: var(--font-size-lg);
        padding: var(--spacing-xxxs) var(--spacing-xxs);
        border-radius: var(--border-md);
        border: var(--border-sm) solid var(--text-primary);
        cursor: pointer;
    }

    .visualization-toggle.active {
        background: white;
        color: var(--shadow-primary);
    }
</style>
