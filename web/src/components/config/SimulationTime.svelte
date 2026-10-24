<script lang="ts">
    import { config, simulation } from "$lib/stores/index.svelte";
    import { loadForcingWindow, utcInputTimestamp, type ForcingWindow } from "$lib/utils";
    import { showToast } from "$lib/stores/toast.svelte";

    let bounds = $state<ForcingWindow | null>(null);
    let availabilityError = $state("");
    let defaultInterval: { start: number; end: number } | null = null;
    const datePart = (time: number) => new Date(time).toISOString().slice(0, 10);
    const timePart = (time: number) => new Date(time).toISOString().slice(11, 16);
    const minDateStr = $derived(bounds ? datePart(bounds.start) : undefined);
    const maxDateStr = $derived(bounds ? datePart(bounds.end) : undefined);

    $effect(() => {
        const model = config.forcingModel;
        let cancelled = false;
        bounds = null;
        availabilityError = "";
        loadForcingWindow(model).then(window => {
            if (cancelled) return;
            bounds = window;
            if (simulation.simulationActive) return;
            const start = utcInputTimestamp(config.startDate, config.startTime);
            const end = utcInputTimestamp(config.endDate, config.endTime);
            const now = new Date();
            const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
            const defaultEnd = today + 7 * 86400000;
            // remember the shortened default so switching back can restore seven days
            const usingDefault = (start === today && end === defaultEnd) ||
                (start === defaultInterval?.start && end === defaultInterval?.end);
            const requestedStart = usingDefault ? today : start;
            const requestedEnd = usingDefault ? defaultEnd : end;
            const clampedStart = Math.min(Math.max(requestedStart, window.start), window.end - 60000);
            const clampedEnd = Math.min(Math.max(requestedEnd, clampedStart + 60000), window.end);
            defaultInterval = usingDefault ? { start: clampedStart, end: clampedEnd } : null;
            let adjusted = false;
            if (Number.isFinite(clampedStart) && clampedStart !== start) {
                config.startDate = datePart(clampedStart);
                config.startTime = timePart(clampedStart);
                adjusted = true;
            }
            if (Number.isFinite(clampedEnd) && clampedEnd !== end) {
                config.endDate = datePart(clampedEnd);
                config.endTime = timePart(clampedEnd);
                adjusted = true;
            }
            if (adjusted) showToast("Simulation time interval adjusted");
        }).catch(error => {
            if (!cancelled) availabilityError = String(error);
        });
        return () => { cancelled = true; };
    });
</script>

<div class="floating-container" aria-busy={!bounds && !availabilityError}>
    <span>Simulation Time Interval (UTC)</span>
    {#if availabilityError}
        <span class="inline-label">{availabilityError}</span>
    {/if}
    
    <div class="simulation-time-container">
        <div class="inline-container light">
            <span class="inline-label">Start</span>
            <div class="date-time-container">
                <input
                    type="date"
                    class="datetime-input"
                    min={minDateStr}
                    max={maxDateStr}
                    bind:value={config.startDate}
                    disabled={!bounds || simulation.simulationActive}
                />
                <input
                    type="time"
                    class="datetime-input"
                    step="60"
                    min={bounds && config.startDate === minDateStr ? timePart(bounds.start) : "00:00"}
                    max={bounds && config.startDate === maxDateStr ? timePart(bounds.end) : "23:59"}
                    disabled={!bounds || simulation.simulationActive}
                    bind:value={config.startTime}
                />
            </div>
        </div>
        
        <div class="inline-container">
            <span class="inline-label">End</span>
            <div class="date-time-container">
                <input
                    type="date"
                    class="datetime-input"
                    min={minDateStr}
                    max={maxDateStr}
                    bind:value={config.endDate}
                    disabled={!bounds || simulation.simulationActive}
                />
                <input
                    type="time"
                    class="datetime-input"
                    step="60"
                    min={bounds && config.endDate === minDateStr ? timePart(bounds.start) : "00:00"}
                    max={bounds && config.endDate === maxDateStr ? timePart(bounds.end) : "23:59"}
                    disabled={!bounds || simulation.simulationActive}
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
    row-gap: var(--spacing-xs);
}

.date-time-container {
    display: flex;
    column-gap: var(--spacing-xxs)
}

.datetime-input {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-family: var(--font-family);
    font-size: var(--font-size-xs);
    font-weight: var(--weight-secondary);
    text-align: right;
    cursor: pointer;
}
</style>
