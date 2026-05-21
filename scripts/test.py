#!/usr/bin/env python3
"""
Python equivalent of the Rust parse_tile_data function with detailed anomaly detection.
Parses tile files with current data (24 hours, multiple depths) and optional wind/SST data.
"""

import sys
import struct
import numpy as np
from pathlib import Path


def parse_tile_data(bytes_data):
    """
    Exact Python equivalent of Rust's parse_tile_data function.
    Returns a dictionary with all parsed data.
    """
    if len(bytes_data) < 12:
        raise ValueError("File too short for header")
    
    # Parse header
    n_lon = struct.unpack('<I', bytes_data[0:4])[0]
    n_lat = struct.unpack('<I', bytes_data[4:8])[0]
    n_depths = struct.unpack('<I', bytes_data[8:12])[0]
    
    # Parse all depth values
    depths = []
    offset = 12
    for _ in range(n_depths):
        depth_val = struct.unpack('<f', bytes_data[offset:offset+4])[0]
        depths.append(depth_val)
        offset += 4
    
    n_cells = n_lon * n_lat
    n_hours = 24
    
    # Parse current data (u and v) for all hours and depths
    u = []
    v = []
    
    for _ in range(n_hours):
        for _ in range(n_depths):
            # Read u component
            u_f16 = bytes_data[offset:offset + n_cells * 2]
            offset += n_cells * 2
            # Convert f16 to f32
            u_array = np.frombuffer(u_f16, dtype=np.float16)
            u.extend(u_array.astype(np.float32).tolist())
            
            # Read v component
            v_f16 = bytes_data[offset:offset + n_cells * 2]
            offset += n_cells * 2
            # Convert f16 to f32
            v_array = np.frombuffer(v_f16, dtype=np.float16)
            v.extend(v_array.astype(np.float32).tolist())
    
    # Check if wind data is present (need at least 12 bytes for wind header)
    if offset + 12 > len(bytes_data):
        # No wind data — return with empty wind/SST vectors
        return {
            'u': u, 'v': v,
            'u_wind': [], 'v_wind': [], 'sst': [],
            'n_lon': n_lon, 'n_lat': n_lat,
            'n_lon_wind': 0, 'n_lat_wind': 0,
            'depths': depths, 'n_hours': n_hours, 'n_steps': 0,
        }
    
    # Parse wind header
    n_lon_wind = struct.unpack('<I', bytes_data[offset:offset+4])[0]
    n_lat_wind = struct.unpack('<I', bytes_data[offset+4:offset+8])[0]
    n_steps = struct.unpack('<I', bytes_data[offset+8:offset+12])[0]
    print(f"Wind grid: {n_lon_wind}x{n_lat_wind}, {n_steps} steps")
    offset += 12
    
    n_cells_wind = n_lon_wind * n_lat_wind
    wind_bytes_needed = n_steps * n_cells_wind * 2 * 3  # u + v + sst, 2 bytes each
    
    # Check if we have enough bytes for wind data
    if offset + wind_bytes_needed > len(bytes_data):
        # Incomplete wind data — return without wind
        return {
            'u': u, 'v': v,
            'u_wind': [], 'v_wind': [], 'sst': [],
            'n_lon': n_lon, 'n_lat': n_lat,
            'n_lon_wind': 0, 'n_lat_wind': 0,
            'depths': depths, 'n_hours': n_hours, 'n_steps': 0,
        }
    
    # Parse wind and SST data
    u_wind = []
    v_wind = []
    sst = []
    
    for _ in range(n_steps):
        # Read u_wind
        u_wind_f16 = bytes_data[offset:offset + n_cells_wind * 2]
        offset += n_cells_wind * 2
        u_wind_array = np.frombuffer(u_wind_f16, dtype=np.float16)
        u_wind.extend(u_wind_array.astype(np.float32).tolist())
        
        # Read v_wind
        v_wind_f16 = bytes_data[offset:offset + n_cells_wind * 2]
        offset += n_cells_wind * 2
        v_wind_array = np.frombuffer(v_wind_f16, dtype=np.float16)
        v_wind.extend(v_wind_array.astype(np.float32).tolist())
        
        # Read sst
        sst_f16 = bytes_data[offset:offset + n_cells_wind * 2]
        offset += n_cells_wind * 2
        sst_array = np.frombuffer(sst_f16, dtype=np.float16)
        sst.extend(sst_array.astype(np.float32).tolist())
    
    return {
        'u': u, 'v': v,
        'u_wind': u_wind, 'v_wind': v_wind, 'sst': sst,
        'n_lon': n_lon, 'n_lat': n_lat,
        'n_lon_wind': n_lon_wind, 'n_lat_wind': n_lat_wind,
        'depths': depths, 'n_hours': n_hours, 'n_steps': n_steps,
    }


