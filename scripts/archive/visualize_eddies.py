#!/usr/bin/env python3
"""
Visualize daily eddy atlas changes over time.
Generates animations and comparison plots from daily eddy_YYYYMMDD.bin files.
"""

import numpy as np
import struct
import os
import json
from datetime import datetime, timedelta
import matplotlib.pyplot as plt
import matplotlib.animation as animation
from matplotlib.colors import LogNorm, Normalize
import cartopy.crs as ccrs
import cartopy.feature as cfeature
from glob import glob
import argparse

# ===== CONFIGURATION =====

DAILY_DIR = "data/eddy_atlas_global_DT/daily"
COORDS_FILE = "data/eddy_atlas_global_DT/eddy_coords.bin"
OUTPUT_DIR = "data/eddy_visualizations"

# Default region (global)
DEFAULT_LAT_RANGE = (-80, 90)
DEFAULT_LON_RANGE = (-180, 180)

os.makedirs(OUTPUT_DIR, exist_ok=True)


# ===== DATA LOADING =====

def load_coordinates(coords_path):
    """Load pre-computed grid coordinates."""
    with open(coords_path, 'rb') as f:
        version, n_lat, n_lon = struct.unpack('3i', f.read(12))
        lon_grid = np.frombuffer(f.read(n_lat * n_lon * 4), dtype=np.float32).reshape((n_lat, n_lon))
        lat_grid = np.frombuffer(f.read(n_lat * n_lon * 4), dtype=np.float32).reshape((n_lat, n_lon))
    return lon_grid, lat_grid, n_lat, n_lon


def load_daily_file(filepath, n_lat=2041, n_lon=4320):
    """Load a single daily eddy file."""
    n_cells = n_lat * n_lon
    expected_size = 16 + (n_cells * 8)  # header + 2 × float32 arrays
    
    with open(filepath, 'rb') as f:
        version, year, month, day = struct.unpack('4i', f.read(16))
        
        radius = np.frombuffer(f.read(n_cells * 4), dtype=np.float32)
        speed = np.frombuffer(f.read(n_cells * 4), dtype=np.float32)
        
        # Verify we got the right amount
        if len(radius) != n_cells or len(speed) != n_cells:
            # Try reverse-engineering from file size
            actual_size = os.path.getsize(filepath)
            actual_cells = (actual_size - 16) // 8
            raise ValueError(f"Expected {n_cells} cells, got {len(radius)}. "
                           f"File size suggests {actual_cells} cells.")
        
        radius = radius.reshape((n_lat, n_lon))
        speed = speed.reshape((n_lat, n_lon))
        
        return radius, speed, year, month, day

def get_available_dates(daily_dir):
    """Get sorted list of available dates from file names."""
    files = sorted(glob(os.path.join(daily_dir, "eddy_*.bin")))
    dates = []
    for f in files:
        basename = os.path.basename(f)
        date_str = basename.replace("eddy_", "").replace(".bin", "")
        try:
            dates.append(datetime.strptime(date_str, "%Y%m%d"))
        except ValueError:
            continue
    return dates, files


# ===== VISUALIZATION =====

def plot_single_day(date, radius, speed, lon_grid, lat_grid, output_path, 
                    lon_range=None, lat_range=None):
    """Plot radius and phase speed for a single day."""
    fig, axes = plt.subplots(1, 2, figsize=(16, 6), 
                            subplot_kw={'projection': ccrs.PlateCarree()})
    
    for ax, data, title, cmap, norm in zip(
        axes,
        [radius, speed],
        ['Eddy Radius (km)', 'Phase Speed (m/s)'],
        ['viridis', 'plasma'],
        [Normalize(vmin=0, vmax=200), Normalize(vmin=0, vmax=0.5)]
    ):
        im = ax.pcolormesh(lon_grid, lat_grid, data, cmap=cmap, norm=norm, 
                          transform=ccrs.PlateCarree(), rasterized=True)
        ax.add_feature(cfeature.LAND, facecolor='lightgray', edgecolor='none')
        ax.add_feature(cfeature.COASTLINE, linewidth=0.5)
        ax.set_title(title, fontsize=12)
        plt.colorbar(im, ax=ax, shrink=0.7, pad=0.05)
        
        if lon_range:
            ax.set_xlim(lon_range)
        if lat_range:
            ax.set_ylim(lat_range)
    
    fig.suptitle(f"AVISO Eddy Atlas — {date.strftime('%Y-%m-%d')}", fontsize=14, y=0.98)
    plt.tight_layout()
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.close()
    print(f"  Saved: {output_path}")


