import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

export default defineConfig({
  plugins: [svelte()],
  optimizeDeps: {
    esbuildOptions: {
      target: "es2022",
    },
  },
  build: {
    target: ["es2022", "safari15"],
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