<script lang="ts">
    import { visualization } from "$lib/stores.svelte";
</script>

<span id="coordinate-display"
    >{visualization.mapLat}° N, {visualization.mapLon}° E</span
>

<style>
    #coordinate-display {
        position: absolute;
        bottom: 18px;
        right: 45px;
        color: var(--text-secondary);
        font-family: monospace;
        font-size: var(--font-size-xs);
        pointer-events: none;
    }
</style>