def create_animation(dates, files, lon_grid, lat_grid, output_path,
                    lon_range=None, lat_range=None, fps=5):
    """Create MP4 animation of eddy field evolution."""
    fig, axes = plt.subplots(1, 2, figsize=(14, 6),
                            subplot_kw={'projection': ccrs.PlateCarree()})
    
    # Load first frame for initialization
    radius0, speed0, _, _, _ = load_daily_file(files[0])
    
    im1 = axes[0].pcolormesh(lon_grid, lat_grid, radius0, cmap='viridis',
                             norm=Normalize(vmin=0, vmax=200),
                             transform=ccrs.PlateCarree(), rasterized=True)
    im2 = axes[1].pcolormesh(lon_grid, lat_grid, speed0, cmap='plasma',
                             norm=Normalize(vmin=0, vmax=0.5),
                             transform=ccrs.PlateCarree(), rasterized=True)
    
    for ax in axes:
        ax.add_feature(cfeature.LAND, facecolor='lightgray', edgecolor='none')
        ax.add_feature(cfeature.COASTLINE, linewidth=0.5)
        if lon_range:
            ax.set_xlim(lon_range)
        if lat_range:
            ax.set_ylim(lat_range)
    
    axes[0].set_title('Eddy Radius (km)')
    axes[1].set_title('Phase Speed (m/s)')
    
    title = fig.suptitle('', fontsize=14)
    plt.colorbar(im1, ax=axes[0], shrink=0.7)
    plt.colorbar(im2, ax=axes[1], shrink=0.7)
    plt.tight_layout()
    
    def update(frame_idx):
        radius, speed, year, month, day = load_daily_file(files[frame_idx])
        im1.set_array(radius.ravel())
        im2.set_array(speed.ravel())
        title.set_text(f"AVISO Eddy Atlas — {year}-{month:02d}-{day:02d}")
        return im1, im2, title
    
    ani = animation.FuncAnimation(fig, update, frames=len(files),
                                 interval=1000//fps, blit=False)
    
    writer = animation.FFMpegWriter(fps=fps, bitrate=2000)
    ani.save(output_path, writer=writer)
    plt.close()
    print(f"  Animation saved: {output_path}")


def plot_change_map(date1_idx, date2_idx, dates, files, lon_grid, lat_grid,
                    output_path, lon_range=None, lat_range=None):
    """Plot difference between two dates."""
    radius1, speed1, y1, m1, d1 = load_daily_file(files[date1_idx])
    radius2, speed2, y2, m2, d2 = load_daily_file(files[date2_idx])
    
    fig, axes = plt.subplots(2, 2, figsize=(16, 10),
                            subplot_kw={'projection': ccrs.PlateCarree()})
    
    plots = [
        (axes[0,0], radius2 - radius1, 'RdYlBu_r', 'Δ Radius (km)'),
        (axes[0,1], speed2 - speed1, 'RdBu_r', 'Δ Phase Speed (m/s)'),
        (axes[1,0], radius2, 'viridis', f'Radius {y2}-{m2:02d}-{d2:02d}'),
        (axes[1,1], speed2, 'plasma', f'Speed {y2}-{m2:02d}-{d2:02d}'),
    ]
    
    for ax, data, cmap, title in plots:
        vmax = np.percentile(np.abs(data[~np.isnan(data)]), 99) if 'Δ' in title else None
        norm = Normalize(vmin=-vmax, vmax=vmax) if 'Δ' in title else None
        
        im = ax.pcolormesh(lon_grid, lat_grid, data, cmap=cmap, norm=norm,
                          transform=ccrs.PlateCarree(), rasterized=True)
        ax.add_feature(cfeature.LAND, facecolor='lightgray', edgecolor='none')
        ax.add_feature(cfeature.COASTLINE, linewidth=0.5)
        ax.set_title(title)
        plt.colorbar(im, ax=ax, shrink=0.7)
        
        if lon_range:
            ax.set_xlim(lon_range)
        if lat_range:
            ax.set_ylim(lat_range)
    
    fig.suptitle(f"Change: {dates[date1_idx].strftime('%Y-%m-%d')} → "
                f"{dates[date2_idx].strftime('%Y-%m-%d')}", fontsize=14)
    plt.tight_layout()
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.close()


def plot_time_series(dates, files, lon_grid, lat_grid, output_path,
                     lat_range=None, lon_range=None):
    """Plot zonal mean statistics over time."""
    all_mean_radius = []
    all_mean_speed = []
    all_std_radius = []
    
    for f in files:
        radius, speed, _, _, _ = load_daily_file(f)
        
        # Subset region if specified
        if lat_range:
            lat_mask = (lon_grid[:, 0] >= lat_range[0]) & (lon_grid[:, 0] <= lat_range[1])
            radius = radius[lat_mask, :]
            speed = speed[lat_mask, :]
        
        all_mean_radius.append(np.nanmean(radius))
        all_mean_speed.append(np.nanmean(speed))
        all_std_radius.append(np.nanstd(radius))
    
    fig, axes = plt.subplots(2, 1, figsize=(12, 8))
    
    axes[0].plot(dates, all_mean_radius, 'b-', linewidth=1)
    axes[0].fill_between(dates, 
                         np.array(all_mean_radius) - np.array(all_std_radius),
                         np.array(all_mean_radius) + np.array(all_std_radius),
                         alpha=0.3)
    axes[0].set_ylabel('Mean Eddy Radius (km)')
    axes[0].grid(True, alpha=0.3)
    
    axes[1].plot(dates, all_mean_speed, 'r-', linewidth=1)
    axes[1].set_ylabel('Mean Phase Speed (m/s)')
    axes[1].set_xlabel('Date')
    axes[1].grid(True, alpha=0.3)
    
    region_str = f" ({lat_range[0]}°–{lat_range[1]}°)" if lat_range else " (Global)"
    fig.suptitle(f"Eddy Statistics Over Time{region_str}", fontsize=14)
    plt.tight_layout()
    plt.savefig(output_path, dpi=150)
    plt.close()


# ===== MAIN =====

def main():
    parser = argparse.ArgumentParser(description='Visualize daily eddy atlas data')
    parser.add_argument('--single', type=str, help='Plot single date (YYYYMMDD)')
    parser.add_argument('--animation', action='store_true', help='Create MP4 animation')
    parser.add_argument('--change', nargs=2, help='Plot change between two dates (YYYYMMDD YYYYMMDD)')
    parser.add_argument('--timeseries', action='store_true', help='Plot time series statistics')
    parser.add_argument('--fps', type=int, default=5, help='Animation frame rate')
    parser.add_argument('--region', type=str, help='Region name or "lat_min,lat_max,lon_min,lon_max"')
    
    args = parser.parse_args()
    
    if not any([args.single, args.animation, args.change, args.timeseries]):
        parser.print_help()
        return
    
    print("Loading coordinates...")
    lon_grid, lat_grid, n_lat, n_lon = load_coordinates(COORDS_FILE)
    
    dates, files = get_available_dates(DAILY_DIR)
    print(f"Found {len(dates)} daily files ({dates[0].date()} to {dates[-1].date()})")
    
    # Parse region
    lon_range, lat_range = None, None
    if args.region:
        if ',' in args.region:
            parts = [float(x) for x in args.region.split(',')]
            lat_range = (parts[0], parts[1])
            lon_range = (parts[2], parts[3])
        elif args.region == 'acc':
            lat_range = (-67, -30)
            lon_range = (-180, 180)
        elif args.region == 'gulf_stream':
            lat_range = (25, 45)
            lon_range = (-80, -50)
        elif args.region == 'kuroshio':
            lat_range = (25, 40)
            lon_range = (130, 160)
    
    if args.single:
        date = datetime.strptime(args.single, "%Y%m%d")
        idx = dates.index(date) if date in dates else 0
        radius, speed, y, m, d = load_daily_file(files[idx])
        output = os.path.join(OUTPUT_DIR, f"eddy_{args.single}.png")
        plot_single_day(datetime(y, m, d), radius, speed, lon_grid, lat_grid, 
                       output, lon_range, lat_range)
    
    if args.animation:
        subset = files  # Could subsample for shorter animations
        output = os.path.join(OUTPUT_DIR, "eddy_animation.mp4")
        create_animation(dates, subset, lon_grid, lat_grid, output,
                        lon_range, lat_range, args.fps)
    
    if args.change:
        date1, date2 = [datetime.strptime(d, "%Y%m%d") for d in args.change]
        idx1 = dates.index(date1)
        idx2 = dates.index(date2)
        output = os.path.join(OUTPUT_DIR, f"eddy_change_{args.change[0]}_{args.change[1]}.png")
        plot_change_map(idx1, idx2, dates, files, lon_grid, lat_grid, output,
                       lon_range, lat_range)
    
    if args.timeseries:
        output = os.path.join(OUTPUT_DIR, "eddy_timeseries.png")
        plot_time_series(dates, files, lon_grid, lat_grid, output, lat_range, lon_range)


if __name__ == "__main__":
    main()