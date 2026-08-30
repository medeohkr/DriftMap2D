import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

export default defineConfig({
  plugins: [svelte()],
  optimizeDeps: {
    exclude: ['maplibre-gl'],
  },
  resolve: {
      alias: {
          '@': path.resolve(__dirname, './src'),
          '$lib': path.resolve(__dirname, './src/lib'),
      },
  },
  server: {
    port: 8080,
  },
});