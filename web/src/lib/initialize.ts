import { initMap } from './map';
import { initGridLayer } from './visualization';

export async function initialize() {
  initMap();
  initGridLayer();
}