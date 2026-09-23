<script lang="ts">
    import { config } from "$lib/stores/config.svelte";

    $effect(() => {
        if (config.tracerType == "sar") {
            config.diffusionCoeffs = [0, 0]
        } else {
            config.diffusionCoeffs = [50, 0.1]
        }
    })
</script>

<summary style="cursor: pointer; user-select: none;">Advanced</summary>
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
        <select bind:value={config.advectionScheme} class="scheme-selector">
            <option value="euler">Euler</option>
            <option value="rk2">RK2</option>
            <option value="rk4">RK4</option>
        </select>
    </div>
    {#if config.tracerType != "sar"}
        <div class="container-secondary">
            <span>Diffusion Scheme</span>
            <select bind:value={config.diffusionScheme} class="scheme-selector">
                <option value="constant">Constant</option>
                <option value="smagorinsky">Smagorinsky</option>
            </select>
        </div>
        {#if config.diffusionScheme == "constant"}
        <div class="container-secondary">
            <span>Diffusivity (m²/s)</span>
            <input
                type="number"
                class="field-primary"
                bind:value={config.diffusionCoeffs[0]}
                step="any"
                min="0"
            />
        </div>
        {:else}
        <div class="container-secondary">
            <span>Tuning Constant</span>
            <input
                type="number"
                class="field-primary"
                bind:value={config.diffusionCoeffs[1]}
                step="any"
                min="0"
            />
        </div>
        {/if}
    {/if}
</div>