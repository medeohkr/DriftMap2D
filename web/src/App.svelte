<script lang="ts">
  import { onMount } from 'svelte';
  import { initialize } from './lib/initialize';
  import Sidebar from './components/Sidebar.svelte';
  import MapView from './components/MapView.svelte';
  import VisualizationToggles from './components/VisualizationToggles.svelte';
  
  import './app.css';
  
  onMount(async () => {
    await initialize();
  });
</script>

<div class="app">
  <MapView />
  <Sidebar />
  <VisualizationToggles />
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