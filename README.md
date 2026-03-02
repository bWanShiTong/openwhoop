# OpenWhoop

OpenWhoop is a project that allows you to download and analyze health data directly from your Whoop 4.0 device without a Whoop subscription or Whoop's servers, making the data your own.

Features include sleep detection, exercise detection, stress calculation, HRV analysis, SpO2, skin temperature, and strain scoring — all computed locally from raw sensor data.

## Getting Started

First copy `.env.example` into `.env` and then scan for your Whoop device:
```sh
cp .env.example .env
cargo run -r -- scan
```

After you find your device:

- On Linux, copy its address to `.env` under `WHOOP`
- On macOS, copy its name to `.env` under `WHOOP`

Then download data from your Whoop:
```sh
cargo run -r -- download-history
```

## Commands

| Command | Description |
|---------|-------------|
| `scan` | Scan for available Whoop devices |
| `download-history` | Download historical data from the device |
| `detect-events` | Detect sleep and exercise events from raw data |
| `sleep-stats` | Print sleep statistics (all-time and last 7 days) |
| `exercise-stats` | Print exercise statistics (all-time and last 7 days) |
| `calculate-stress` | Calculate stress scores (Baevsky stress index) |
| `calculate-spo2` | Calculate blood oxygen from raw sensor data |
| `calculate-skin-temp` | Calculate skin temperature from raw sensor data |
| `set-alarm <time>` | Set device alarm (see [Alarm Formats](#alarm-formats)) |
| `sync` | Sync data between local and remote databases |
| `merge <database_url>` | Copy packets from another database into the current one |
| `rerun` | Reprocess stored packets (useful after adding new packet handlers) |
| `enable-imu` | Enable IMU (accelerometer/gyroscope) data collection |
| `download-firmware` | Download firmware from WHOOP API |
| `version` | Get device firmware version |
| `restart` | Restart device |
| `erase` | Erase all history data from device |
| `completions <shell>` | Generate shell completions (bash, zsh, fish) |

### Alarm Formats

The `set-alarm` command accepts several time formats:

- **Datetime**: `2025-01-15 07:00:00` or `2025-01-15T07:00:00`
- **Time of day**: `07:00:00`
- **Relative offsets**: `1min`, `5min`, `10min`, `15min`, `30min`, `hour`

## Configuration

Configuration is done through environment variables or a `.env` file.

| Variable | Description | Required |
|----------|-------------|----------|
| `DATABASE_URL` | Database connection string (SQLite or PostgreSQL) | Yes |
| `WHOOP` | Device identifier (MAC address on Linux, name on macOS) | For device commands |
| `REMOTE` | Remote database URL for `sync` command | For sync |
| `BLE_INTERFACE` | BLE adapter to use, e.g. `"hci1 (usb:Something)"` (Linux only) | No |
| `DEBUG_PACKETS` | Set to `true` to store raw packets in database | No |
| `RUST_LOG` | Logging level (default: `info`) | No |
| `WHOOP_EMAIL` | WHOOP account email for `download-firmware` | For firmware |
| `WHOOP_PASSWORD` | WHOOP account password for `download-firmware` | For firmware |

### Database URLs

SQLite:
```
DATABASE_URL=sqlite://db.sqlite?mode=rwc
```

PostgreSQL:
```
DATABASE_URL=postgresql://user:password@localhost:5432/openwhoop
```

## Importing Data to Python

```py
import pandas as pd
import os

# Heart rate data
QUERY = "SELECT time, bpm FROM heart_rate"

# Other available tables:
# "SELECT * FROM sleep_cycles"
# "SELECT * FROM activities"

PREFIX = "sqlite:///"  # Use "sqlite:///../" if working from notebooks/
DATABASE_URL = os.getenv("DATABASE_URL").replace("sqlite://", PREFIX)
df = pd.read_sql(QUERY, DATABASE_URL)
```

## Protocol

For the full reverse engineering writeup, see [Reverse Engineering Whoop 4.0 for fun and FREEDOM](https://github.com/bWanShiTong/reverse-engineering-whoop-post).

### BLE Service

The device communicates over a custom BLE service (`61080001-8d6d-82b8-614a-1c8cb0f8dcc6`) with the following characteristics:

| UUID       | Name              | Direction    | Description |
|------------|-------------------|--------------|-------------|
| 0x61080002 | CMD_TO_STRAP      | Write        | Send commands to the device |
| 0x61080003 | CMD_FROM_STRAP    | Notify       | Device command responses |
| 0x61080004 | EVENTS_FROM_STRAP | Notify       | Event notifications |
| 0x61080005 | DATA_FROM_STRAP   | Notify       | Sensor and history data |
| 0x61080007 | MEMFAULT          | Notify       | Memory fault logs |

### Packet Structure

All packets follow the same general structure:

| Field | Size | Description |
|-------|------|-------------|
| SOF | 1 byte | Start of frame (`0xAA`) |
| Length | 2 bytes | Payload length (little-endian) |
| Header | 2 bytes | Packet type identifier |
| Payload | variable | Command or data payload |
| CRC-32 | 4 bytes | Checksum |

### CRC

Packets use a CRC-32 with custom parameters:
- Polynomial: `0x4C11DB7`
- Reflect input/output: `true`
- Initial value: `0x0`
- XOR output: `0xF43F44AC`

### Command Categories

Commands sent to `CMD_TO_STRAP` use a category byte:

| Category | Purpose |
|----------|---------|
| `0x03` | Start/end activity and recording |
| `0x0e` | Enable/disable broadcast heart rate |
| `0x16` | Trigger data retrieval |
| `0x19` | Erase device |
| `0x1d` | Reboot device |
| `0x23` | Sync/history requests |
| `0x42` | Set alarm time |
| `0x4c` | Get device name |

### History Data

Each historical reading (96 bytes) contains:

| Field | Description |
|-------|-------------|
| Heart rate | BPM (beats per minute) |
| RR intervals | Beat-to-beat timing in milliseconds |
| Activity | Classification (active, sleep, inactive, awake) |
| PPG green/red/IR | Photoplethysmography sensor values |
| SpO2 red/IR | Blood oxygen sensor values |
| Skin temperature | Thermistor ADC reading |
| Accelerometer | 3-axis gravity vector |
| Respiratory rate | Derived respiratory rate |

The remaining sensor fields in each packet (which the original blog post marked as unknown) have since been fully decoded and are used to compute SpO2, skin temperature, and stress metrics.

## TODO

- [x] Sleep detection and activity detection
- [x] SpO2 readings
- [x] Temperature readings
- [x] Stress calculation (Baevsky stress index)
- [x] HRV analysis (RMSSD)
- [x] Strain scoring (Edwards TRIMP)
- [x] Database sync between SQLite and PostgreSQL
- [ ] Mobile/Desktop app
- [ ] Testout Whoop 5.0
