<script lang="ts">
    import { config } from '$lib/stores.svelte';
    import { searchObjects, getObjectJsonForRust, getGenericObjects } from '$lib/objects';
    import { createProteus } from '$lib/simulation';

    interface SarObject {
        name: string;
        downwind: number[];
        right: number[];
        left: number[];
    }

    let query = $state('');
    let results: SarObject[] = $state([]);
    let isFocused = $state(false);

    $effect(() => {
        if (query.length > 0) {
            results = searchObjects(query);
        } else if (isFocused) {
            results = getGenericObjects();
        }
    });

    function selectObject(obj: SarObject) {
        config.tracerJson = getObjectJsonForRust(obj.name);
        query = obj.name;
        results = [];
        isFocused = false;

        createProteus();
    }
</script>

<div class="object-selector-container">
    <div class="floating-container">
        <span>SAR Object</span>
        <input
            class="selector-primary object-search"
            type="text"
            bind:value={query}
            onfocus={() => isFocused = true}
            onblur={() => isFocused = false}
            placeholder="Search the SAROPS Leeway Database..."
        />
    </div>

    {#if isFocused && results.length > 0}
        <ul class="object-results">
            {#each results as object}
                <li>
                    <button
                        class="object-item"
                        onmousedown={() => selectObject(object)}
                    >
                        <span class="object-name">{object.name}</span>
                    </button>
                </li>
            {/each}
        </ul>
    {/if}
</div>

<style>
.object-selector-container {
    position: relative;
    width: 100%;
}

.object-search {
    width: 100%;
    text-overflow: ellipsis;
}

.object-results {
    position: absolute;
    top: 100%;
    overflow-x: auto;
    overflow-y: auto;
    width: 100%;
    max-height: var(--search-results-height);
    padding: 0;
    background: var(--bg-tertiary);
    border-radius: var(--border-md);
    box-shadow: 2px 2px 5px var(--shadow-secondary);
    scrollbar-width: thin;
    list-style: none;
    z-index: 10;
    margin-top: 3.5px;
}

.object-results li {
    margin: 0;
    padding: 0;
}

.object-item {
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

.object-item:hover {
    color: var(--text-primary);
    background: var(--text-secondary);
}

.object-item:last-child {
    border-bottom: none;
}

.object-name {
    flex: 1;
    white-space: nowrap;
}
</style>