<script lang="ts">
    import { config } from "$lib/stores/config.svelte";
</script>

<summary style="cursor: pointer">Advanced</summary>
<div class="box-container">
    <div class="container-secondary">
        <span>Particle Count</span>
        <input
            type="number"
            class="field-primary"
            bind:value={config.particleCount}
            step="any"
            min="100"
            max="50000"
        />
    </div>
    <div class="container-secondary">
        <span>Time Step (minutes)</span>
        <input
            type="number"
            class="field-primary"
            bind:value={config.timeStepMin}
            step="any"
            min="1"
        />
    </div>
    <div class="container-secondary">
        <span>Advection Scheme</span>
        <select bind:value={config.advectionScheme} class="selector-primary" id="scheme-selector">
            <option value="euler">Euler</option>
            <option value="rk2">RK2</option>
            <option value="rk4">RK4</option>
        </select>
    </div>
</div>

<style>
#scheme-selector {
    height: auto;
    width: var(--width-number);
    background-color: var(--bg-tertiary);
    margin-right: var(--spacing-xxs);
}

#scheme-selector:hover {
    opacity: 0.6;
}
</style>