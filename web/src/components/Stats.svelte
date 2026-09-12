<script lang="ts">
    import { simulation, config, stats } from "$lib/stores/index.svelte";
</script>

{#if simulation.simulationActive}
    {#if config.tracerType == "oil"}
        <div class="stats-container" id="stats-container">
            <div class="container-secondary">
                <span class="stats-primary-text">Stranded</span>
                <span class="unit-text" id="stranded">{stats.stranded}%</span>
            </div>
            <div class="container-secondary">
                <span class="stats-primary-text">Emulsified</span>
                <span class="unit-text" id="emulsified"
                    >{stats.emulsified}%</span
                >
            </div>
            <div class="container-secondary">
                <span class="stats-primary-text">Evaporated</span>
                <span class="unit-text" id="evaporated"
                    >{stats.evaporated}%</span
                >
            </div>
            <div class="container-secondary">
                <span class="stats-primary-text">Floating Mass</span>
                <span class="unit-text" id="total-mass"
                    >{stats.totalMass} t</span
                >
            </div>
        </div>
    {/if}
{/if}

<style>
    .stats-container {
        display: grid;
        position: absolute;
        right: var(--spacing-md);
        top: var(--spacing-md);
        padding: var(--spacing-xs) 0px var(--spacing-xs) var(--spacing-xs);
        row-gap: var(--spacing-xxs);
        background-color: var(--bg-secondary);
        box-shadow: var(--shadow-size-secondary) var(--shadow-secondary);
        border-radius: var(--border-md);
    }

    .stats-primary-text {
        font-size: var(--font-size-sm);
        font-weight: var(--weight-primary);
        color: var(--text-secondary);
        margin-right: 50px;
    }

    #stranded,
    #emulsified,
    #evaporated,
    #total-mass {
        width: var(--width-stats-units);
    }
</style>
