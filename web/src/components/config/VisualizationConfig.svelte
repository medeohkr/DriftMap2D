<script lang="ts">
    import { config } from "$lib/stores/index.svelte";
    import { visualization } from "$lib/stores/visualization.svelte";
    import { numberInput } from "$lib/utils";
</script>

<summary style="cursor: pointer; user-select: none;">Visualization Settings</summary>
<div class="box-container">
    <div class="container-secondary">
        <span>Auto-Zoom</span>
        <select bind:value={visualization.autoZoom} class="scheme-selector">
            <option value={true}>True</option>
            <option value={false}>False</option>
        </select>
    </div>
    <div class="container-secondary">
        <span>Particle Radius</span>
        <input
            type="number"
            class="field-primary"
            bind:value={visualization.particleRadius}
            step="any"
            min="0"
            use:numberInput={{ min: 0, max: 1e3 }}
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
            use:numberInput={{ min: 0, max: 1e3 }}
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
            use:numberInput={{ min: 0, max: 1e3 }}
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
            use:numberInput={{ min: 1, max: 1e3 }}
        />
    </div>
    <div class="container-secondary">
        <span>Steps Per Snapshot</span>
        <input
            type="number"
            class="field-primary"
            bind:value={visualization.snapshotInterval}
            step="any"
            min="1"
            use:numberInput={{ min: 1, max: 1e3 }}
        />
    </div>
</div>