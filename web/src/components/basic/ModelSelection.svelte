<script lang="ts">
    import OilSearch from "./OilSearch.svelte";

    let selectorType: 'oil-weathering' | 'plastic-drift' | 'search-and-rescue' = 'oil-weathering';
</script>

<div class="container-wrapper">
    <!-- model selector -->
    <div class="container-secondary" id="model-selector-container-basic">
        <span class="model-text">Model</span>
        <select bind:value={selectorType} name="model-selector-basic" class="selector-primary" id="model-selector-basic">
        <option value="oil-weathering">Oil Weathering</option>
        <option value="plastic-drift">Plastic Drift</option>
        <option value="search-and-rescue">Search & Rescue</option>
        </select>
    </div>
    <!-- oil selector -->
     {#if selectorType === 'oil-weathering'}
        <OilSearch />
    {:else if selectorType === 'plastic-drift'}
        <div class="container-secondary" id="plastic-selector-container-basic">
            <span>Plastic Type</span>
            <select name="plastic-selector" class="selector-primary" id="plastic-selector-basic">
                <option value="polyethylene" selected>Polyethylene (PE)</option>
                <option value="polypropylene">Polypropylene (PP)</option>
                <option value="polystyrene">Polystyrene (PS)</option>
                <option value="nylon">Nylon (PA)</option>
                <option value="polyester">Polyester (PET)</option>
                <option value="pvc">PVC</option>
                <option value="biodegradable">Biodegradable</option>
            </select>
        </div>
    {:else if selectorType === 'search-and-rescue'}
        <div class="container-secondary" id="object-selector-container-basic">
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
</div>