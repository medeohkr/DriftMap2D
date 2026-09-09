<script lang="ts">
    import { releaseConfig } from "$lib/stores.svelte";
    import { updateMarker } from "$lib/map";
    import { normalizeLongitude } from "$lib/utils";
    import trash from "$lib/assets/images/TrashCan.webp";
    import { untrack } from 'svelte'

    $effect(() => {
        const lon = releaseConfig.activeRelease.lon;
        const lat = releaseConfig.activeRelease.lat;
        untrack(() => {
            updateMarker(lon, lat);
        });
    })

    function updateReleaseLat(e: Event) {
        const value = parseFloat((e.target as HTMLInputElement).value);
        if (!isNaN(value)) {
            releaseConfig.activeRelease.lat = value;
        }
    }

    function updateReleaseLon(e: Event) {
        const value = parseFloat((e.target as HTMLInputElement).value);
        if (!isNaN(value)) {
            releaseConfig.activeRelease.lon = value;
        }
    }
</script>

<div class="floating-container">
    <span>
        Release Settings
        {#if releaseConfig.releases.length > 1}
            &nbsp; — &nbsp;Editing Release {releaseConfig.activeReleaseIndex +
                1}/{releaseConfig.releases.length}
        {/if}
    </span>
    <div class="box-container">
        <div class="container-secondary">
            <span class="release-text">Latitude</span>
            <input
                onblur={updateReleaseLat}
                type="number"
                class="field-primary"
                value={releaseConfig.activeRelease.lat.toFixed(2)}
                step="any"
            />
            <span class="unit-text">° N</span>
        </div>
        <div class="container-secondary">
            <span class="release-text">Longitude</span>
            <input
                onblur={updateReleaseLon}
                type="number"
                class="field-primary"
                value={normalizeLongitude(
                    releaseConfig.activeRelease.lon,
                ).toFixed(2)}
                step="any"
            />
            <span class="unit-text">° E</span>
        </div>
        <div class="container-secondary">
            <span class="release-radius-text">Radius</span>
            <input
                bind:value={releaseConfig.activeRelease.radius}
                type="number"
                class="field-primary"
                step="any"
            />
            <span class="unit-text">km</span>
        </div>
    </div>
</div>
<div class="floating-container">
    <span>Schedule</span>

    <div class="inline-rows-container">
    {#each releaseConfig.activeRelease.schedule as interval, index}
        <div
            class="inline-container"
            class:light={index % 2 == 0}
        >
            <span style="width: 34px; white-space: nowrap"
                >Interval {index + 1}:
            </span>
            <div class="interval-container">
                <input
                    bind:value={interval.amount}
                    type="number"
                    class="field-primary transparent interval-field"
                    step="any"
                    size="3"
                    oninput={(e) =>
                        (e.currentTarget.size = Math.max(
                            1,
                            e.currentTarget.value.length,
                        ))}
                />
                <span>tons &nbsp;for</span>
                <input
                    bind:value={interval.duration}
                    type="number"
                    class="field-primary transparent interval-field"
                    class:dark={index % 2 != 0}
                    step="any"
                    size="2"
                    oninput={(e) =>
                        (e.currentTarget.size = Math.max(
                            1,
                            e.currentTarget.value.length,
                        ))}
                />
                <span class="interval-text">hours</span>
            </div>
            {#if releaseConfig.activeRelease.schedule.length > 1}
                <button
                    style="background-color: transparent; border: none; cursor: pointer"
                    onclick={(e) => {
                        releaseConfig.removeInterval(index);
                    }}
                >
                    <img src={trash} alt="Remove Release" class="trash-logo" />
                </button>
            {/if}
        </div>
    {/each}
    </div>
    <button class="interval-btn" onclick={releaseConfig.addInterval}>
        + &nbsp;Add Interval
    </button>
</div>
<details class="floating-container">
    <summary style="cursor: pointer">Manage Releases ({releaseConfig.releases.length})</summary>
        <div class="inline-rows-container">
        {#each releaseConfig.releases as release, index}
            <div
                class="inline-container"
                class:light={index % 2 == 0}
                onclick={() => (releaseConfig.activeReleaseIndex = index)}
                role="button"
                tabindex="0"
                onkeydown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        releaseConfig.activeReleaseIndex = index;
                    }
                }}
            >
                <div class="releases-container">
                    <input
                        type="radio"
                        class="release-toggle"
                        onclick={(e) => {
                            releaseConfig.activeReleaseIndex = index;
                        }}
                        checked={index == releaseConfig.activeReleaseIndex}
                    />
                    <span style="width: 44px; white-space: nowrap">
                        Release {index + 1}&nbsp;:
                    </span>
                </div>
                <span class="interval-text">{release.lat.toFixed(2)}° N, {release.lon.toFixed(2)}° E</span>
                {#if releaseConfig.releases.length > 1}
                    <button
                        style="background-color: transparent; border: none; cursor: pointer"
                        onclick={(e) => {
                            e.stopPropagation();
                            releaseConfig.removeRelease(index);
                        }}
                    >
                        <img src={trash} alt="Remove Release" class="trash-logo" />
                    </button>
                {/if}
            </div>
        {/each}
        </div>
    <button class="release-btn" onclick={releaseConfig.addRelease}>
        + &nbsp;Add Release
    </button>
</details>

<style>
    .interval-btn,
    .release-btn {
        background: none;
        border: none;
        color: var(--text-secondary);
        font-family: var(--font-family);
        font-weight: var(--weight-secondary);
        font-size: var(--font-size-xs);
        cursor: pointer;
        transition: opacity var(--transition-fast);
    }

    .release-btn {
        width: 100%;
        margin-top: var(--spacing-xs);
    }

    .interval-btn:hover,
    .release-btn:hover,
    .release-toggle:hover {
        opacity: 0.6;
    }

    .release-toggle {
        appearance: none;
        width: 12px;
        height: 12px;
        background-color: var(--text-muted);
        border-radius: var(--border-xl);
        transition: opacity var(--transition-fast);
        cursor: pointer;
    }

    .release-toggle:checked {
        border: var(--border-md) solid var(--text-primary);
    }

    .releases-container {
        display: flex;
        column-gap: var(--spacing-sm);
    }

    .trash-logo {
        width: 9px;
        opacity: 0.4;
    }

    .interval-field {
        width: auto;
        box-sizing: content-box;
        padding: 0;
        margin: 0;
    }

    .interval-container {
        display: flex;
        align-items: center;
        min-width: 0;
    }

    .interval-text {
        display: inline-block;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        padding-right: var(--spacing-xxs);
        min-width: 0;
    }

    .inline-rows-container {
        display: flex;
        flex-direction: column;
        row-gap: var(--spacing-xs);
    }
</style>
