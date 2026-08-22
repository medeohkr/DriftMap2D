// main.ts (Svelte 5)
import { mount } from 'svelte';
import App from './App.svelte';

const app = mount(App, {
  target: document.getElementById('app')!,
  props: { /* optional props */ }
});

export default app;