<script lang="ts">
    import { simulation, timeline} from "$lib/stores/index.svelte";
</script>

{#if simulation.simulationActive && !timeline.playbackMode}
    <span id="current-day">{simulation.currentTime}</span>
{/if}

<style>
    #current-day {
        position: absolute;
        color: var(--text-primary);
        font-family: var(--font-family);
        font-size: var(--font-size-md);
        font-weight: var(--weight-primary);
        bottom: var(--spacing-lg);
        left: 50%;
        transform: translateX(-50%);
    }
    @media (max-width: 768px) {
        #current-day {
            top: 52px;
            bottom: auto;
            left: auto;
            right: var(--spacing-sm);
            text-align: right;
            transform: none;
            font-size: var(--font-size-sm);
            white-space: nowrap;
        }
    }
</style>
