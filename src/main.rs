use std::{
    fs::{rename, File, OpenOptions},
    io::{Read, Seek, Write},
    net,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use chrono::{Utc, TimeZone};
use crossbeam_channel::bounded;
use nakamoto::client::{
    network::{Network, Services},
    traits::Handle,
    Client, Config,
};
use thiserror::Error;
use log::{info, debug};

use crate::util::BlockAggregateOutput;

mod persistence;
mod util;

/// The network reactor we're going to use.
type Reactor = nakamoto::net::poll::Reactor<net::TcpStream>;

const HEADER: &str = "Height,Total P2PK addresses,Total P2PK satoshis";

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    NakamotoError(#[from] nakamoto::client::Error),
    #[error(transparent)]
    NakamotoHandleError(#[from] nakamoto::client::handle::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("channel send error")]
    ChannelSend(#[from] crossbeam_channel::SendError<u32>),
    #[error(transparent)]
    SledError(#[from] sled::Error),
    #[error(transparent)]
    SqliteError(#[from] anyhow::Error),
    #[error("{0}")]
    CustomError(String),
}

/// Function to spawn a thread and handle errors asynchronously
fn spawn_thread<F>(task: F) -> mpsc::Receiver<Result<(), Box<dyn std::error::Error + Send + Sync>>>
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = task();
        let _ = tx.send(result);
    });
    rx
}

/// Run the light-client.
#[tokio::main]
async fn main() -> Result<(), AppError> {

    // Initialize env_logger with default level of info
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .filter_module("p2p", log::LevelFilter::Warn)
        .init();

    info!("Initializing sled key-value store to track P2PK transactions...");
    let db = sled::open("db")?;
    let db = Arc::new(db); // Wrap in Arc for thread-safe sharing

    info!("Initializing sqlite to store block data");
    let sqlite_persistence = persistence::SQLitePersistence::new(1).await.map_err(|e| AppError::SqliteError(e))?;

    info!("Initializing output vector...");
    let out = Arc::new(Mutex::new(Vec::<String>::new()));

    info!("Opening or creating 'out.csv'...");
    // Open the file if it exists, otherwise create it
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("out.csv")?;

    // Read the file content into a string
    let mut content = String::new();
    {
        let mut file_guard = file.try_clone()?;
        file_guard.read_to_string(&mut content)?;
    }

    {
        let mut out_lock = out.lock().unwrap();
        // Check if the file is empty or doesn't start with the header
        if content.is_empty() || !content.starts_with(HEADER) {
            // If empty or no header, add the header to the beginning of out
            out_lock.push(HEADER.to_owned());
        }

        // Split the content into lines and collect into the out vector
        out_lock.extend(content.lines().map(|line| line.to_string()));
    }

    // Get the last block height from the sqlite database
    let resume_height = {
        let last_height = sqlite_persistence.get_last_block_height().await?;
        debug!("Last height from database: {:?}", last_height);
        match last_height {
            Some(height) => height.checked_add(1).unwrap_or(1) as u64,
            None => 0 // If the database is empty, start from the first block
        }
    };

    // Get the P2PK addresses and coins from the last processed block
    let (p2pk_addresses, p2pk_coins) = {
        if resume_height > 0 {
            let last_block = sqlite_persistence.get_block_by_height((resume_height - 1) as i64).await?;
            match last_block {
                Some(block) => (
                    block.total_p2pk_addresses as i32,
                    block.total_p2pk_value as i64
                ),
                None => (0, 0)
            }
        } else {
            (0, 0)
        }
    };

    info!(
        "Resuming from height {}, P2PK addresses: {}, P2PK satoshis: {}",
        resume_height, p2pk_addresses, p2pk_coins
    );

    info!("Configuring Nakamoto client...");
    let cfg = Config::new(Network::Mainnet);

    info!("Creating Nakamoto client...");
    // Create a client using the above network reactor.
    let client = Client::<Reactor>::new()?;
    let header_handle = client.handle();
    let block_handle = client.handle();

    info!("Setting up block processed channel...");
    // Create a channel to signal when a block has been processed.
    let (block_processed_tx, block_processed_rx) = bounded::<u32>(1);

    info!("Spawning client thread...");
    // Spawn the client thread
    let client_rx = spawn_thread(move || {
        client
            .run(cfg)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });

    info!("Waiting for peers to connect...");
    header_handle.wait_for_peers(4, Services::Chain)?;
    info!("Connected to 4 peers.");

    info!("Fetching initial tip height...");
    let (mut tip_height, _) = header_handle.get_tip()?;
    info!("Initial tip height: {}", tip_height);

    info!("Spawning block processing thread...");
    // Clone the necessary Arcs for the processing thread
    let out_clone = Arc::clone(&out);
    let db_clone = Arc::clone(&db);

    // Spawn the block processing thread
    let block_processor_rx = spawn_thread(move || {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let mut p2pk_tx_count: i32 = p2pk_addresses;
            let mut p2pk_satoshis: i64 = p2pk_coins;

            info!("Starting block processing thread...");

            for (block, height) in block_handle.blocks() {
                info!(
                    "Processing Block {}: {} transactions",
                    height,
                    block.txdata.len()
                );

                // Scan the block for P2PK transactions
                for tx in block.txdata.iter() {
                    let txid = tx.txid();

                    for (i, output) in tx.output.iter().enumerate() {
                        if output.script_pubkey.is_p2pk() {
                            db_clone.insert(
                                format!("{}:{}", txid, i).as_bytes(),
                                output.value.to_le_bytes().to_vec(),
                            )?;

                            p2pk_tx_count += 1;
                            p2pk_satoshis += output.value as i64;
                        }
                    }

                    for input in tx.input.iter() {
                        let input_txid = input.previous_output.txid;
                        let input_vout = input.previous_output.vout;
                        let input_key = format!("{}:{}", input_txid, input_vout);
                        if let Some(value_bytes) = db_clone.get(input_key.as_bytes())? {
                            let value = i64::from_le_bytes(value_bytes.as_ref().try_into().unwrap());
                            p2pk_tx_count -= 1;
                            p2pk_satoshis -= value;
                            db_clone.remove(input_key.as_bytes())?;
                        }
                    }
                }

                info!(
                    "P2PK Transactions: {}, P2PK Satoshis: {}",
                    p2pk_tx_count, p2pk_satoshis
                );

                // Append new entry to 'out'
                {
                    let mut out_lock = out_clone.lock().unwrap();
                    let new_entry = format!("{},{},{}", height, p2pk_tx_count, p2pk_satoshis);
                    out_lock.push(new_entry);
                }

                // Update the CSV file atomically
                {
                    let content = {
                        let out_lock = out_clone.lock().unwrap();
                        out_lock.join("\n")
                    };
                    let temp_path = "out.csv.tmp";
                    File::create(temp_path)?.write_all(content.as_bytes())?;
                    rename(temp_path, "out.csv")?;
                }

                // Signal that we've processed this block
                block_processed_tx.send(height as u32)?;

                // Persist the block data to the SQLite database
                let block_data = BlockAggregateOutput {
                    date: Utc.timestamp_opt(block.header.time as i64, 0)
                        .unwrap()
                        .format("%Y-%m-%d %H:%M:%S UTC")
                        .to_string(),
                    block_height: height as usize,
                    block_hash_big_endian: block.block_hash().to_string(),
                    total_p2pk_addresses: p2pk_tx_count as u32,
                    total_p2pk_value: p2pk_satoshis as f64,
                };

                sqlite_persistence.persist_block_aggregates(&block_data).await?;
            }

            Ok(())
        })
    });

    info!(
        "Processing blocks from {} to {}...",
        resume_height, tip_height
    );

    #[allow(clippy::mut_range_bound)]
    for i in resume_height..=tip_height {
        info!("Fetching block at height {}...", i);
        let block_header = header_handle.get_block_by_height(i)?;
        let block_hash = match block_header {
            Some(h) => h.block_hash(),
            None => {
                eprintln!("No block found at height {}", i);
                continue;
            }
        };

        info!("Block {} hash: {:?}", i, block_hash);

        // Request the block.
        header_handle.get_block(&block_hash)?;

        // Wait for the block thread to process this block.
        match block_processed_rx.recv() {
            Ok(height) => {
                assert_eq!(
                    height, i as u32,
                    "Received block height {} doesn't match requested height {}",
                    height, i
                );
                info!("Successfully processed block {}", height);
            }
            Err(e) => {
                eprintln!("Error waiting for block processing: {}", e);
                break;
            }
        }

        // Update the tip height after processing each block
        let (new_tip_height, _) = header_handle.get_tip()?;
        if new_tip_height > tip_height {
            info!("New tip height detected: {}", new_tip_height);
            tip_height = new_tip_height;
        }
    }

    info!("All blocks processed up to height {}.", tip_height);

    info!("Updating 'out.csv' with final data...");
    // When writing back to the file, ensure we start from the beginning
    let mut file = file.try_clone()?;
    {
        let out_lock = out.lock().unwrap();
        file.seek(std::io::SeekFrom::Start(0))?;
        file.set_len(0)?; // Truncate the file
        for line in &*out_lock {
            writeln!(file, "{}", line)?;
        }
    }

    info!("Shutting down Nakamoto client...");
    // Ask the client to terminate.
    header_handle.shutdown()?;
    info!("Client shut down gracefully.");

    // Handle potential errors from both threads simultaneously
    let (client_result, block_processor_result) = (client_rx.recv(), block_processor_rx.recv());

    // Check client thread result
    if let Ok(Err(e)) = client_result {
        eprintln!("Client encountered an error: {}", e);
        return Err(AppError::Other(e));
    } else if let Ok(Ok(_)) = client_result {
        info!("Client thread terminated gracefully.");
        return Err(AppError::CustomError(
            "Client thread terminated gracefully.".to_owned(),
        ));
    } else if let Err(e) = client_result {
        eprintln!("Failed to receive from client thread: {}", e);
        return Err(AppError::CustomError(format!(
            "Failed to receive from client thread: {}",
            e
        )));
    }

    // Check block processor thread result
    if let Ok(Err(e)) = block_processor_result {
        eprintln!("Block processor encountered an error: {}", e);
        return Err(AppError::Other(e));
    } else if let Ok(Ok(_)) = block_processor_result {
        info!("Block processor thread terminated gracefully.");
        return Err(AppError::CustomError(
            "Block processor thread terminated gracefully.".to_owned(),
        ));
    } else if let Err(e) = block_processor_result {
        eprintln!("Failed to receive from block processor thread: {}", e);
        return Err(AppError::CustomError(format!(
            "Failed to receive from block processor thread: {}",
            e
        )));
    }

    info!("Program completed successfully.");
    Ok(())
}
