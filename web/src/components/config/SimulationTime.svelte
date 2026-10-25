<script lang="ts">
    import { untrack } from "svelte";
    import { config, simulation, simulationTimeState, toast } from "$lib/stores/index.svelte";
    import { loadForcingWindow, defaultSimulationWindow, utcInputTimestamp, type ForcingWindow, type ForcingModel } from "$lib/utils";
    import { showToast } from "$lib/stores/toast.svelte";

    let bounds = $state<ForcingWindow | null>(null);
    let loading = $state(true);
    let availabilityError = $state("");
    let previousModel = config.forcingModel;
    let displayedModel = config.forcingModel;
    let editVersion = 0;
    let previousDefaultVersion = simulationTimeState.defaultVersion;
    let previousImportVersion = simulationTimeState.importVersion;
    const intervals = new Map<ForcingModel, ForcingWindow & { usingDefault: boolean }>();
    const datePart = (time: number) => new Date(time).toISOString().slice(0, 10);
    const timePart = (time: number) => new Date(time).toISOString().slice(11, 16);
    const minDateStr = $derived(bounds ? datePart(bounds.start) : undefined);
    const maxDateStr = $derived(bounds ? datePart(bounds.end) : undefined);
    const availabilityFormat = new Intl.DateTimeFormat("en-GB", {
        day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit", hourCycle: "h23", timeZone: "UTC",
    });

    function markCustomInterval() {
        simulationTimeState.usingDefault = false;
        displayedModel = config.forcingModel;
        editVersion++;
    }

    $effect(() => {
        const model = config.forcingModel;
        const defaultVersion = simulationTimeState.defaultVersion;
        const importVersion = simulationTimeState.importVersion;
        const replaced = defaultVersion !== previousDefaultVersion || importVersion !== previousImportVersion;
        const forcingChanged = model !== previousModel;
        const request = untrack(() => {
            if (replaced) {
                intervals.clear();
                displayedModel = model;
            }
            if (forcingChanged && !replaced && !simulation.simulationActive) {
                const start = utcInputTimestamp(config.startDate, config.startTime);
                const end = utcInputTimestamp(config.endDate, config.endTime);
                if (Number.isFinite(start) && Number.isFinite(end) && end > start) {
                    intervals.set(displayedModel, { start, end, usingDefault: simulationTimeState.usingDefault });
                } else {
                    intervals.delete(displayedModel);
                }
            }
            const saved = intervals.get(model);
            return { saved, needsDefault: simulationTimeState.usingDefault && !saved, editVersion };
        });
        previousModel = model;
        previousDefaultVersion = defaultVersion;
        previousImportVersion = importVersion;
        let cancelled = false;
        loading = true;
        availabilityError = "";
        loadForcingWindow(model).then(window => {
            if (cancelled || config.forcingModel !== model) return;
            bounds = window;
            if (simulation.simulationActive) return;
            const start = utcInputTimestamp(config.startDate, config.startTime);
            const end = utcInputTimestamp(config.endDate, config.endTime);
            const edited = editVersion !== request.editVersion;
            const saved = edited ? undefined : request.saved;
            const usingDefault = saved ? saved.usingDefault : simulationTimeState.usingDefault;
            const requested = saved ?? (request.needsDefault && !edited && usingDefault
                ? defaultSimulationWindow(window) : { start, end });
            const requestedStart = requested.start;
            const requestedEnd = requested.end;
            const clampedStart = Math.min(Math.max(requestedStart, window.start), window.end - 60000);
            const clampedEnd = Math.min(Math.max(requestedEnd, clampedStart + 60000), window.end);
            simulationTimeState.usingDefault = usingDefault;
            displayedModel = model;
            if (Number.isFinite(clampedStart) && clampedStart !== start) {
                config.startDate = datePart(clampedStart);
                config.startTime = timePart(clampedStart);
            }
            if (Number.isFinite(clampedEnd) && clampedEnd !== end) {
                config.endDate = datePart(clampedEnd);
                config.endTime = timePart(clampedEnd);
            }
            if (Number.isFinite(clampedStart) && Number.isFinite(clampedEnd)) {
                intervals.set(model, { start: clampedStart, end: clampedEnd, usingDefault });
            }
            const clipped = Number.isFinite(requestedStart) && Number.isFinite(requestedEnd) &&
                (clampedStart !== requestedStart || clampedEnd !== requestedEnd);
            if (clipped && !usingDefault) {
                const forcing = model === "norkyst" ? "NorKyst" : "CMEMS";
                const message = `Simulation interval adjusted to fit ${forcing} availability`;
                const markerMoved = toast.visible && ["Default release moved to Norway", "Default release moved back to Vancouver"].includes(toast.message);
                showToast(markerMoved
                    ? `${toast.message}\n${message}` : message);
            }
        }).catch(error => {
            if (!cancelled) availabilityError = String(error);
        }).finally(() => {
            if (!cancelled && config.forcingModel === model) loading = false;
        });
        return () => { cancelled = true; };
    });
