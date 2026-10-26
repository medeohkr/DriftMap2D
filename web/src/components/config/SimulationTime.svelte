<script lang="ts">
    import { config, simulation, simulationTimeState, toast } from "$lib/stores/index.svelte";
    import { loadForcingWindow, utcInputTimestamp, type ForcingWindow } from "$lib/utils";
    import { showToast } from "$lib/stores/toast.svelte";

    let bounds = $state<ForcingWindow | null>(null);
    let loading = $state(true);
    let availabilityError = $state("");
    const datePart = (time: number) => new Date(time).toISOString().slice(0, 10);
    const timePart = (time: number) => new Date(time).toISOString().slice(11, 16);
    const minDateStr = $derived(bounds ? datePart(bounds.start) : undefined);
    const maxDateStr = $derived(bounds ? datePart(bounds.end) : undefined);
    const latestStart = $derived(bounds ? Math.min(bounds.end, utcInputTimestamp(config.endDate, config.endTime) - 60000) : null);
    const earliestEnd = $derived(bounds ? Math.max(bounds.start, utcInputTimestamp(config.startDate, config.startTime) + 60000) : null);
    const availabilityFormat = new Intl.DateTimeFormat("en-GB", {
        day: "2-digit", month: "short", hour: "2-digit", minute: "2-digit", hourCycle: "h23", timeZone: "UTC",
    });

    function updateEndpoint(event: Event, endpoint: "start" | "end", part: "Date" | "Time") {
        const input = event.currentTarget as HTMLInputElement;
        if (!bounds || !input.value) return;
        const date = part === "Date" ? input.value : config[`${endpoint}Date`];
        const time = part === "Time" ? input.value : config[`${endpoint}Time`];
        const requested = utcInputTimestamp(date, time);
        if (!Number.isFinite(requested)) return;
        const first = endpoint === "end" ? earliestEnd! : bounds.start;
        const last = endpoint === "start" ? latestStart! : bounds.end;
        const adjusted = Math.min(Math.max(requested, first), last);
        config[`${endpoint}Date`] = datePart(adjusted);
        config[`${endpoint}Time`] = timePart(adjusted);
        // typed dates can bypass the native picker limits
        input.value = config[`${endpoint}${part}`];
    }

    $effect(() => {
        const model = config.forcingModel;
        // imports and resets can replace dates without changing the forcing model
        const version = simulationTimeState.version;
        let cancelled = false;
        loading = true;
        availabilityError = "";
        loadForcingWindow(model).then(window => {
            if (cancelled || config.forcingModel !== model || simulationTimeState.version !== version) return;
            bounds = window;
            if (simulation.simulationActive) return;
            const start = utcInputTimestamp(config.startDate, config.startTime);
            const end = utcInputTimestamp(config.endDate, config.endTime);
            if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return;
            const clampedStart = Math.min(Math.max(start, window.start), window.end - 60000);
            const clampedEnd = Math.min(Math.max(end, clampedStart + 60000), window.end);
            if (Number.isFinite(clampedStart) && clampedStart !== start) {
                config.startDate = datePart(clampedStart);
                config.startTime = timePart(clampedStart);
            }
            if (Number.isFinite(clampedEnd) && clampedEnd !== end) {
                config.endDate = datePart(clampedEnd);
                config.endTime = timePart(clampedEnd);
            }
            if (clampedStart !== start || clampedEnd !== end) {
                const forcing = model === "norkyst" ? "NorKyst" : "CMEMS";
                const message = `Simulation interval adjusted to fit ${forcing} availability`;
                const markerMoved = toast.visible && toast.message === "Default release moved to Norway";
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
                    max={latestStart === null ? undefined : datePart(latestStart)}
                    value={config.startDate}
                    oninput={event => updateEndpoint(event, "start", "Date")}
                    onblur={event => { event.currentTarget.value = config.startDate; }}
                    disabled={!bounds || simulation.simulationActive}
                />
                <input
                    type="time"
                    class="datetime-input"
                    aria-describedby="forcing-time-availability"
                    step="60"
                    min={bounds && config.startDate === minDateStr ? timePart(bounds.start) : "00:00"}
                    max={latestStart !== null && config.startDate === datePart(latestStart) ? timePart(latestStart) : "23:59"}
                    disabled={!bounds || simulation.simulationActive}
                    value={config.startTime}
                    oninput={event => updateEndpoint(event, "start", "Time")}
                    onblur={event => { event.currentTarget.value = config.startTime; }}
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
                    min={earliestEnd === null ? undefined : datePart(earliestEnd)}
                    max={maxDateStr}
                    value={config.endDate}
                    oninput={event => updateEndpoint(event, "end", "Date")}
                    onblur={event => { event.currentTarget.value = config.endDate; }}
                    disabled={!bounds || simulation.simulationActive}
                />
                <input
                    type="time"
                    class="datetime-input"
                    aria-describedby="forcing-time-availability"
                    step="60"
                    min={earliestEnd !== null && config.endDate === datePart(earliestEnd) ? timePart(earliestEnd) : "00:00"}
                    max={bounds && config.endDate === maxDateStr ? timePart(bounds.end) : "23:59"}
                    disabled={!bounds || simulation.simulationActive}
                    value={config.endTime}
                    oninput={event => updateEndpoint(event, "end", "Time")}
                    onblur={event => { event.currentTarget.value = config.endTime; }}
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
