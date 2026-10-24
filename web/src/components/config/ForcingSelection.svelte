<script lang="ts">
    import { map } from "$lib/map";
    import { config, simulation } from "$lib/stores/index.svelte"
    import { loadNorkystCoverage, updateDefaultReleaseLocation } from "$lib/utils";
    import { showToast } from "$lib/stores/toast.svelte";
    import { untrack } from "svelte";

    $effect(() => {
        const model = config.forcingModel;
        untrack(() => {
            if (!simulation.simulationActive) updateDefaultReleaseLocation(model);
        });
    });

    async function flyToNorKystCoverage() {
        try {
            const coverage = await loadNorkystCoverage();
            if (config.forcingModel !== "norkyst") return;
            map.fitBounds(coverage.bounds, {
                padding: {
                    bottom: 100,
                    top: 100,
                    left: 200,
                },
                duration: 2000,
                essential: true,
            });
        } catch (error) {
            showToast(String(error));
        }
    }
</script>


<div class="floating-container" id="model-selector-container-basic">
    <div class="container-secondary">
        <span>Forcing Model</span>
        {#if config.forcingModel == "norkyst"}
            <button class="zoom-btn" onclick={flyToNorKystCoverage}>
                ⌕ &nbsp;Zoom to Coverage
            </button>
        {/if}
    </div>
    <select bind:value={config.forcingModel} name="model-selector-basic" class="selector-primary" id="model-selector-basic">
        <option value="global">Global (CMEMS 1/12°)</option>
        <option value="norkyst">Norway (NorKyst 800m)</option>
    </select>
</div>

<style>
.zoom-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    font-family: var(--font-family);
    font-weight: var(--weight-secondary);
    font-size: var(--font-size-xs);
    cursor: pointer;
    transition: opacity var(--transition-fast);
}

.zoom-btn:hover {
    opacity: 0.6;
}
</style>
