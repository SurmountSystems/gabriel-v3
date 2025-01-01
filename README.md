gabriel-v3

- [1. Introduction](#1-introduction)
- [2. Setup](#2-setup)
  - [2.1. Pre-reqs](#21-pre-reqs)
    - [2.1.1. Hardware](#211-hardware)
    - [2.1.2. Software](#212-software)
      - [2.1.2.1. Rust](#2121-rust)
      - [2.1.2.2. SQLite client](#2122-sqlite-client)
  - [2.2. Build Gabriel](#22-build-gabriel)
- [3. Inspect P2PK Analysis data](#3-inspect-p2pk-analysis-data)


## 1. Introduction
Measures how many unspent public key addresses there are, and how many coins are in them over time. Early Satoshi-era coins that are just sitting with exposed public keys. If we see lots of coins move... That's a potential sign that quantum computers have silently broken bitcoin.

Gabriel uses the [Nakamoto bitcoin client](https://github.com/cloudhead/nakamoto) to query the bitcoin network for blocks.
Each block is subsequently evaluated for UTXOs that may be vulnerable to a quantum threat.

## 2. Setup

### 2.1. Pre-reqs

#### 2.1.1. Hardware

Gabriel requires a stable connection to the internet.

#### 2.1.2. Software
##### 2.1.2.1. Rust
Gabriel is written in Rust.
The best way to install Rust is to use [rustup](https://rustup.rs).

##### 2.1.2.2. SQLite client
  
Gabriel persists P2PK utxo analysis to a local SQLite database.
If you would want to inspect this SQLite data directly,
you will need to download and install the  [SQLite client](https://sqlite.org/download.html) for your operating system.
  
Once installed, set the SQLITE_ABSOLUTE_PATH environment variable to the path of the SQLite database:
  
        $ export SQLITE_ABSOLUTE_PATH=/path/to/gabriel_p2pk.db

### 2.2. Build Gabriel
    
* You'll need the Gabriel source code:
  ```
  $ git clone https://github.com/SurmountSystems/gabriel-v3.git

  ```

* Set appropriate environment variables as follows:

  - RUST_LOG
    - set to a valid value (ie: "info", "debug", "error", etc) to override default logging level
    - NOTE: nakamoto client logging is currently hard-coded to "warn".  This can be overridden by setting the RUST_LOG environment variable to "info,p2p=info" .  ie: `export RUST_LOG=info,p2p=info`
  - RUST_BACKTRACE
    - set to 1 to enable backtrace
  - SQLITE_ABSOLUTE_PATH=/path/to/gabriel_p2pk.db

* Build and run Gabriel
  ```
  $ cargo build
  $ cargo run
  ```
  

## 3. Inspect P2PK Analysis data
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
