<script lang="ts">
    import { config } from "$lib/stores.svelte"

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
        <div class="container-secondary" id="oil-selector-container-basic">
            <span class="oil-text">Oil Type</span>
            <select bind:value={config.oilName} name="oil-selector-basic" class="selector-primary" id="oil-selector-basic">
            <option value="marine-diesel">Marine Diesel</option>
            <option value="bonny-light">Bonny Light</option>
            <option value="arabian-light" selected>Arabian Light</option>
            <option value="venezuelan-heavy">Venezuelan Heavy</option>
            <option value="ifo-380">IFO 380</option>
            </select>
        </div>
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