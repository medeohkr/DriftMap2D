<script lang="ts">
    import { releaseConfig } from "$lib/stores.svelte";
    import { normalizeLongitude, updateMarker } from "$lib/map";

    function onLonInput(e: Event) {
        const value = parseFloat((e.target as HTMLInputElement).value);
        if (!isNaN(value)) {
            releaseConfig.activeRelease.lon = value;
        }
        updateMarker();
    }

    function updateReleaseLat(e: Event) {
        const value = parseFloat((e.target as HTMLInputElement).value);
        if (!isNaN(value)) {
            releaseConfig.activeRelease.lat = value;
        }
        updateMarker();
    }
</script>

<div class="floating-container">
    <span>
        Release Settings
        {#if releaseConfig.releases.length > 1}
            &nbsp; — &nbsp;Editing Release {releaseConfig.activeReleaseIndex}/{releaseConfig
                .releases.length}
        {/if}
    </span>
    <div class="box-container">
        <div class="container-secondary">
            <span class="release-text">Latitude</span>
            <input
                onblur={updateReleaseLat}
                type="number"
                class="field-primary"
                id="lat-field"
                value={releaseConfig.activeRelease.lat.toFixed(2)}
            />
            <span class="unit-text">° N</span>
        </div>
        <div class="container-secondary">
            <span class="release-text">Longitude</span>
            <input
                onblur={onLonInput}
                type="number"
                class="field-primary"
                id="lon-field"
                value={normalizeLongitude(
                    releaseConfig.activeRelease.lon,
                ).toFixed(2)}
            />
            <span class="unit-text">° E</span>
        </div>
        <div class="container-secondary">
            <span class="release-radius-text">Radius</span>
            <input
                bind:value={releaseConfig.activeRelease.radius}
                type="number"
                class="field-primary"
                id="release-radius-field"
                step="1.0"
            />
            <span class="unit-text">km</span>
        </div>
    </div>
</div>
<div class="floating-container">
    <span>Schedule</span>
    <div class="box-container">
        <div class="container-secondary">
            <span>Total Mass</span>
            <input
                bind:value={releaseConfig.activeRelease.amount}
                type="number"
                class="field-primary"
                id="release-amount-field"
                step="50"
            />
            <span class="unit-text">tons</span>
        </div>
        <div class="container-secondary">
            <span>Duration</span>
            <input
                bind:value={releaseConfig.activeRelease.duration}
                type="number"
                class="field-primary"
                id="release-duration-field"
                step="1.0"
            />
            <span class="unit-text">days</span>
        </div>
    </div>
    <button onclick={releaseConfig.addInterval}> + &nbsp;Add Interval </button>
</div>

<style>
    #lat-field,
    #lon-field {
        width: var(--width-number);
        margin-left: auto;
        margin-right: var(--spacing-xs);
    }

    #release-amount-field,
    #release-duration-field,
    #release-radius-field {
        width: var(--width-number);
        margin-left: auto;
        margin-right: var(--spacing-xs);
    }
</style>
