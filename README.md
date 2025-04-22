gabriel-v3

- [1. Introduction](#1-introduction)
- [2. Pre-reqs](#2-pre-reqs)
    - [2.0.1. Hardware](#201-hardware)
    - [2.0.2. Software](#202-software)
      - [2.0.2.1. Rust](#2021-rust)
      - [Node JS](#node-js)
      - [OS packages](#os-packages)
      - [2.0.2.2. SQLite client](#2022-sqlite-client)
- [3. Build and run Gabriel](#3-build-and-run-gabriel)
    - [3.0.1. Backend (Rust)](#301-backend-rust)
    - [3.0.2. Frontend (React)](#302-frontend-react)
- [4. Inspect Block Aggregate data in SQLite](#4-inspect-block-aggregate-data-in-sqlite)
- [5. Export Block Aggregate data to CSV](#5-export-block-aggregate-data-to-csv)
- [6. API Documentation](#6-api-documentation)
  - [6.1. Latest Block Aggregates](#61-latest-block-aggregates)
  - [6.2. Block Queries](#62-block-queries)
  - [6.3. Example Curl Commands](#63-example-curl-commands)


## 1. Introduction
Measures how many unspent public key addresses there are, and how many coins are in them over time. Early Satoshi-era coins that are just sitting with exposed public keys. If we see lots of coins move... That's a potential sign that quantum computers have silently broken bitcoin.

Gabriel uses the [Nakamoto bitcoin client](https://github.com/cloudhead/nakamoto) to query the bitcoin network for blocks.
Each block is subsequently evaluated for UTXOs that may be vulnerable to a quantum threat.

## 2. Pre-reqs

#### 2.0.1. Hardware

Gabriel requires a stable broadband connection to the internet.

#### 2.0.2. Software
##### 2.0.2.1. Rust
Gabriel is written in Rust.
The best way to install Rust is to use [rustup](https://rustup.rs).

##### Node JS
Gabriel includes a ReactJS web frontend.

Gabriel also includes functionality to automatically capture UTXO dashboard charts as png files when a new block is evaluated.
This chart capture functionality is written in Nodejs.

Subsequently, you'll need to install `nodejs` and `npm` as per your operating system.
ie (for Debian ):  `sudo apt install -y nodejs npm`

##### OS packages

```
# apt install libnss3 libatk1.0-0 libatk-bridge2.0-0 libcups2 libxdamage1 libxkbcommon0 libpango-1.0-0 libcairo2 libasound2
```

##### 2.0.2.2. SQLite client
  
Gabriel persists P2PK utxo analysis to a local SQLite database.
If you would want to inspect this SQLite data directly,
you will need to download and install the  [SQLite client](https://sqlite.org/download.html) for your operating system.
  
Once installed, set the SQLITE_ABSOLUTE_PATH environment variable to the path of the SQLite database:
  
        $ export SQLITE_ABSOLUTE_PATH=/path/to/gabriel_p2pk.db

## 3. Build and run Gabriel
    
* You'll need the Gabriel source code:
  ```
  $ git clone https://github.com/SurmountSystems/gabriel-v3.git

  ```

#### 3.0.1. Backend (Rust)

Set appropriate environment variables as follows:

  - RUST_LOG
    - set to a valid value (ie: "info", "debug", "error", etc) to override default logging level
    - NOTE: nakamoto client logging is currently hard-coded to "warn".  This can be overridden by setting the RUST_LOG environment variable to "info,p2p=info" .  ie: `export RUST_LOG=info,p2p=info`
  - RUST_BACKTRACE
    - optional
    - set to 1 to view Rust backtraces (aka: stacktraces) when an error occurs
  - SQLITE_ABSOLUTE_PATH
    - optional
    - default path is "db" directory in project root dir
    - ie: /path/to/gabriel_p2pk.db
  - REACT_APP_API_BASE_URL
    - optional
    - defaults to "http://0.0.0.0:3000"  (which corresponds to running in release mode in a local environment)
    - set to "http://0.0.0.0:3001" when running in development mode
  - RUN_NAKAMOTO_ANALYSIS
    - optional
    - set to "true" to run the Nakamoto analysis
    - set to "false" to skip the Nakamoto analysis
    - defaults to "true"
  - NAKAMOTO_PEER_COUNT
    - optional
    - defaults to 4
    - set to a different number to change number of Bitcoin Core peers Gabriel will connect to
  - CHART_CAPTURE_FREQUENCY_BLOCKS
    - optional
    - defaults to 3
    - captures charts every N blocks
  - CHART_CAPTURE_DELAY_SECONDS
    - optional
    - defaults to 10
    - number of seconds to wait after the chart is rendered before capturing the image.  Allows for dynamic data to fully render.
  - CHART_CAPTURE_IMAGE_DIR_PATH
    - optional
    - defaults to "/tmp/gabriel/images"
    - directory to save captured images
  
```bash
$ (cd web && npm install && npm run build)
$ cargo build
$ cargo run --release
```

#### 3.0.2. Frontend (React)
The React web application is rendered by the Rust backend server.

Alternatively, you can run the React web application in development mode:

Run the React development server (with hot reloading):
```bash
$ cd web
$ export GABRIEL_REACT_APP_BASE_URL=http://0.0.0.0:3000
$ export PORT=3001 
$npm start
```
This will:
- Start the React dev server on port 3001
- Enable hot reloading for frontend changes
- Connect to the Rust backend API on port 3000
  

## 4. Inspect Block Aggregate data in SQLite
Gabriel will persist analysis of P2PK utxos in a SQLite database.

The path of the SQLite database is the value of the SQLITE_ABSOLUTE_PATH environment variable.

At the command line, you can inspect the data in SQLite database similar to the following:

```
$ sqlite3 $SQLITE_ABSOLUTE_PATH
   
# list tables;
sqlite> .tables

# view the schema of the   p2pk_utxo_block_aggregates table:
sqlite> .schema p2pk_utxo_block_aggregates

# identify number of records in p2pk_utxo_block_aggregates table
sqlite> select count(block_height) from p2pk_utxo_block_aggregates;

# delete all records
sqlite> delete from p2pk_utxo_block_aggregates;

# quit sqlite command line:  press  <ctrl> d

```

## 5. Export Block Aggregate data to CSV

```
$ sqlite3 $SQLITE_ABSOLUTE_PATH ".headers on" ".mode csv" ".once \
        /tmp/p2pk_utxo_block_aggregates.csv" \
        "SELECT * FROM p2pk_utxo_block_aggregates;"
```


## 6. API Documentation

The API provides several endpoints to query Bitcoin block data and UTXO aggregates:

### 6.1. Latest Block Aggregates
`GET /api/blocks/latest`

Retrieves UTXO aggregates for recent blocks. Supports query parameters:
- `address_type`: Type of Bitcoin address (p2pk or p2tr)
- `num_blocks`: Number of recent blocks to return (default: 10)

Example responses:

```json
[
{
"block_height": 830000,
"total_utxos": 1234,
"total_sats": 5678900000,
"address_type": "P2PK"
},
// ... more blocks
]
```

### 6.2. Block Queries
- `GET /api/block/hash/:hash` - Get block by hash
- `GET /api/block/height/:height` - Get block by height
- `GET /api/blocks/stream` - Stream new blocks as Server-Sent Events (SSE)

### 6.3. Example Curl Commands

```bash
# Get latest 10 blocks for P2PK (default)
curl "http://0.0.0.0:3000/api/blocks/latest"

# Get latest 20 blocks for P2PK
curl "http://0.0.0.0:3000/api/blocks/latest?num_blocks=20"

# Get latest 10 blocks for P2TR
curl "http://0.0.0.0:3000/api/blocks/latest?address_type=p2tr"

# Get latest 15 blocks for P2TR
curl "http://0.0.0.0:3000/api/blocks/latest?address_type=p2tr&num_blocks=15"

# Get block by hash
curl "http://0.0.0.0:3000/api/block/hash/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"

# Get block by height
curl "http://0.0.0.0:3000/api/block/height/0"

# Stream new blocks (requires curl 7.68.0+ for EventStream support)
curl -N "http://0.0.0.0:3000/api/blocks/stream"

# Generate latest P2PK chart
curl -X PUT "http://0.0.0.0:3000/api/chart/p2pk/generate/latest"

```







