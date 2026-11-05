import { mount } from 'svelte';
import App from './App.svelte';
import { getCentroidPosition } from './lib/utils';

Object.assign(window, { getCentroidPosition });

const app = mount(App, {
  target: document.getElementById('app')!,
  props: { }
});

export default app;
