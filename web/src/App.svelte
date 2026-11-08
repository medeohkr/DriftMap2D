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
    let appElement: HTMLDivElement;
    let uiScale = $state(1);
    let uiHeight = $state(0);
    let sidebarHeight = $state<number | null>(null);
    let sidebarBottom = $state<number | null>(null);

    $effect(() => {
        const observer = new ResizeObserver(measureSidebar);
        let cancelled = false;
        async function measureSidebar() {
            if (!appElement) return;
            const viewport = appElement.getBoundingClientRect();
            uiScale = Math.min(1, Math.max(1, viewport.width) / 768);
            uiHeight = viewport.height / uiScale;
            await tick();
            if (cancelled) return;
            const sidebar = document.querySelector('.sidebar');
            const playback = document.getElementById('timeline-container');
            if (viewport.width > 768 || !sidebar || !playback) {
                sidebarHeight = null;
                sidebarBottom = null;
                return;
            }
            // browser rectangles are scaled pixels while overlay styles use logical pixels
            const top = sidebar.getBoundingClientRect().top;
            const bottom = playback.getBoundingClientRect().top - 24 * uiScale;
            sidebarHeight = Math.max(0, (bottom - top) / uiScale);
            sidebarBottom = Math.max(0, (viewport.bottom - bottom) / uiScale);
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

<div class="app" bind:this={appElement}>
    <MapView />
    <div class="map-ui"
        style:--ui-scale={uiScale}
        style:--ui-height={uiHeight ? `${uiHeight}px` : undefined}
        style:--mobile-sidebar-height={sidebarHeight === null ? undefined : `${sidebarHeight}px`}
        style:--mobile-sidebar-bottom={sidebarBottom === null ? undefined : `${sidebarBottom}px`}
    >
        <Sidebar />
        <VisualizationToggles />
        <Stats />
        <Legend />
        <Timeline />
        <CurrentDay />
        <Toast />
    </div>
</div>

<style>
    .app {
        display: flex;
        height: 100vh;
        height: 100dvh;
        width: 100vw;
        overflow: hidden;
        position: relative;
        background: var(--bg-primary);
    }

    .map-ui {
        position: absolute;
        inset: 0;
        width: 100%;
        height: var(--ui-height, 100%);
        pointer-events: none;
        -webkit-text-size-adjust: 100%;
        text-size-adjust: 100%;
        transform-origin: top left;
        transform: scale(var(--ui-scale, 1));
    }

    .map-ui > :global(*) { pointer-events: auto; }

    @media (max-width: 768px) {
        .map-ui {
            --mobile-sidebar-top: 102px;
            --ui-width: 768px;
            --sidebar-scale: max(1, calc(0.56 / var(--ui-scale, 1)));
            --width-sidebar: calc(300px * var(--sidebar-scale));
            width: var(--ui-width);
        }
    }
</style>
