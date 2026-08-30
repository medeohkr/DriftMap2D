<script lang="ts">
    import { config } from '$lib/stores.svelte';

    const today = new Date();
    const minDate = new Date(today);
    minDate.setDate(today.getDate() - 30);
    const maxDate = new Date(today);
    maxDate.setDate(today.getDate() + 9);

    const minDateStr = minDate.toISOString().split('T')[0];
    const maxDateStr = maxDate.toISOString().split('T')[0];
</script>

<div class="container-wrapper">
    <div class="container-secondary">
        <span>Start Date</span>
        <input min={minDateStr} max={maxDateStr} bind:value={config.startDate} type="date" class="field-primary" id="start-day-selector">
    </div>
    <div class="container-secondary">
        <span>Total Days</span>
        <input bind:value={config.releaseDuration} type="number" class="field-primary" id="total-day-field" step="1.0">
    </div>
</div>

<style>
    #start-day-selector, #total-day-field {
        width: var(--width-selector);
    }
</style>