def find_anomalies(data_array, grid_shape, var_name, min_val, max_val, steps, cells_per_step):
    """
    Find and report exact locations of anomalous values in a multi-dimensional grid.
    
    Args:
        data_array: 1D array of values
        grid_shape: tuple (n_lon, n_lat) for a single step
        var_name: name of the variable (for reporting)
        min_val: minimum acceptable value
        max_val: maximum acceptable value
        steps: number of time steps
        cells_per_step: number of grid cells per time step
    """
    data = np.array(data_array)
    n_lon, n_lat = grid_shape
    
    # Find indices of anomalous values
    anomalous_mask = (data < min_val) | (data > max_val)
    anomalous_indices = np.where(anomalous_mask)[0]
    
    if len(anomalous_indices) == 0:
        print(f"  ✅ {var_name}: No anomalous values detected (all within [{min_val}, {max_val}])")
        return
    
    print(f"  ⚠️ {var_name}: {len(anomalous_indices)} anomalous values detected!")
    print(f"     Acceptable range: [{min_val}, {max_val}]")
    print(f"     Found range: [{np.min(data[anomalous_mask]):.4f}, {np.max(data[anomalous_mask]):.4f}]")
    print(f"     Locations (step, lon_idx, lat_idx):")
    
    # Show first 20 anomalies with their locations
    for i, flat_idx in enumerate(anomalous_indices[:20]):
        step = flat_idx // cells_per_step
        cell_idx = flat_idx % cells_per_step
        lon_idx = cell_idx % n_lon
        lat_idx = cell_idx // n_lon
        
        print(f"       Step {step:3d}, Lon[{lon_idx:3d}], Lat[{lat_idx:3d}] = {data[flat_idx]:.4f}")
    
    if len(anomalous_indices) > 20:
        print(f"       ... and {len(anomalous_indices) - 20} more")
    
    # Summary by time step
    print(f"     Anomalies per time step:")
    for step in range(steps):
        step_start = step * cells_per_step
        step_end = step_start + cells_per_step
        step_anomalies = np.sum(anomalous_mask[step_start:step_end])
        if step_anomalies > 0:
            step_data = data[step_start:step_end]
            step_anom_data = step_data[anomalous_mask[step_start:step_end]]
            print(f"       Step {step:3d}: {step_anomalies:5d} anomalies (min: {np.min(step_anom_data):.4f}, max: {np.max(step_anom_data):.4f})")


