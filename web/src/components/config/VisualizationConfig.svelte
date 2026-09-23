<script lang="ts">
    import { visualization } from "$lib/stores/visualization.svelte";
</script>

<summary style="cursor: pointer; user-select: none;">Visualization Settings</summary>
<div class="box-container">
    <div class="container-secondary">
        <span>Particle Radius</span>
        <input
            type="number"
            class="field-primary"
            bind:value={visualization.particleRadius}
            step="any"
            min="0"
        />
    </div>
    <div class="container-secondary">
        <span>Heatmap Grid Size (°)</span>
        <input
            type="number"
            class="field-primary"
            bind:value={visualization.gridSize}
            step="any"
            min="0"
        />
    </div>
    <div class="container-secondary">
        <span>Smoothing Level</span>
        <input
            type="number"
            class="field-primary"
            bind:value={visualization.smoothLevel}
            step="any"
            min="0"
        />
    </div>
    <div class="container-secondary">
        <span>Render Delay (ms)</span>
        <input
            type="number"
            class="field-primary"
            bind:value={visualization.gridUpdateInterval}
            step="any"
            min="1"
        />
    </div>
</div>