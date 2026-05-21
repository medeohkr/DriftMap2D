import geopandas as gpd
from shapely.ops import unary_union, orient
from shapely.validation import make_valid
import numpy as np
import regionmask
import xarray as xr

def clean_geometry(geom):
    """Fix invalid geometries before merging."""
    # Make valid first (handles self-intersections)
    geom = make_valid(geom)
    
    # Ensure polygon orientation (exterior CCW, interior CW)
    if geom.geom_type == 'Polygon':
        geom = orient(geom, sign=1.0)
    elif geom.geom_type == 'MultiPolygon':
        polys = [orient(p, sign=1.0) for p in geom.geoms]
        from shapely.geometry import MultiPolygon
        geom = MultiPolygon(polys)
    
    return geom

def generate_mask(shp_path, output_path, resolution_deg=1/120):
    print("Reading shapefile...")
    gdf = gpd.read_file(shp_path)
    print(f"Original polygon count: {len(gdf)}")
    
    # Filter to only land polygons (GSHHS level 1)
    # 'level' = 1 means land. Other levels are lakes, islands, etc.
    if 'level' in gdf.columns:
        gdf = gdf[gdf['level'] == 1]
        print(f"Land polygons only: {len(gdf)}")
    
    # Clean each geometry
    print("Cleaning geometries...")
    gdf['geometry'] = gdf['geometry'].apply(clean_geometry)
    
    # Remove any remaining invalid or empty geometries
    gdf = gdf[gdf.is_valid & ~gdf.is_empty]
    print(f"Valid polygons after cleaning: {len(gdf)}")
    
    # Merge in chunks to avoid memory blowup
    print("Merging polygons (this may take a few minutes)...")
    chunk_size = 500
    chunks = []
    
    for i in range(0, len(gdf), chunk_size):
        chunk = gdf.iloc[i:i+chunk_size]
        merged = unary_union(chunk.geometry)
        chunks.append(merged)
        print(f"  Processed {min(i+chunk_size, len(gdf))}/{len(gdf)}")
    
    print("Final union of all chunks...")
    merged_geometry = unary_union(chunks)
    merged_geometry = clean_geometry(merged_geometry)
    
    # Create grid
    lon = np.arange(-180, 180, resolution_deg)
    lat = np.arange(-90, 90, resolution_deg)
    print(f"Grid size: {len(lon)} x {len(lat)} = {len(lon) * len(lat)} cells")
    
    # Rasterize
    print("Rasterizing...")
    gdf_single = gpd.GeoDataFrame({"geometry": [merged_geometry]}, crs="EPSG:4326")
    mask = regionmask.mask_geopandas(gdf_single, lon, lat)
    
    # 1 = land, 0 = ocean
    mask = xr.where(mask == 0.0, 1, 0).fillna(0).astype(np.uint8)
    
    # Save
    print(f"Saving {mask.values.nbytes / 1e9:.2f} GB to {output_path}")
    with open(output_path, "wb") as f:
        mask.values.tofile(f)
    
    print("Done!")

generate_mask(
    'GSHHS_shp/f/GSHHS_f_L1.shp',
    'mask.bin',
    resolution_deg=1/120
)