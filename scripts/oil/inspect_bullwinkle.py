from adios_db.scripting import Oil
from adios_db.computation import physical_properties as pp
import json

with open("web/src/lib/assets/oils/oil/GN/GN00008.json", encoding="utf-8") as f:
    raw = json.load(f)

oil = Oil.from_py_json(raw)

value = pp.bullwinkle_fraction(oil)

print("value:", repr(value))
print("type:", type(value))