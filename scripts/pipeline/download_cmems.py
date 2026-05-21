import copernicusmarine
from datetime import datetime, timedelta, date

copernicusmarine.login()

DEPTHS = [0.49402499198913574]
TODAY = datetime.now()
START_DAY = datetime(TODAY.year, TODAY.month, TODAY.day) - timedelta(days=30)
END_DAY = datetime(TODAY.year, TODAY.month, TODAY.day) + timedelta(days=9, hours=23)

# current_date = START_DAY

# while current_date <= END_DAY:
#     # End of month
#     if TODAY.month == 12:
#         end_date = datetime(TODAY.year + 1, 1, 1) - timedelta(hours=1)
#     elif int((datetime(TODAY.year, TODAY.month + 1, 1)-timedelta(hours=1)).timestamp()) > int(END_DAY.timestamp()):
#         end_date = END_DAY-timedelta(hours=1)
#     else:
#         end_date = datetime(TODAY.year, TODAY.month + 1, 1) - timedelta(hours=1)

start_str = START_DAY.strftime("%Y-%m-%dT%H:%M:%S")
end_str = END_DAY.strftime("%Y-%m-%dT%H:%M:%S")
today_str = TODAY.strftime("%Y-%m-%dT00:00:00")
    
for depth in DEPTHS:
    print(f"Downloading: {start_str} to {end_str}, depth {depth:.0f}m")
    
    copernicusmarine.subset(
        dataset_id="cmems_mod_glo_phy_anfc_merged-uv_PT1H-i",
        variables=["utotal", "vtotal"],
        minimum_longitude=-180,
        maximum_longitude=179.91668701171875,
        minimum_latitude=-80,
        maximum_latitude=90,
        start_datetime=today_str,
        end_datetime=today_str,
        minimum_depth=depth,
        maximum_depth=depth,
        output_directory="data/smoc_nc",
        output_filename=f"smoc_{date.today()}.nc"
    )

# current_date = end_date + timedelta(days=1)

print("All downloads finished!")