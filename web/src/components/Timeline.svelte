

<div id="timeline-container" class="timeline-container" style="display:none;">
    <div class="timeline-controls">
        <button id="timeline-rewind" class="timeline-btn">
            <img src="assets/images/RewindBtn.webp" alt="Timeline Rewind Button" style="width: 12px;">
        </button>
        <button id="timeline-play" class="timeline-btn">▶</button>
        <button id="timeline-pause" class="timeline-btn" style="display:none;">
            <img src="assets/images/PauseBtn.webp" alt="Timeline Pause Button" style="width: 9px; height: 10px;">
        </button>
        <button id="timeline-speed" class="timeline-btn">1x</button>
    </div>
    <input type="range" id="timeline-slider" class="timeline-slider" min="0" max="0" value="0">
    <div class="timeline-labels">
        <span id="timeline-start">Day 0</span>
        <span id="timeline-current">Day 0</span>
        <span id="timeline-end">Day 0</span>
    </div>
</div>

<style>
    .timeline-container {
        position: absolute;
        display: none;
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
        background-color: var(--bg-secondary);;
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
</style>