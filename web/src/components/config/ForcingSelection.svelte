<script lang="ts">
    import { map } from "$lib/map";
    import { config } from "$lib/stores/index.svelte"

    function flyToNorKystCoverage() {
        const norkystBounds = [
            [-1.56, 55.8],
            [37.94, 75.2],
        ];

        map.fitBounds(norkystBounds, {
            padding: {
                bottom: 200,
                left: 300,
            },
            duration: 2000,
            essential: true,
        });
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
