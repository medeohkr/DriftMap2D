<script lang="ts">
    import { config } from "$lib/stores/index.svelte"
    import ObjectSearch from "./ObjectSearch.svelte";
    import OilSearch from "./OilSearch.svelte";
</script>


<div class="floating-container" id="model-selector-container-basic">
    <span>Select a Model</span>
    <select bind:value={config.tracerType} name="model-selector-basic" class="selector-primary" id="model-selector-basic">
    <option value="generic" onclick={() => config.tracerType = "generic"}>Generic Drift</option>
    <option value="oil" onclick={() => config.tracerType = "oil"}>Oil Weathering</option>
    <option value="sar" onclick={() => config.tracerType = "sar"}>Search & Rescue</option>
    </select>
</div>

{#if config.tracerType === 'oil'}
    <OilSearch />

{:else if config.tracerType === 'sar'}
    <ObjectSearch />
{/if}
