<script lang="ts">
    import { config } from "$lib/stores.svelte"
    import OilSearch from "./OilSearch.svelte";

    let selectorType: 'generic-drift' | 'oil-weathering' | 'search-and-rescue' = 'generic-drift';
</script>


<div class="floating-container" id="model-selector-container-basic">
    <span>Select a Model</span>
    <select bind:value={selectorType} name="model-selector-basic" class="selector-primary" id="model-selector-basic">
    <option value="generic-drift" onclick={config.tracerType = "generic"}>Generic Drift</option>
    <option value="oil-weathering" onclick={config.tracerType = "oil"}>Oil Weathering</option>
    <option value="search-and-rescue" onclick={config.tracerType = "generic"}>Search & Rescue</option>
    </select>
</div>

{#if selectorType === 'oil-weathering'}
    <OilSearch />

{:else if selectorType === 'search-and-rescue'}
    <div class="floating-container" id="object-selector-container-basic">
        <span class="object-text">Object Type</span>
        <select name="object-selector-basic" class="selector-primary" id="object-selector-basic">
        <option value="person-in-water">Person In Water</option>
        <option value="skiff-modified-v">Skiff Modified-V</option>
        <option value="life-raft" selected>Life Raft</option>
        <option value="fishing-vessel">Fishing Vessel</option>
        <option value="sailboat">Sailboat</option>
        <option value="bait-wharf-box">Bait/Wharf Box</option>
        </select>
    </div>
{/if}
