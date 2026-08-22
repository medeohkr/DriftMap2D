<script lang="ts">

</script>

<div class="import-autozoom-container">
    <button id="import-geojson">Import Scenario</button>
    <input type="file" id="import-geojson-file" accept=".json,.geojson">
    <input type="checkbox" id="autozoom-checkbox" checked>
    <span class="autozoom-text">Auto-zoom</span>
</div>

<style>
    .import-autozoom-container {
        display: flex;
        align-items: center;
        width: var(--width-container-margin);
        color: var(--text-secondary);
        font-family: var(--font-family);
    }

    #import-geojson {
        padding: var(--spacing-xxs);
        background-color: var(--bg-secondary);
        color: var(--text-primary);
        border: var(--border-sm) solid var(--import-border);
        border-radius: var(--border-lg);
        font-family: var(--font-family);
        font-size: var(--font-size-xs);
        cursor: pointer;
    }

    #import-geojson-file {
        display: none;
    }

    #autozoom-checkbox {
        margin-left: var(--spacing-sm);
        margin-right: var(--spacing-xs);
    }
</style>