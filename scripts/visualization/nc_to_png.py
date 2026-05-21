import xarray as xr
import numpy as np
import matplotlib.pyplot as plt
from PIL import Image
import io
import copernicusmarine
from datetime import datetime, timedelta, date

# copernicusmarine.login(".", ".")


TODAY = datetime.now()
today_str = TODAY.strftime("%Y-%m-%dT00:00:00")

# copernicusmarine.subset(
#     dataset_id="cmems_mod_glo_phy_anfc_merged-uv_PT1H-i",
#     variables=["utotal", "vtotal"],
#     minimum_longitude=-180,
#     maximum_longitude=179.91668701171875,
#     minimum_latitude=-80,
#     maximum_latitude=90,
#     start_datetime=today_str,
#     end_datetime=today_str,
#     minimum_depth=0.49402499198913574,
#     maximum_depth=0.49402499198913574,
#     output_directory="data/visualization",
#     output_filename=f"viz_{date.today()}.nc"
# )

# current_date = end_date + timedelta(days=1)
import datetime
import cartopy.crs as ccrs
import matplotlib.pyplot as plt
import numpy as np
import xarray as xr

# 1. Load data
ds = xr.open_dataset("data/visualization/viz_2026-05-13.nc")

# 2. Calculate speed
u = ds['utotal'].isel(time=0, depth=0)
v = ds['vtotal'].isel(time=0, depth=0)
speed = np.sqrt(u**2 + v**2)

# Extract raw geographic coordinates from NetCDF
lon = ds.longitude.values
lat = ds.latitude.values
lon_min, lon_max = float(lon.min()), float(lon.max())
lat_min, lat_max = float(lat.min()), float(lat.max())

# 3. Create Figure using Cartopy Web Mercator (EPSG:3857)
fig = plt.figure(figsize=(10, 8), facecolor='none')

# ccrs.Mercator.GOOGLE matches MapLibre's internal engine
ax = fig.add_subplot(1, 1, 1, projection=ccrs.Mercator.GOOGLE)

# --- MODERN SYNTAX TO STRIP BACKGROUNDS & BORDERS ---
ax.patch.set_visible(False)          # Replaces background_patch
ax.spines['geo'].set_visible(False)  # Replaces outline_patch
ax.axis('off')

# Set the map view limits strictly to your NetCDF bounding box
ax.set_extent([lon_min, lon_max, lat_min, lat_max], crs=ccrs.PlateCarree())

# Extract the actual pixel dimensions of your data array
data_height, data_width = speed.shape
print(f"Original Data Resolution: {data_width}x{data_height} pixels")

# Plot data forcing Cartopy to match your exact grid dimensions
im = ax.imshow(
    speed,
    extent=[lon_min, lon_max, lat_min, lat_max],
    transform=ccrs.PlateCarree(),
    cmap='viridis',
    vmin=0,
    vmax=1,
    origin='lower',
    interpolation='bilinear',
    interpolation_stage='data',
    regrid_shape=(data_height, data_width)  # <-- FORCE FULL RESOLUTION HERE
)


# 4. Save without margins or borders 
plt.subplots_adjust(left=0, right=1, bottom=0, top=1)

plt.savefig(
    'currents.png',
    dpi=150,
    transparent=True
)
plt.close(fig)

