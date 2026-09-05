<script lang="ts">
    import { config } from "$lib/stores.svelte";

    const today = new Date();
    const minDate = new Date(today);
    minDate.setDate(today.getDate() - 30);
    const maxDate = new Date(today);
    maxDate.setDate(today.getDate() + 9);

    const minDateStr = minDate.toISOString().split("T")[0];
    const maxDateStr = maxDate.toISOString().split("T")[0];
</script>

<div class="floating-container">
    <span>Simulation Time Interval</span>
    
    <div class="simulation-time-container">
        <div class="inline-container top">
            <span class="inline-label">Start</span>
            <div class="date-time-container">
                <input
                    type="date"
                    class="datetime-input"
                    min={minDateStr}
                    max={maxDateStr}
                    bind:value={config.startDate}
                />
                <input
                    type="time"
                    class="datetime-input"
                    step="60"
                    bind:value={config.startTime}
                />
            </div>
        </div>
        
        <div class="inline-container bottom">
            <span class="inline-label">End</span>
            <div class="date-time-container">
                <input
                    type="date"
                    class="datetime-input"
                    min={minDateStr}
                    max={maxDateStr}
                    bind:value={config.endDate}
                />
                <input
                    type="time"
                    class="datetime-input"
                    step="60"
                    bind:value={config.endTime}
                />
            </div>
        </div>
    </div>
</div>

<style>
.simulation-time-container {
    display: flex;
    flex-direction: column;
    row-gap: var(--spacing-sm);
}

.date-time-container {
    display: flex;
    column-gap: var(--spacing-xxs)
}

.datetime-input {
    background: transparent;
    border: none;
    color: var(--text-primary);
    font-family: var(--font-family);
    font-size: var(--font-size-xs);
    font-weight: var(--weight-secondary);
    text-align: right;
    cursor: pointer;
}

.inline-container.top {
    border-radius: var(--border-md) var(--border-md) 0 0;
}

.inline-container.bottom {
    border-radius: 0 0 var(--border-md) var(--border-md);
    background-color: var(--bg-secondary)
}
</style>