<div class="container-wrapper">
    <span>Release Settings</span>
    <div class="container-secondary">
        <span class="release-text">Latitude</span>
        <input type="text" class="field-primary" id="lat-field" value="26.60">
        <span class="unit-text">° N</span>
    </div>
    <div class="container-secondary">
        <span class="release-text">Longitude</span>
        <input type="text" class="field-primary" id="lon-field" value="56.50">
        <span class="unit-text">° E</span>
    </div>
    <div class="container-secondary">
        <span>Total Mass</span>
        <input type="number" class="field-primary" id="release-amount-field" value="1000.0" step="10">
        <span class="unit-text">tons</span>
    </div>
    <div class="container-secondary">
        <span>Duration</span>
        <input type="number" class="field-primary" id="release-duration-field" value="1.0" step="1.0">
        <span class="unit-text">days</span>
    </div>
    <div class="container-secondary">
        <span class="release-radius-text">Radius</span>
        <input type="number" class="field-primary" id="release-radius-field" value="1.0" step="1.0">
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