<script lang="ts">
    import { onMount } from "svelte";
    import { releaseConfig } from "$lib/stores/index.svelte";
    import { getReleaseColor } from "$lib/utils/releases";

    let { scaleElement, attributionElement, toggleGraticule }: {
        scaleElement: HTMLElement;
        attributionElement: HTMLElement;
        toggleGraticule: (visible: boolean) => void;
    } = $props();
    let showGraticule = $state(false);
    let scaleHost: HTMLDivElement;
    let attributionHost: HTMLDivElement;
    let attributionExpanded = $state(false);
    const color = $derived(releaseConfig.releases[0]
        ? getReleaseColor(releaseConfig.releases[0]) : "#37579b");

    onMount(() => {
        scaleHost.appendChild(scaleElement);
        attributionHost.appendChild(attributionElement);
        const button = attributionElement.querySelector("summary")!;
        const syncAttribution = () => {
            // keep MapLibre's style and drag updates from changing the user's choice
            if (!attributionElement.classList.contains("maplibregl-compact")) {
                attributionElement.classList.add("maplibregl-compact");
            }
            if (attributionElement.classList.contains("maplibregl-compact-show") !== attributionExpanded) {
                attributionElement.classList.toggle("maplibregl-compact-show", attributionExpanded);
            }
            if (attributionElement.hasAttribute("open") !== attributionExpanded) {
                attributionElement.toggleAttribute("open", attributionExpanded);
            }
            button.setAttribute("aria-expanded", String(attributionExpanded));
        };
        const toggleAttribution = (event: MouseEvent) => {
            if (!(event.target instanceof Element) || !event.target.closest("summary")) return;
            event.preventDefault();
            event.stopImmediatePropagation();
            attributionExpanded = !attributionExpanded;
            syncAttribution();
        };
        syncAttribution();
        const observer = new MutationObserver(syncAttribution);
        observer.observe(attributionElement, { attributes: true, attributeFilter: ["class", "open"] });
        attributionElement.addEventListener("click", toggleAttribution, true);
        return () => {
            observer.disconnect();
            attributionElement.removeEventListener("click", toggleAttribution, true);
        };
    });
</script>

<div class="map-display">
    <label>
        <span>Show Graticule</span>
        <input
            type="checkbox"
            bind:checked={showGraticule}
            onchange={(event) => toggleGraticule(event.currentTarget.checked)}
            style:accent-color={color}
        />
    </label>
    <div class="scale" bind:this={scaleHost}></div>
    <div class="attribution" bind:this={attributionHost}></div>
</div>

<style>
    .map-display {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        width: max-content;
        padding-right: calc(var(--spacing-md) - 10px);
        padding-bottom: calc(var(--spacing-md) - 8px);
        gap: 6px;
        color: var(--text-secondary);
        font-family: var(--font-family);
        font-size: var(--font-size-xs);
    }

    label {
        display: flex;
        margin-bottom: 4px;
        align-items: center;
        gap: 5px;
        white-space: nowrap;
        cursor: pointer;
    }

    input {
        margin: 0;
        cursor: pointer;
    }

    .scale {
        display: flex;
        width: 100px;
        justify-content: flex-end;
    }

    .scale :global(.maplibregl-ctrl-scale) {
        margin: 0;
    }

    .attribution {
        min-width: 24px;
        width: max-content;
        max-width: calc(100vw - 20px);
    }

    .attribution :global(.maplibregl-ctrl-attrib) {
        margin: 0;
    }


    @media (max-width: 768px) {
        .map-display { display: none; }
    }
</style>
