gabriel-v3

##### 2.1.2.3. SQLite client
  
Gabriel persists P2PK utxo analysis to a local SQLite database.
You will need to download and install the  [SQLite client](https://sqlite.org/download.html) for your operating system.
  
Once installed, set the SQLITE_ABSOLUTE_PATH environment variable to the path of the SQLite database:
  
        $ export SQLITE_ABSOLUTE_PATH=/path/to/gabriel_p2pk.db

### 2.2. Clone Gabriel
    
You'll need the Gabriel source code:

```
$ git clone https://github.com/SurmountSystems/gabriel-v3.git
$ git checkout HB/gabriel-v2
```

- RUST_LOG
  - set to a valid value (ie: "info", "debug", "error", etc) to override default logging level
- RUST_BACKTRACE
  - set to 1 to enable backtrace
