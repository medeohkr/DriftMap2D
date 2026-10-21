import numpy as np

R_KM = 6371.0088


def haversine_km(lon1, lat1, lon2, lat2):
    lon1, lat1, lon2, lat2 = map(np.radians, [lon1, lat1, lon2, lat2])
    dlon = lon2 - lon1
    dlat = lat2 - lat1
    a = np.sin(dlat / 2.0) ** 2 + np.cos(lat1) * np.cos(lat2) * np.sin(dlon / 2.0) ** 2
    return R_KM * 2.0 * np.arcsin(np.sqrt(a))


def cumulative_path_length(lon, lat):
    """Cumulative distance traveled. result[0] = 0."""
    n = len(lon)
    out = np.zeros(n)
    for i in range(1, n):
        out[i] = out[i - 1] + haversine_km(lon[i - 1], lat[i - 1], lon[i], lat[i])
    return out