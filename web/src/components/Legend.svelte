<script lang="ts">
    import { simulation, config, visualization } from "$lib/stores/index.svelte";
    import { roundToSigFigs } from "$lib/utils";
    import { getScaledConcentrations, COLORS, PROBABILTIES} from "$lib/visualization";

    let oilScaling = $state(getScaledConcentrations());

    $effect(() => {
        oilScaling = getScaledConcentrations();
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
            {#if config.tracerType === "sar"}
                {#each PROBABILTIES.slice() as value}
                    <div>{value}% Confidence</div>
                {/each}
            {:else}
                {#each oilScaling.slice().reverse() as value}
                    <div>{roundToSigFigs(value, 3)} tons/km²</div>
                {/each}
            {/if}
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
        background-color: var(--bg-timeline);
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
        width: 30px;
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
