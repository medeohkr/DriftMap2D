<script lang="ts">
    import { config } from '$lib/stores.svelte'
    import { normalizeLongitude, updateMarker} from '$lib/map';

    function onLonInput(e: Event) {
        const value = parseFloat((e.target as HTMLInputElement).value);
        if (!isNaN(value)) {
            config.lon = value;
        }
        updateMarker();
    }

</script>

<div class="container-wrapper">
    <span>Release Settings</span>
    <div class="container-secondary">
        <span class="release-text">Latitude</span>
        <input onblur={updateMarker} bind:value={config.lat} type="text" class="field-primary" id="lat-field">
        <span class="unit-text">° N</span>
    </div>
    <div class="container-secondary">
        <span class="release-text">Longitude</span>
        <input onblur={onLonInput} type="text" class="field-primary" id="lon-field" value={normalizeLongitude(config.lon).toFixed(2)}>
        <span class="unit-text">° E</span>
    </div>
    <div class="container-secondary">
        <span>Total Mass</span>
        <input bind:value={config.releaseAmount} type="number" class="field-primary" id="release-amount-field" step="50">
        <span class="unit-text">tons</span>
    </div>
    <div class="container-secondary">
        <span>Duration</span>
        <input bind:value={config.releaseDuration} type="number" class="field-primary" id="release-duration-field" step="1.0">
        <span class="unit-text">days</span>
    </div>
    <div class="container-secondary">
        <span class="release-radius-text">Radius</span>
        <input bind:value={config.spreadKm} type="number" class="field-primary" id="release-radius-field" step="1.0">
        <span class="unit-text">km</span>
    </div>
</div>
<style>
    #lat-field, #lon-field {
        width: var(--width-number);
        margin-left: auto;
        margin-right: var(--spacing-xs);
    }

    #release-amount-field, #release-duration-field, #release-radius-field {
        width: var(--width-number);
        margin-left: auto;
        margin-right: var(--spacing-xs);
    }
</style>