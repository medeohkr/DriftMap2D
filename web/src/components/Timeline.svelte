<script lang="ts">
    import { onDestroy, untrack } from "svelte";
    import rewindBtn from "$lib/assets/images/RewindBtn.webp";
    import pauseBtn from "$lib/assets/images/PauseBtn.webp";
    import { config, stats, timeline, history, visualization} from "$lib/stores/index.svelte";
    import { map } from "$lib/map";

    const lastStep = $derived(timeline.playbackMode ? Math.max(0, history.simulationHistory.length - 1) : 0);
    let previousTime = 0;
    let progress = 0;

    $effect(() => {
        if (timeline.playbackMode) {
            updateDisplay(history.simulationHistory.length - 1);
        }
    });

    $effect(() => {
        if (!timeline.playbackMode) untrack(() => {
            stopPlayback();
            progress = 0;
        });
    });
    onDestroy(stopPlayback);

    function updateDisplay(index: number) {
        const s = history.simulationHistory[index];
        if (!s) return;
        timeline.timelineDay = index;
        timeline.timelineDate = s.dateStr;

        stats.activeParticles = s.stats.activeParticles;
        stats.stranded = s.stats.stranded;
        stats.emulsified = s.stats.emulsified;
        stats.evaporated = s.stats.evaporated;
        stats.totalMass = s.stats.totalMass;

        map.getSource("particles-unstranded").setData(s.unstrandedGeojson);
        map.getSource("particles-stranded").setData(s.strandedGeojson);
        map.getSource("concentration").setData(s.heatmapGeojson);
    }

    function advancePlayback(now: number) {
        if (!timeline.timelinePlaying || !timeline.playbackMode) return;
        const snapshotMinutes = config.timeStepMin * visualization.snapshotInterval;
        const interval = timeline.playbackSpeed * (snapshotMinutes / 30);
        progress += Math.max(0, now - previousTime) / interval;
        previousTime = now;
        const steps = Math.floor(progress);
        if (steps > 0) {
            progress -= steps;
            // render only the latest due snapshot when several fit within one display frame
            updateDisplay(Math.min(timeline.timelineDay + steps, history.simulationHistory.length - 1));
        }
        if (timeline.timelineDay >= history.simulationHistory.length - 1) {
            stopPlayback();
            progress = 0;
        }
    }

    function timelinePlayback(now: number) {
        timeline.timelineAnimationId = null;
        advancePlayback(now);
        if (timeline.timelinePlaying && timeline.playbackMode) {
            timeline.timelineAnimationId = requestAnimationFrame(timelinePlayback);
        }
    }

    function startPlayback() {
        if (timeline.timelinePlaying || !timeline.playbackMode || timeline.timelineDay >= lastStep) return;
        previousTime = performance.now();
        timeline.timelinePlaying = true;
        timeline.timelineAnimationId = requestAnimationFrame(timelinePlayback);
    }

    function stopPlayback() {
        if (timeline.timelineAnimationId !== null) cancelAnimationFrame(timeline.timelineAnimationId);
        timeline.timelineAnimationId = null;
        timeline.timelinePlaying = false;
    }

    function pausePlayback() {
        advancePlayback(performance.now());
        stopPlayback();
    }

    function updatePlaybackSpeed() {
        advancePlayback(performance.now());
        if (timeline.playbackSpeed === 40) {
            timeline.playbackSpeed = 20;
        } else if (timeline.playbackSpeed === 20) {
            timeline.playbackSpeed = 10;
        } else if (timeline.playbackSpeed === 10) {
            timeline.playbackSpeed = 80;
        } else {
            timeline.playbackSpeed = 40;
        }
    }

    function goToStart() {
        stopPlayback();
        progress = 0;
        updateDisplay(0);
    }

    function onSliderInput(e: Event) {
        const target = e.target as HTMLInputElement;
        const index = parseInt(target.value, 10);
        progress = 0;
        previousTime = performance.now();
        updateDisplay(index);
    }
</script>

    <div id="timeline-container" class="timeline-container" class:inactive={!timeline.playbackMode}
        inert={!timeline.playbackMode} aria-hidden={!timeline.playbackMode}>
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
                onclick={startPlayback}
                class:active={!timeline.timelinePlaying}
                id="timeline-play"
                class="timeline-btn">▶</button
            >
            <button
                onclick={pausePlayback}
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
                class="timeline-btn">{40 / timeline.playbackSpeed}x</button
            >
        </div>
        <input
            oninput={onSliderInput}
            type="range"
            id="timeline-slider"
            class="timeline-slider"
            min="0"
            max={lastStep}
            value={timeline.timelineDay}
        />
        <div class="timeline-labels">
            <span>{timeline.timelineDate || '\u00a0'}</span>
        </div>
    </div>

<style>
    .timeline-container.inactive {
        visibility: hidden;
        pointer-events: none;
    }
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
        border: none;
        border-radius: var(--border-lg);
        box-shadow: var(--shadow-size-secondary) var(--shadow-secondary);
    }
    @media (max-width: 768px) {
        .timeline-container {
            position: fixed;
            left: var(--spacing-sm);
            right: var(--spacing-md);
            width: auto;
            transform: none;
            padding: var(--spacing-sm);
        }
        .timeline-container .timeline-controls { column-gap: var(--spacing-xs); }
        .timeline-container .timeline-btn { width: 40px; }
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
        padding: var(--spacing-xxs) 0;
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
        background-color: var(--text-secondary);
        border: 2px solid var(--text-muted);
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
