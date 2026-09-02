<script lang="ts">
    import { config } from '$lib/stores.svelte';
    import { searchOils, type OilRecord, getOilJsonForRust, getGenericOils } from '$lib/oils';
    import { createProteus } from '$lib/simulation';

    let query = $state('');
    let results: OilRecord[] = $state([]);
    let isFocused = $state(false);

    $effect(() => {
        if (query.length > 0) {
            results = searchOils(query);
        } else if (isFocused) {
            results = getGenericOils();
        }
    });

    function selectOil(oil: OilRecord) {
        config.oilJson = getOilJsonForRust(oil.oil_id);
        query = oil.name || oil.oil_id;
        results = [];
        isFocused = false;

        createProteus();
    }
</script>

<div class="oil-selector-container">
    <div class="container-secondary">
        <span>Oil Type</span>
        <input
            class="field-primary"
            id="oil-search"
            type="text"
            bind:value={query}
            onfocus={() => isFocused = true}
            onblur={() => isFocused = false}
            placeholder="Search oil..."
        />
    </div>

    {#if isFocused && results.length > 0}
        <ul class="oil-results">
            {#each results as oil}
                <li>
                    <button
                        class="oil-item"
                        onmousedown={() => selectOil(oil)}
                    >
                        <span class="oil-name">{oil.name || oil.oil_id}</span>
                        <span class="oil-api">API: {oil.api_gravity?.toFixed(1) ?? '?'}</span>
                    </button>
                </li>
            {/each}
        </ul>
    {/if}
</div>

<style>
    .oil-selector-container {
        position: relative;
        width: 100%;
    }

    #oil-search {
        width: var(--width-selector);
        text-overflow: ellipsis;
    }

    .oil-results {
        position: absolute;
        top: 100%;
        left: 50%;
        transform: translateX(-50%);
        max-height: var(--oil-results-height);
        overflow-y: auto;
        width: var(--width-container);
        list-style: none;
        padding: 0;
        margin: var(--spacing-xxs) 0 0 0;
        background: var(--bg-selector);
        border: var(--border-md) solid var(--text-muted);
        border-radius: var(--border-xl);
        z-index: 100;
        box-shadow: var(--shadow-size-secondary) var(--shadow-secondary);
    }

    .oil-results li {
        margin: 0;
        padding: 0;
    }

    .oil-item {
        width: 100%;
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: var(--spacing-xs) var(--spacing-sm);
        background: none;
        border: none;
        border-bottom: var(--border-sm) solid var(--border-color);
        color: var(--bg-primary);
        font-family: var(--font-family);
        font-weight: var(--weight-primary);
        font-size: var(--font-size-xxs);
        cursor: pointer;
        text-align: left;
    }

    .oil-item:hover {
        background: var(--text-secondary);
    }

    .oil-item:last-child {
        border-bottom: none;
    }

    .oil-name {
        flex: 1;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .oil-api {
        margin-left: var(--spacing-sm)
    }
</style>