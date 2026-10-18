<script lang="ts">
    import "./app.css";
    import { onMount } from "svelte";
    import { initialize } from "$lib/initialize";
    import Sidebar from "./components/Sidebar.svelte";
    import MapView from "./components/MapView.svelte";
    import VisualizationToggles from "./components/VisualizationToggles.svelte";
    import Stats from "./components/Stats.svelte";
    import Legend from "./components/Legend.svelte";
    import Timeline from "./components/Timeline.svelte";
    import CurrentDay from "./components/CurrentDay.svelte";
    import CoordinateDisplay from "./components/CoordinateDisplay.svelte";
    import Toast from "./components/Toast.svelte";
    import { simulation, visualization } from "$lib/stores/index.svelte";
    import { setMarkerVisible } from "$lib/map";

    $effect(() => {
        const active = simulation.simulationActive;
        const marker = visualization.currentMarker;
        if (marker) {
            setMarkerVisible(!active);
        }
    });
    onMount(async () => {
        await initialize();
    });
</script>

<div class="app">
    <MapView />
    <Sidebar />
    <VisualizationToggles />
    <Stats />
    <Legend />
    <Timeline />
    <CurrentDay />
    <CoordinateDisplay />
    <Toast />
</div>

<style>
    .app {
        display: flex;
        height: 100vh;
        width: 100vw;
        overflow: hidden;
        position: relative;
        background: var(--bg-primary);
    }
</style>
