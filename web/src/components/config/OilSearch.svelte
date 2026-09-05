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
    <div class="floating-container">
        <span>Oil Type</span>
        <input
            class="selector-primary oil-search"
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

.oil-search {
    width: 100%;

    text-overflow: ellipsis;
}

.oil-results {
    position: absolute;
    top: 100%;
    right: 0;
    overflow-y: auto;
    width: calc(100% - var(--spacing-md));
    max-height: var(--oil-results-height);
    margin: 6px 0;
    padding: 0;
    background: var(--bg-tertiary);
    border-radius: var(--border-md);
    box-shadow: 2px 2px 5px var(--shadow-secondary);
    scrollbar-width: thin;
    list-style: none;
    z-index: 10;
}

.oil-results li {
    margin: 0;
    padding: 0;
}

.oil-item {
    display: flex;
    width: 100%;
    padding: var(--spacing-xs) var(--spacing-sm);
    font-family: var(--font-family);
    font-size: var(--font-size-xxs);
    font-weight: var(--weight-primary);
    text-align: left;
    color: var(--text-muted);
    background: none;
    border: none;
    cursor: pointer;

    justify-content: space-between;
    align-items: center;
    border-bottom: var(--border-sm) solid var(--border-color);
}

.oil-item:hover {
    color: var(--text-primary);
    background: var(--text-secondary);
}

.oil-item:last-child {
    border-bottom: none;
}

.oil-name {
    overflow: hidden;
    flex: 1;

    text-overflow: ellipsis;
    white-space: nowrap;
}

.oil-api {
    margin-left: var(--spacing-sm);
}


</style>