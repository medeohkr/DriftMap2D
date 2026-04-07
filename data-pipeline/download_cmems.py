import copernicusmarine
from datetime import datetime

copernicusmarine.login()

DEPTHS = [0.49402499198913574, 40.344051361083984, 92.3260726928711, 318.1274108886719,]
DATE_TIME_START = datetime(2011, 1, 1)
DATE_TIME_END = datetime(2013, 12, 31)

current_date = DATE_TIME_START

while current_date <= DATE_TIME_END:
    # End of month
    if current_date.month == 12:
        end_date = datetime(current_date.year + 1, 1, 1)
    else:
        end_date = datetime(current_date.year, current_date.month + 1, 1)
    
    start_str = current_date.strftime("%Y-%m-%dT%H:%M:%S")
    end_str = end_date.strftime("%Y-%m-%dT%H:%M:%S")
    
    for depth in DEPTHS:
        print(f"Downloading: {start_str} to {end_str}, depth {depth:.0f}m")
        
        copernicusmarine.subset(
            dataset_id="cmems_mod_glo_phy_my_0.083deg_P1D-m",
            variables=["vo", "uo"],
            minimum_longitude=-180,
            maximum_longitude=179.9166717529297,
            minimum_latitude=-80,
            maximum_latitude=90,
            start_datetime=start_str,
            end_datetime=end_str,
            minimum_depth=depth,
            maximum_depth=depth,
            output_directory="glorys_3yr_global",
            output_filename=f"glorys_{current_date.strftime('%Y%m')}_depth_{depth:.0f}.nc"
        )
    
    current_date = end_date

print("All downloads finished!")