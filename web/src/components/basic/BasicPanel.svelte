<script lang="ts">
    import ImportAutoZoom from './ImportAutoZoom.svelte';
    import Presets from './Presets.svelte';
    import ModelSelection from './ModelSelection.svelte';
    import SimulationTime from './SimulationTime.svelte';
    import ReleaseSettings from './ReleaseSettings.svelte';
    import SimulationButtons from './SimulationButtons.svelte';
</script>

<div class="sidebar-content">
    <div class="container-primary">
        <ImportAutoZoom />
        <Presets />
        <ModelSelection />
        <SimulationTime />
        <ReleaseSettings />
        <SimulationButtons />
    </div>
</div>