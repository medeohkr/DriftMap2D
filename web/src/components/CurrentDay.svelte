<script lang="ts">
    import { simulation } from '$lib/stores.svelte'
</script>

{#if simulation.simulationActive}
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
</style>