def inspect_tile(tile_path):
    """Inspect a tile file using the exact same logic as the Rust parser."""
    with open(tile_path, 'rb') as f:
        data = f.read()
    
    print(f"File size: {len(data)} bytes")
    
    # Parse using the Rust-equivalent function
    result = parse_tile_data(data)
    
    print(f"\n--- Header ---")
    print(f"  n_lon (current grid): {result['n_lon']}")
    print(f"  n_lat (current grid): {result['n_lat']}")
    print(f"  n_depths: {len(result['depths'])}")
    print(f"  depths: {result['depths']}")
    
    n_cells = result['n_lon'] * result['n_lat']
    n_hours = result['n_hours']
    expected_current_size = n_hours * len(result['depths']) * n_cells * 2 * 2  # u+v, 2 bytes each
    
    print(f"\n--- Current Data ---")
    print(f"  Cells per layer: {n_cells}")
    print(f"  Hours: {n_hours}")
    print(f"  Depths: {len(result['depths'])}")
    print(f"  Expected current data size: {expected_current_size} bytes")
    
    if result['u']:
        u_array = np.array(result['u'])
        v_array = np.array(result['v'])
        print(f"  U data points: {len(u_array)}")
        print(f"  U range: {np.min(u_array):.4f} to {np.max(u_array):.4f}")
        print(f"  U NaN count: {np.sum(np.isnan(u_array))}")
        print(f"  V data points: {len(v_array)}")
        print(f"  V range: {np.min(v_array):.4f} to {np.max(v_array):.4f}")
        print(f"  V NaN count: {np.sum(np.isnan(v_array))}")
        
        # Check current data for anomalies (currents shouldn't exceed ~5 m/s typically)
        current_grid_shape = (result['n_lon'], result['n_lat'])
        current_cells_per_step = n_cells
        print(f"\n  Current U anomalies (threshold: ±5 m/s):")
        find_anomalies(result['u'], current_grid_shape, "U_current", -5.0, 5.0, 
                      n_hours * len(result['depths']), current_cells_per_step)
        print(f"\n  Current V anomalies (threshold: ±5 m/s):")
        find_anomalies(result['v'], current_grid_shape, "V_current", -5.0, 5.0,
                      n_hours * len(result['depths']), current_cells_per_step)
    
    print(f"\n--- Wind Data ---")
    print(f"  n_lon_wind: {result['n_lon_wind']}")
    print(f"  n_lat_wind: {result['n_lat_wind']}")
    print(f"  n_steps: {result['n_steps']}")
    
    if result['u_wind']:
        u_wind_array = np.array(result['u_wind'])
        v_wind_array = np.array(result['v_wind'])
        sst_array = np.array(result['sst'])
        
        wind_grid_shape = (result['n_lon_wind'], result['n_lat_wind'])
        wind_cells_per_step = result['n_lon_wind'] * result['n_lat_wind']
        n_wind_steps = result['n_steps']
        
        print(f"  Wind U shape: {u_wind_array.shape}")
        print(f"  Wind U range: {np.min(u_wind_array):.4f} to {np.max(u_wind_array):.4f}")
        print(f"  Wind U NaN count: {np.sum(np.isnan(u_wind_array))}")
        print(f"  Wind V range: {np.min(v_wind_array):.4f} to {np.max(v_wind_array):.4f}")
        print(f"  Wind V NaN count: {np.sum(np.isnan(v_wind_array))}")
        
        # Find wind anomalies (> ±50 m/s)
        print(f"\n  Wind U anomalies (threshold: ±50 m/s):")
        find_anomalies(result['u_wind'], wind_grid_shape, "U_wind", -50.0, 50.0,
                      n_wind_steps, wind_cells_per_step)
        print(f"\n  Wind V anomalies (threshold: ±50 m/s):")
        find_anomalies(result['v_wind'], wind_grid_shape, "V_wind", -50.0, 50.0,
                      n_wind_steps, wind_cells_per_step)
        
        print(f"\n--- SST Data ---")
        print(f"  SST shape: {sst_array.shape}")
        print(f"  SST range: {np.min(sst_array):.2f} to {np.max(sst_array):.2f} K")
        print(f"  SST NaN count: {np.sum(np.isnan(sst_array))}")
        
        # Find SST anomalies (unrealistic temperatures)
        print(f"\n  SST anomalies (threshold: 250K to 320K):")
        find_anomalies(result['sst'], wind_grid_shape, "SST", 250.0, 320.0,
                      n_wind_steps, wind_cells_per_step)
    
    # Calculate total expected size
    header_size = 12 + len(result['depths']) * 4
    current_data_size = n_hours * len(result['depths']) * n_cells * 2 * 2
    wind_header_size = 12 if result['n_steps'] > 0 else 0
    wind_data_size = result['n_steps'] * result['n_lon_wind'] * result['n_lat_wind'] * 2 * 3 if result['n_steps'] > 0 else 0
    total_expected = header_size + current_data_size + wind_header_size + wind_data_size
    
    print(f"\n--- Summary ---")
    print(f"  File size: {len(data)} bytes")
    print(f"  Expected total: {total_expected} bytes")
    print(f"  Header: {header_size}")
    print(f"  Current data: {current_data_size}")
    print(f"  Wind header: {wind_header_size}")
    print(f"  Wind+SST data: {wind_data_size}")
    
    if len(data) != total_expected:
        print(f"  ⚠️ WARNING: File size mismatch! Expected {total_expected}, got {len(data)}")
    else:
        print(f"  ✅ File structure appears correct")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python script.py <tile_file>")
        sys.exit(1)
    
    path = sys.argv[1]
    inspect_tile(path)