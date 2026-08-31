<script lang="ts">
    import rewindBtn from "$lib/assets/images/RewindBtn.webp";
    import pauseBtn from "$lib/assets/images/PauseBtn.webp";
    import { simulation, stats, timeline } from "$lib/stores.svelte";
    import { map } from "$lib/map";
    import { GeoJSONSource } from "maplibre-gl";

    $effect(() => {
        if (timeline.playbackMode) {
            timeline.timelineDay = simulation.simulationHistory.length - 1
            timeline.timelineDate = simulation.simulationHistory[simulation.simulationHistory.length - 1].dateStr;
            updateDisplay(simulation.simulationHistory.length - 1);
        }
    });

    function updateDisplay(index: number) {
        const s = simulation.simulationHistory[index];
        if (!s) return;

        timeline.timelineDay = index;
        timeline.timelineDate = s.dateStr;

        stats.stranded = s.stranded;
        stats.emulsified = s.emulsified;
        stats.evaporated = s.evaporated;
        stats.totalMass = s.totalMass;

        (map.getSource("particles-unstranded") as GeoJSONSource).setData(
            s.unstrandedGeojson,
        );
        (map.getSource("particles-stranded") as GeoJSONSource).setData(
            s.strandedGeojson,
        );
        (map.getSource("concentration") as GeoJSONSource).setData(
            s.heatmapGeojson,
        );
    }

    function timelinePlayback() {
        if (!timeline.timelinePlaying) return;
        if (timeline.timelineDay < simulation.simulationHistory.length - 1) {
            timeline.timelineDay++;
            updateDisplay(timeline.timelineDay);
            timeline.timelineAnimationId = setTimeout(
                () => requestAnimationFrame(timelinePlayback),
                timeline.playbackSpeed,
            );
        } else {
            timeline.timelinePlaying = false;
        }
    }

    function updatePlaybackSpeed() {
        if (timeline.playbackSpeed === 100) {
            timeline.playbackSpeed = 50;
        } else if (timeline.playbackSpeed === 50) {
            timeline.playbackSpeed = 25;
        } else {
            timeline.playbackSpeed = 100;
        }
    }

    function goToStart() {
        timeline.timelinePlaying = false;
        updateDisplay(0);
    }

    function onSliderInput(e: Event) {
        const target = e.target as HTMLInputElement;
        const index = parseInt(target.value, 10);
        timeline.timelineDay = index;
        updateDisplay(index);
    }
</script>

{#if timeline.playbackMode}
    <div id="timeline-container" class="timeline-container">
        <div class="timeline-controls">
            <button
                onclick={goToStart}
                id="timeline-rewind"
                class="timeline-btn"
            >
                <img
                    src={rewindBtn}
                    alt="Timeline Rewind Button"
                    style="width: 12px;"
                />
            </button>
            <button
                onclick={() => {timeline.timelinePlaying = true; timelinePlayback()}}
                class:active={!timeline.timelinePlaying}
                id="timeline-play"
                class="timeline-btn">▶</button
            >
            <button
                onclick={() => timeline.timelinePlaying = false}
                class:active={timeline.timelinePlaying}
                id="timeline-pause"
                class="timeline-btn"
            >
                <img
                    src={pauseBtn}
                    alt="Timeline Pause Button"
                    style="width: 9px; height: 10px;"
                />
            </button>
            <button
                onclick={updatePlaybackSpeed}
                id="timeline-speed"
                class="timeline-btn">{100 / timeline.playbackSpeed}x</button
            >
        </div>
        <input
            oninput={onSliderInput}
            type="range"
            id="timeline-slider"
            class="timeline-slider"
            min="0"
            max={simulation.simulationHistory.length - 1}
            value={timeline.timelineDay}
        />
        <div class="timeline-labels">
            <span id="timeline-start">Day 0</span>
            <span id="timeline-current">{timeline.timelineDate}</span>
            <span id="timeline-end"
                >Day {simulation.simulationHistory[
                    simulation.simulationHistory.length - 1
                ]?.day ?? 0}</span
            >
        </div>
    </div>
{/if}

<style>
    .timeline-container {
        position: absolute;
        display: flex;
        flex-direction: column;
        bottom: var(--spacing-md);
        left: 50%;
        width: 50%;
        row-gap: var(--spacing-sm);
        padding: var(--spacing-md) var(--spacing-lg);
        transform: translateX(-45%);
        background: var(--bg-primary);
        border: var(--border-lg) solid var(--bg-secondary);
        border-radius: var(--border-lg);
        box-shadow: var(--shadow-size-secondary) var(--shadow-secondary);
    }

    .timeline-controls {
        display: flex;
        column-gap: var(--spacing-sm);
        justify-content: center;
    }

    .timeline-btn {
        width: var(--width-timeline-btn);
        height: var(--height-timeline-btn);
        background-color: var(--bg-secondary);
        color: var(--text-primary);
        padding: var(--spacing-xxs) var(--spacing-md);
        border: var(--border-sm) solid var(--import-border);
        border-radius: var(--border-lg);
        font-family: var(--font-family);
        font-size: var(--font-size-md);
        cursor: pointer;
    }

    .timeline-btn:hover {
        background-color: var(--timeline-slider);
    }

    .timeline-slider {
        width: 100%;
        height: var(--spacing-xxs);
        -webkit-appearance: none;
        appearance: none;
        background: var(--timeline-slider);
        border-radius: var(--border-md);
        outline: none;
    }

    .timeline-slider::-webkit-slider-thumb {
        -webkit-appearance: none;
        appearance: none;
        width: var(--spacing-md);
        height: var(--spacing-md);
        border-radius: 50%;
        background: var(--button-primary);
        cursor: pointer;
    }

    .timeline-labels {
        display: flex;
        justify-content: space-between;
        color: var(--text-muted);
        font-family: var(--font-family);
        font-size: var(--font-size-sm);
    }

    #timeline-play {
        display: none;
    }

    #timeline-pause {
        display: none;
    }

    #timeline-play.active {
        display: block;
    }

    #timeline-pause.active {
        display: block;
    }
</style>
