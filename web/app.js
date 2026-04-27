
// map init 
var map = new maplibregl.Map({
    container: 'map',
    style: {
        version: 8,
        sources: {
            'carto-dark': {
                type: 'raster',
                tiles: [
                    'https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png',
                    'https://b.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png',
                    'https://c.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png',
                    'https://d.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png'
                ],
                tileSize: 256,
                attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> &copy; <a href="https://carto.com/attributions">CARTO</a>'
            }
        },
        layers: [{
            id: 'carto-dark-layer',
            type: 'raster',
            source: 'carto-dark',
            minzoom: 0,
            maxzoom: 22
        }]
    },
    center: [165, 25],
    zoom: 3
});

// zoom buttons 
map.addControl(new maplibregl.NavigationControl({
    showCompass: false,
    showZoom: true
}),'top-right');

// scale bar 
map.addControl(new maplibregl.ScaleControl(), 'bottom-right');

// lon/lat indicator
map.on('click', function(e) {
    // gets lon and lat values 
    var lon = e.lngLat.lng.toFixed(2);
    var lat = e.lngLat.lat.toFixed(2);

    if (lat <= -80) {
        document.querySelector('.lat-field').value = `No Data`;
    } else{
        document.querySelector('.lat-field').value = `${lat}`;
        document.querySelector('.lon-field').value = `${lon}`;
    }
    
if (window.currentMarker) {
        window.currentMarker.remove();
    }
    
    // Create new marker
    window.currentMarker = new maplibregl.Marker({
        color: '#244886',
        scale: 0.9
    })
    .setLngLat([lon, lat])
    .addTo(map);
});