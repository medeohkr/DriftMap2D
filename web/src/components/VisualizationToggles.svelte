<script lang="ts">
    import { updateOverlay } from '../lib/visualization';
    let visualizationMode: 'particles' | 'heatmap' = 'particles';
</script>
<div class="visualization-toggle-container">
    <button class="visualization-toggle" id="particle-toggle" class:active={visualizationMode === 'particles'} on:click={() => visualizationMode = 'particles'}>Particles</button>
    <button class="visualization-toggle" id="heatmap-toggle" class:active={visualizationMode === 'heatmap'} on:click={() => visualizationMode = 'heatmap'}>Heatmap</button>
    <div class="overlay-container">
        <input type="checkbox" on:change={(e) => updateOverlay(e.currentTarget.checked)} id="overlay-checkbox">
        <span>Show Today's Currents</span>
    </div>
</div>

<style>
    .visualization-toggle-container {
        position: absolute;
        display: flex;
        top: var(--spacing-md);
        left: 55%;
        transform: translateX(-50%);
        column-gap: var(--spacing-sm);
        font-family: var(--font-family);
        color: var(--text-primary);
        font-weight: var(--weight-secondary);
        font-size: var(--font-size-md);
        align-items: center;
        white-space: nowrap;
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
        background: var(--text-primary); color: var(--shadow-primary);
    }
</style>