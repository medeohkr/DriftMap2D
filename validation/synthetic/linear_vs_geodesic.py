from pyproj import Geod
import numpy as np

g = Geod(ellps="WGS84")

# WGS84 radii at lat -75
from pyproj import Geod
lat = -75.0
a = 6378137.0
e2 = 6.69437999014e-3
sin_phi = np.sin(np.radians(lat))
N = a / np.sqrt(1 - e2 * sin_phi**2)
M = a * (1 - e2) / (1 - e2 * sin_phi**2)**1.5
R_parallel = N * np.cos(np.radians(lat))

# Linear update
u, v, dt = 0.5, 0.5, 3600.0
lon_lin = -175 + (u * dt) / R_parallel * (180 / np.pi)
lat_lin = -75 + (v * dt) / M * (180 / np.pi)

# Geodesic forward
speed = np.sqrt(u*u + v*v)
az = np.degrees(np.arctan2(u, v))
lon_geo, lat_geo, _ = g.fwd(-175, -75, az, speed * dt)

print(f"linear:   ({lon_lin:.10f}, {lat_lin:.10f})")
print(f"geodesic: ({lon_geo:.10f}, {lat_geo:.10f})")
print(f"Δlon: {(lon_lin - lon_geo) * R_parallel * np.pi/180:.3f} m")
print(f"Δlat: {(lat_lin - lat_geo) * M * np.pi/180:.3f} m")