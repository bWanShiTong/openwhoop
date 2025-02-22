# OpenWhoop

OpenWhoop is project that allows you to download heart rate data directly from your Whoop4.0 device without Whoop subscription or Whoops servers, making data your own.

### How to Run?

First you need to copy `.env.example` into `.env` and then scan until you find your Whoop device:
```sh
cp .env.example .env
cargo run -r -- scan
```

After you find your device, set the appropriate environment variable in `.env`:

#### Linux
Copy the device address to `.env` under `WHOOP_ADDR`:
```sh
WHOOP_ADDR=00:11:22:33:44:55
```

#### macOS
Copy the device name to `.env` under `WHOOP_NAME`:
```sh
WHOOP_NAME="WHOOP 4A0360692"
```

Then you can download data from your whoop by running:
```sh
cargo run -r -- download-history
```

### Importing data to python

If you want to import heart rate data into python you can use following code:
```py
import pandas as pd
import os

QUERY = "SELECT time, bpm from heart_rate"
PREFIX = "sqlite:///" # This is prefix if you are working in same dir as `.env` if you are working in `notebooks/` change to `sqlite:///../`
DATABASE_URL = os.getenv("DATABASE_URL").replace("sqlite://", PREFIX)
df = pd.read_sql(QUERY, DATABASE_URL)
```

## TODO:

- [ ] Sleep detection, for most of things like strain, recovery, HRV, etc..., I have been able to reverse engineer calculations, but I need reverse engineer sleep detection and activity detection before they can be automatically calculated
- [ ] Mobile/Desktop app
- [ ] Sp02 readings
- [ ] Temperature readings
