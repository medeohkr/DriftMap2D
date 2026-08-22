<script lang="ts">
    import ModelTracer from './ModelTracer.svelte';
</script>
<div class="sidebar-content">
    <details class="details-primary" open>
        <summary class="summary-primary">
            <span class="summary-title">Model & Tracer</span>
            <span class="summary-indicator">▶</span>
        </summary>
        <ModelTracer />
    </details>
    <details class="details-primary" open>
        <summary class="summary-primary">
            <span class="summary-title">Timing & Release</span>
            <span class="summary-indicator">▶</span>
        </summary>
    </details>
    <details class="details-primary" open>
        <summary class="summary-primary ">
            <span class="summary-title">Advection & Diffusion</span>
            <span class="summary-indicator">▶</span>
        </summary>                </details>
    <details class="details-primary" open>
        <summary class="summary-primary">
            <span class="summary-title">Visualization</span>
            <span class="summary-indicator">▶</span>
        </summary>
    </details>
</div>
<style>
    .details-primary {
        display: flex;
        flex-direction: column;
    }

    .summary-primary {
        display: flex;
        width: var(--width-sidebar);
        max-width: 100%;
        padding: var(--spacing-xxs) var(--spacing-lg);
        margin: var(--spacing-xxs) 0px;
        color: var(--text-secondary);
        background-color: var(--bg-summary);
        font-family: var(--font-family);
        font-size: var(--font-size-md);
        font-weight: var(--weight-primary);
        list-style: none;
        cursor: pointer;
        box-shadow: var(--shadow-size-secondary) var(--shadow-primary);
        user-select: none;
    }

    .summary-title {
        width: var(--width-summary-text);
    }

    .summary-indicator {
        font-size: var(--font-size-sm);
        transition: var(--transition-medium);
        transform: translateY(2px);
    }

    .details-primary[open] .summary-indicator {
        transform: rotate(90deg) translateX(2px) translateY(2px);
    }
</style>