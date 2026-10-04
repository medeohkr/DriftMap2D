import gzip

with open('web/src/lib/assets/oils/oil_catalog.json', 'rb') as f:
    data = f.read()

with gzip.open('oil_catalog.json.gz', 'wb', compresslevel=9) as f:
    f.write(data)