</script>

<div class="floating-container" aria-busy={loading}>
    <span>Simulation Time Interval (UTC)</span>
    {#if availabilityError}
        <span class="inline-label">{availabilityError}</span>
    {/if}
    
    <div class="time-picker">
    <div class="simulation-time-container">
        <div class="inline-container light">
            <span class="inline-label">Start</span>
            <div class="date-time-container">
                <input
                    type="date"
                    class="datetime-input"
                    aria-describedby="forcing-time-availability"
                    min={minDateStr}
                    max={maxDateStr}
                    bind:value={config.startDate}
                    oninput={markCustomInterval}
                    disabled={!bounds || simulation.simulationActive}
                />
                <input
                    type="time"
                    class="datetime-input"
                    aria-describedby="forcing-time-availability"
                    step="60"
                    min={bounds && config.startDate === minDateStr ? timePart(bounds.start) : "00:00"}
                    max={bounds && config.startDate === maxDateStr ? timePart(bounds.end) : "23:59"}
                    disabled={!bounds || simulation.simulationActive}
                    bind:value={config.startTime}
                    oninput={markCustomInterval}
                />
            </div>
        </div>
        
        <div class="inline-container">
            <span class="inline-label">End</span>
            <div class="date-time-container">
                <input
                    type="date"
                    class="datetime-input"
                    aria-describedby="forcing-time-availability"
                    min={minDateStr}
                    max={maxDateStr}
                    bind:value={config.endDate}
                    oninput={markCustomInterval}
                    disabled={!bounds || simulation.simulationActive}
                />
                <input
                    type="time"
                    class="datetime-input"
                    aria-describedby="forcing-time-availability"
                    step="60"
                    min={bounds && config.endDate === minDateStr ? timePart(bounds.start) : "00:00"}
                    max={bounds && config.endDate === maxDateStr ? timePart(bounds.end) : "23:59"}
                    disabled={!bounds || simulation.simulationActive}
                    bind:value={config.endTime}
                    oninput={markCustomInterval}
                />
            </div>
        </div>
    </div>
    <span id="forcing-time-availability" class="availability-info" aria-live="polite">
        {#if bounds}
            Available (UTC) {availabilityFormat.format(bounds.start)} – {availabilityFormat.format(bounds.end)}
        {:else if loading}
            Loading forcing availability…
        {:else}
            Forcing availability unavailable
        {/if}
    </span>
    </div>
</div>

<style>
.time-picker {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xxs);
}

.simulation-time-container {
    display: flex;
    flex-direction: column;
    row-gap: var(--spacing-xs);
}

.availability-info {
    color: var(--text-muted);
    font-size: var(--font-size-xxs);
    line-height: 1.4;
    margin: 0;
    padding: 0;
    white-space: nowrap;
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
