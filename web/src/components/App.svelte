<script lang="ts">
  import { onMount } from 'svelte';
  import '../app.css'
  onMount(() => {
    console.log('Svelte app mounted!');
  });
</script>

<h1>DriftMap</h1>
<div id="map"></div>

<style>
  #map {
    height: 100vh;
    width: 100%;
  }
</style>