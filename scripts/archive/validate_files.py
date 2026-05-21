import struct
import numpy as np
from pathlib import Path
import sys

def read_tile(tile_path):
    """Read a tile file and return its data"""
    with open(tile_path, 'rb') as f:
        data = f.read()
    
    if len(data) < 12:
        return None, f"File too small: {len(data)} bytes"
    
    # Read header
    n_lon = struct.unpack('<I', data[0:4])[0]
    n_lat = struct.unpack('<I', data[4:8])[0]
    n_depths = struct.unpack('<I', data[8:12])[0]
    
    offset = 12
    
    # Read depths
    depths = []
    for _ in range(n_depths):
        if offset + 4 > len(data):
            return None, "Truncated depth values"
        depth = struct.unpack('<f', data[offset:offset+4])[0]
        depths.append(depth)
        offset += 4
    
    # Calculate data sizes
    n_cells = n_lon * n_lat
    data_bytes = n_cells * n_depths * 2  # 2 bytes per float16
    
    if offset + data_bytes * 2 > len(data):
        return None, f"Truncated: expected {offset + data_bytes * 2} bytes, got {len(data)}"
    
    # Read u data (float16)
    u_bytes = data[offset:offset + data_bytes]
    u = np.frombuffer(u_bytes, dtype=np.float16).astype(np.float32)
    
    offset += data_bytes
    
    # Read v data (float16)
    v_bytes = data[offset:offset + data_bytes]
    v = np.frombuffer(v_bytes, dtype=np.float16).astype(np.float32)
    
    # Reshape to (depth, lat, lon)
    u = u.reshape(n_depths, n_lat, n_lon)
    v = v.reshape(n_depths, n_lat, n_lon)
    
    return {
        'n_lon': n_lon,
        'n_lat': n_lat,
        'n_depths': n_depths,
        'depths': depths,
        'u': u,
        'v': v,
    }, None

def analyze_tile(tile_path):
    """Print statistics about a tile's velocity values"""
    data, err = read_tile(tile_path)
    
    if err:
        print(f"❌ ERROR: {err}")
        return False
    
    print(f"📁 {tile_path}")
    print(f"   Dimensions: {data['n_lon']} lon × {data['n_lat']} lat × {data['n_depths']} depth")
    print(f"   Depths: {data['depths']}")
    
    # Surface level only (depth=0)
    u_surface = data['u'][0]
    v_surface = data['v'][0]
    
    # Statistics
    u_mean = np.mean(u_surface)
    u_std = np.std(u_surface)
    u_min = np.min(u_surface)
    u_max = np.max(u_surface)
    u_abs_max = np.max(np.abs(u_surface))
    
    v_mean = np.mean(v_surface)
    v_std = np.std(v_surface)
    v_min = np.min(v_surface)
    v_max = np.max(v_surface)
    v_abs_max = np.max(np.abs(v_surface))
    
    speed = np.sqrt(u_surface**2 + v_surface**2)
    speed_mean = np.mean(speed)
    speed_max = np.max(speed)
    
    print(f"\n   📊 U (east-west) velocity (m/s):")
    print(f"      Mean: {u_mean:.4f}")
    print(f"      Std:  {u_std:.4f}")
    print(f"      Min:  {u_min:.4f}")
    print(f"      Max:  {u_max:.4f}")
    print(f"      |max|: {u_abs_max:.4f}")
    
    print(f"\n   📊 V (north-south) velocity (m/s):")
    print(f"      Mean: {v_mean:.4f}")
    print(f"      Std:  {v_std:.4f}")
    print(f"      Min:  {v_min:.4f}")
    print(f"      Max:  {v_max:.4f}")
    print(f"      |max|: {v_abs_max:.4f}")
    
    print(f"\n   🌊 Speed (m/s):")
    print(f"      Mean: {speed_mean:.4f}")
    print(f"      Max:  {speed_max:.4f}")
    
    # Sanity check
    is_reasonable = True
    issues = []
    
    if u_abs_max > 10 or v_abs_max > 10:
        issues.append(f"❌ Too high! {u_abs_max:.1f} m/s is unrealistic (ocean currents < 2 m/s)")
        is_reasonable = False
    elif u_abs_max < 0.001 and v_abs_max < 0.001:
        issues.append(f"⚠️ Too low! All velocities near zero")
        is_reasonable = False
    elif speed_mean < 0.01:
        issues.append(f"⚠️ Very slow: mean speed {speed_mean:.4f} m/s")
    elif speed_mean > 2.0:
        issues.append(f"⚠️ Very fast: mean speed {speed_mean:.1f} m/s")
    else:
        issues.append(f"✅ Reasonable: mean speed {speed_mean:.2f} m/s")
    
    print(f"\n   🔍 Sanity check:")
    for issue in issues:
        print(f"      {issue}")
    
    return is_reasonable

def find_tiles(root_dir, max_tiles=5):
    """Find tile files and analyze them"""
    root = Path(root_dir)
    tile_files = list(root.rglob('*.bin'))
    
    if not tile_files:
        print(f"No .bin files found in {root_dir}")
        return
    
    print(f"Found {len(tile_files)} tile files\n")
    print("=" * 60)
    
    analyzed = 0
    for tile_path in tile_files[:max_tiles]:
        analyze_tile(tile_path)
        print("\n" + "-" * 60 + "\n")
        analyzed += 1
    
    if len(tile_files) > max_tiles:
        print(f"(Showing {max_tiles} of {len(tile_files)} files)")

if __name__ == "__main__":
    # Change this to your tiles folder
    TILES_PATH = "D:\projects\driftmap\web\data/forecast_tiles"
    
    if len(sys.argv) > 1:
        TILES_PATH = sys.argv[1]
    
    if not Path(TILES_PATH).exists():
        print(f"❌ Path not found: {TILES_PATH}")
        print(f"Usage: python check_tiles.py /path/to/tiles")
        sys.exit(1)
    
    find_tiles(TILES_PATH)