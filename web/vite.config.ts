import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  optimizeDeps: {
    exclude: ['maplibre-gl'],
  },
  server: {
    port: 8080,
  },
});