<script lang="ts">
    import { config } from "$lib/stores/config.svelte";
    import { genericOverrides, objectOverrides, oilOverrides } from "$lib/stores/index.svelte";

</script>

<summary style="cursor: pointer; user-select: none;">Tracer Settings</summary>
{#if config.tracerType === "oil"}
    <div class="box-container">
        <div class="container-secondary">
            <span>Windage Factor (%)</span>
            <input
                type="number"
                class="field-primary"
                bind:value={oilOverrides.windFactor}
                step="any"
                min="0"
            />
        </div>
        <div class="container-secondary">
            <span>Deflection Scheme</span>
            <select bind:value={oilOverrides.deflectionScheme} class="scheme-selector">
                <option value="constant">Constant</option>
                <option value="samuels">Samuels</option>
            </select>
        </div>
        {#if oilOverrides.deflectionScheme === "constant"}
            <div class="container-secondary">
                <span>Deflection Angle (°)</span>
                <input
                    type="number"
                    class="field-primary"
                    bind:value={oilOverrides.windDeflection}
                    step="any"
                    min="0"
                />
            </div>
        {/if}
    </div>
    {#if oilOverrides.id.length > 0}
        <div class="box-container" style="margin-top: 12px;">
            <div class="container-secondary">
                <span>Emulsification Onset</span>
                <select bind:value={oilOverrides.emulOnset} class="scheme-selector">
                    <option value="time">Time</option>
                    <option value="fraction">Fraction</option>
                </select>
            </div>
            {#if oilOverrides.emulOnset === "time"}
                <div class="container-secondary">
                    <span>Time Elapsed (hours)</span>
                    <input
                        type="number"
                        class="field-primary"
                        bind:value={oilOverrides.bulltime}
                        step="any"
                        min="0"
                    />
                </div>
            {:else}
                <div class="container-secondary">
                    <span>Fraction Evaporated (%)</span>
                    <input
                        type="number"
                        class="field-primary"
                        bind:value={oilOverrides.bullwinkleFrac}
                        step="any"
                        min="0"
                    />
                </div>
            {/if}
        </div>
    {/if}
{:else if config.tracerType === "sar"}
    <div class="box-container">
        <div class="container-secondary">
            <span>Jibing Probability (%)</span>
            <input
                type="number"
                class="field-primary"
                bind:value={objectOverrides.jibeProbability}
                step="any"
                min="0"
            />
        </div>
        <div class="container-secondary">
            <span>Object Can Capsize</span>
            <select bind:value={objectOverrides.capsizing} class="scheme-selector">
                <option value={true}>True</option>
                <option value={false}>False</option>
            </select>
        </div>
    </div>
    {#if objectOverrides.capsizing}
        <div class="box-container" style="margin-top: 12px;">
            <div class="container-secondary">
                <span>Wind Threshold (m/s)</span>
                <input
                    type="number"
                    class="field-primary"
                    bind:value={objectOverrides.capsizeThreshold}
                    step="any"
                    min="0"
                />
            </div>
            <div class="container-secondary">
                <span>Capsize Fraction (%)</span>
                <input
                    type="number"
                    class="field-primary"
                    bind:value={objectOverrides.capsizeFraction}
                    step="any"
                    min="0"
                />
            </div>
            <div class="container-secondary">
                <span>Capsize Sigma</span>
                <input
                    type="number"
                    class="field-primary"
                    bind:value={objectOverrides.capsizeSigma}
                    step="any"
                    min="0"
                />
            </div>
        </div>

    {/if}
{:else}
    <div class="box-container">
        <div class="container-secondary">
            <span>Windage Factor (%)</span>
            <input
                type="number"
                class="field-primary"
                bind:value={genericOverrides.windFactor}
                step="any"
                min="0"
            />
        </div>
        <div class="container-secondary">
            <span>Deflection Angle (°)</span>
            <input
                type="number"
                class="field-primary"
                bind:value={genericOverrides.windDeflection}
                step="any"
                min="0"
            />
        </div>
    </div>
{/if}