<script lang="ts">
    import rewindBtn from "$lib/assets/images/RewindBtn.webp";
    import pauseBtn from "$lib/assets/images/PauseBtn.webp";
    import { stats, timeline, history} from "$lib/stores.svelte";
    import { map } from "$lib/map";

    $effect(() => {
        if (timeline.playbackMode) {
            timeline.timelineDay = history.simulationHistory.length - 1
            timeline.timelineDate = history.simulationHistory[history.simulationHistory.length - 1].dateStr;
            updateDisplay(history.simulationHistory.length - 1);
        }
    });

    function updateDisplay(index: number) {
        const s = history.simulationHistory[index];
        if (!s) return;

        timeline.timelineDay = index;
        timeline.timelineDate = s.dateStr;

        stats.stranded = s.stranded;
        stats.emulsified = s.emulsified;
        stats.evaporated = s.evaporated;
        stats.totalMass = s.totalMass;

        map.getSource("particles-unstranded").setData(s.unstrandedGeojson);
        map.getSource("particles-stranded").setData(s.strandedGeojson);
        map.getSource("concentration").setData(s.heatmapGeojson);
    }

    function timelinePlayback() {
        if (!timeline.timelinePlaying) return;
        if (timeline.timelineDay < history.simulationHistory.length - 1) {
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
        if (timeline.playbackSpeed === 60) {
            timeline.playbackSpeed = 30;
        } else if (timeline.playbackSpeed === 30) {
            timeline.playbackSpeed = 15;
        } else {
            timeline.playbackSpeed = 60;
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
                class="timeline-btn">{60 / timeline.playbackSpeed}x</button
            >
        </div>
        <input
            oninput={onSliderInput}
            type="range"
            id="timeline-slider"
            class="timeline-slider"
            min="0"
            max={history.simulationHistory.length - 1}
            value={timeline.timelineDay}
        />
        <div class="timeline-labels">
            <span>{timeline.timelineDate}</span>
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
        background: var(--bg-timeline);
        border: none;
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
        background-color: var(--bg-tertiary);
        color: var(--text-primary);
        padding: var(--spacing-xxs) var(--spacing-md);
        border: none;
        border-radius: var(--border-md);
        font-family: var(--font-family);
        font-size: var(--font-size-md);
        cursor: pointer;
        opacity: 0.8;
    }

    .timeline-btn:hover {
        opacity: 0.6;
    }

    .timeline-slider {
        width: 100%;
        height: var(--spacing-xxs);
        -webkit-appearance: none;
        appearance: none;
        background-color: var(--bg-tertiary);
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
        justify-content: center;
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
