<script lang="ts">
    let selectorType: 'generic-drift' | 'oil-weathering' | 'plastic-drift' | 'search-and-rescue' = 'generic-drift';
</script>
<div class="container-primary">
    <div class="container-wrapper">
        <div class="container-secondary" id="model-selector-container">
            <span>Model</span>
            <select bind:value={selectorType} name="model-selector" class="selector-primary" id="model-selector">
                <option value="generic-drift" selected>Generic Drift</option>
                <option value="oil-weathering">Oil Weathering</option>
                <option value="plastic-drift">Plastic Drift</option>
                <option value="search-and-rescue">Search & Rescue</option>
            </select>
        </div>
        {#if selectorType === 'oil-weathering'}
            <div class="container-secondary" id="oil-selector-container">
                <span>Oil Type</span>
                <select name="oil-selector" class="selector-primary" id="oil-selector">
                <option value="marine-diesel">Marine Diesel</option>
                <option value="bonny-light">Bonny Light</option>
                <option value="arabian-light" selected>Arabian Light</option>
                <option value="venezuelan-heavy">Venezuelan Heavy</option>
                <option value="ifo-380">IFO 380</option>
                <option value="custom">Custom</option>
                </select>
            </div>
        {:else if selectorType === 'plastic-drift'}
            <div class="container-secondary" id="plastic-selector-container">
                <span>Plastic Type</span>
                <select name="plastic-selector" class="selector-primary" id="plastic-selector">
                <option value="polyethylene" selected>Polyethylene (PE)</option>
                <option value="polypropylene">Polypropylene (PP)</option>
                <option value="polystyrene">Polystyrene (PS)</option>
                <option value="nylon">Nylon (PA)</option>
                <option value="polyester">Polyester (PET)</option>
                <option value="pvc">PVC</option>
                <option value="biodegradable">Biodegradable</option>
                <option value="custom">Custom</option>
                </select>
            </div>
        {:else if selectorType === 'search-and-rescue'}
            <div class="container-secondary" id="object-selector-container">
                <span>Object Type</span>
                <select name="object-selector" class="selector-primary" id="object-selector">
                <option value="person-in-water">Person In Water</option>
                <option value="skiff-modified-v">Skiff Modified-V</option>
                <option value="life-raft" selected>Life Raft</option>
                <option value="fishing-vessel">Fishing Vessel</option>
                <option value="sailboat">Sailboat</option>
                <option value="bait-wharf-box">Bait/Wharf Box</option>
                <option value="custom">Custom</option>
                </select>
            </div>
        {/if}
        <div class="container-secondary hidden" id="oil-selector-container">
            <span>Oil Type</span>
            <select name="oil-selector" class="selector-primary" id="oil-selector">
            <option value="marine-diesel">Marine Diesel</option>
            <option value="bonny-light">Bonny Light</option>
            <option value="arabian-light" selected>Arabian Light</option>
            <option value="venezuelan-heavy">Venezuelan Heavy</option>
            <option value="ifo-380">IFO 380</option>
            <option value="custom">Custom</option>
            </select>
        </div>
        <div class="container-secondary hidden" id="object-selector-container">
            <span>Object Type</span>
            <select name="object-selector" class="selector-primary" id="object-selector">
            <option value="person-in-water">Person In Water</option>
            <option value="skiff-modified-v">Skiff Modified-V</option>
            <option value="life-raft" selected>Life Raft</option>
            <option value="fishing-vessel">Fishing Vessel</option>
            <option value="sailboat">Sailboat</option>
            <option value="bait-wharf-box">Bait/Wharf Box</option>
            <option value="custom">Custom</option>
            </select>
        </div>
        <div class="container-secondary hidden" id="plastic-selector-container">
            <span>Plastic Type</span>
            <select name="plastic-selector" class="selector-primary" id="plastic-selector">
            <option value="polyethylene" selected>Polyethylene (PE)</option>
            <option value="polypropylene">Polypropylene (PP)</option>
            <option value="polystyrene">Polystyrene (PS)</option>
            <option value="nylon">Nylon (PA)</option>
            <option value="polyester">Polyester (PET)</option>
            <option value="pvc">PVC</option>
            <option value="biodegradable">Biodegradable</option>
            <option value="custom">Custom</option>
            </select>
        </div>
    </div>
</div>