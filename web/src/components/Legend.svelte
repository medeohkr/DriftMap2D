<script lang="ts">
    import { simulation, config, visualization } from "$lib/stores.svelte";
    import { getScaledConcentrations } from "$lib/visualization";

    const COLORS = [
        "rgb(65, 85, 185)",
        "rgb(60, 150, 130)",
        "rgb(70, 180, 120)",
        "rgb(150, 200, 90)",
        "rgb(195, 210, 100)",
        "rgb(240, 180, 60)",
        "rgb(240, 140, 40)",
        "rgb(220, 80, 40)",
        "rgb(190, 30, 50)",
        "rgb(140, 15, 100)",
    ];

    let scaled = $state(getScaledConcentrations());

    $effect(() => {
        if (!simulation.simulationActive) {
            scaled = getScaledConcentrations();
        }
    });
</script>

{#if visualization.visualizationMode === "heatmap" && simulation.simulationActive}
    <div id="concentration-legend">
        <div class="legend-bars">
            {#each COLORS.slice().reverse() as color, i}
                <div style="background: {color};"></div>
            {/each}
        </div>
        <div class="legend-labels">
            {#each scaled.slice().reverse() as value, i}
                <div>{value.toFixed(4)} tons/km²</div>
            {/each}
        </div>
    </div>
{/if}

<style>
    #concentration-legend {
        position: absolute;
        display: flex;
        bottom: var(--legend-offset);
        right: var(--spacing-md);
        padding: var(--spacing-xs) var(--spacing-sm);
        column-gap: var(--spacing-sm);
        font-family: monospace;
        background-color: var(--bg-header);
        border: var(--border-lg) solid var(--bg-primary);
        border-radius: var(--border-lg);
        box-shadow: var(--shadow-size-secondary) var(--shadow-secondary);
        pointer-events: none;
    }

    .legend-bars {
        display: flex;
        flex-direction: column;
        gap: var(--spacing-xxxs);
    }

    .legend-bars div {
        height: var(--spacing-lg);
        width: var(--width-units);
    }

    .legend-labels {
        display: flex;
        flex-direction: column;
        gap: var(--spacing-xxxs);
        text-align: right;
    }

    .legend-labels div {
        color: var(--text-secondary);
        font-size: var(--font-size-xs);
        line-height: var(--spacing-lg);
        white-space: nowrap;
    }
</style>
