<script lang="ts">
    import { config } from "$lib/stores.svelte";
</script>

<div class="import-autozoom-container">
    <button class="btn-tertiary">Import Scenario</button>
    <input type="file" id="import-geojson-file" accept=".json,.geojson" />
    <button class="btn-tertiary">Save Config</button>
    <button class="btn-tertiary">Reset Config</button>

</div>

<style>
.import-autozoom-container {
    display: flex;
    width: var(--width-sidebar);
    font-family: var(--font-family);
    color: var(--text-secondary);
    background-color: var(--bg-tertiary);
    padding: 0px var(--spacing-md);
    justify-content: space-between;
    align-items: center;
}

.btn-tertiary {
    padding: var(--spacing-xxs);
    font-family: var(--font-family);
    font-size: var(--font-size-xs);
    color: var(--text-secondary);
    background-color: var(--bg-tertiary);
    border: none;
    border-radius: var(--border-md);
    cursor: pointer;
    transition: opacity var(--transition-fast)
}

.btn-tertiary:hover {
    opacity: 0.6;
}
#import-geojson-file {
    display: none;
}

</style>
