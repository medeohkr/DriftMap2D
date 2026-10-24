<script lang="ts">
    import "./app.css";
    import { onMount, untrack, tick } from "svelte";
    import { initialize } from "$lib/initialize";
    import Sidebar from "./components/Sidebar.svelte";
    import MapView from "./components/MapView.svelte";
    import VisualizationToggles from "./components/VisualizationToggles.svelte";
    import Stats from "./components/Stats.svelte";
    import Legend from "./components/Legend.svelte";
    import Timeline from "./components/Timeline.svelte";
    import CurrentDay from "./components/CurrentDay.svelte";
    import Toast from "./components/Toast.svelte";
    import { simulation, visualization, config } from "$lib/stores/index.svelte";
    import { setMarkerVisible, updateForcingCoverage } from "$lib/map";
    let sidebarHeight = $state<number | null>(null);
    let sidebarBottom = $state<number | null>(null);

    $effect(() => {
        const observer = new ResizeObserver(measureSidebar);
        let cancelled = false;
        function measureSidebar() {
            const sidebar = document.querySelector('.sidebar');
            const playback = document.getElementById('timeline-container');
            if (window.innerWidth > 768 || !sidebar || !playback) {
                sidebarHeight = null;
                sidebarBottom = null;
                return;
            }
            const top = sidebar.getBoundingClientRect().top;
            const bottom = playback.getBoundingClientRect().top - 24;
            sidebarHeight = Math.max(0, bottom - top);
            sidebarBottom = Math.max(0, window.innerHeight - bottom);
        }
        tick().then(() => {
            if (cancelled) return;
            for (const selector of ['.app', '#timeline-container']) {
                const element = document.querySelector(selector);
                if (element) observer.observe(element);
            }
            measureSidebar();
        });
        window.addEventListener('resize', measureSidebar);
        return () => {
            cancelled = true;
            observer.disconnect();
            window.removeEventListener('resize', measureSidebar);
        };
    });

    $effect(() => {
        config.forcingModel;
        simulation.simulationActive;
        untrack(updateForcingCoverage);
    });

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

<div class="app"
    style:--mobile-sidebar-height={sidebarHeight === null ? undefined : `${sidebarHeight}px`}
    style:--mobile-sidebar-bottom={sidebarBottom === null ? undefined : `${sidebarBottom}px`}
>
    <MapView />
    <Sidebar />
    <VisualizationToggles />
    <Stats />
    <Legend />
    <Timeline />
    <CurrentDay />
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

    @media (max-width: 768px) {
        .app { --mobile-sidebar-top: 102px; }
    }
</style>
