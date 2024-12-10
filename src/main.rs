use std::{
    fs::{rename, File, OpenOptions},
    io::{Read, Seek, Write},
    net,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use crossbeam_channel::bounded;
use nakamoto::client::{
    network::{Network, Services},
    traits::Handle,
    Client, Config,
};
use thiserror::Error;

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
fn main() -> Result<(), AppError> {
    println!("Initializing sled database...");
    let db = sled::open("db")?;
    let db = Arc::new(db); // Wrap in Arc for thread-safe sharing

    println!("Initializing output vector...");
    let out = Arc::new(Mutex::new(Vec::<String>::new()));

    println!("Opening or creating 'out.csv'...");
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

    println!("Determining resume height and P2PK stats...");
    // Get the last line of the CSV file and parse the height from it
    let resume_height = {
        let out_lock = out.lock().unwrap();
        if let Some(last_line) = out_lock.last() {
            let fields: Vec<&str> = last_line.split(',').collect();
            if let Some(height_str) = fields.first() {
                height_str.parse::<u64>().unwrap_or(1)
            } else {
                1
            }
        } else {
            1
        }
    };

    // Increment resume_height to start from the next block
    let resume_height = resume_height.checked_add(1).unwrap_or(1);

    // Optional: Log the adjusted resume height
    println!("Resuming from height {}", resume_height);

    // Get the last line of the CSV file and parse the P2PK addresses and coins from it
    let (p2pk_addresses, p2pk_coins) = {
        let out_lock = out.lock().unwrap();
        if let Some(last_line) = out_lock.last() {
            let fields: Vec<&str> = last_line.split(',').collect();
            let p2pk_addresses: i32 = if fields.len() >= 2 {
                fields[1].parse().unwrap_or(0)
            } else {
                0
            };
            let p2pk_coins: i64 = if fields.len() >= 3 {
                fields[2].parse().unwrap_or(0)
            } else {
                0
            };
            (p2pk_addresses, p2pk_coins)
        } else {
            (0, 0)
        }
    };

    // If the file only contains the header, set the resume height to 1
    let resume_height = if resume_height == 0 { 1 } else { resume_height };

    println!(
        "Resuming from height {}, P2PK addresses: {}, P2PK satoshis: {}",
        resume_height, p2pk_addresses, p2pk_coins
    );

    println!("Configuring Nakamoto client...");
    let cfg = Config::new(Network::Mainnet);

    println!("Creating Nakamoto client...");
    // Create a client using the above network reactor.
    let client = Client::<Reactor>::new()?;
    let header_handle = client.handle();
    let block_handle = client.handle();

    println!("Setting up block processed channel...");
    // Create a channel to signal when a block has been processed.
    let (block_processed_tx, block_processed_rx) = bounded::<u32>(1);

    println!("Spawning client thread...");
    // Spawn the client thread
    let client_rx = spawn_thread(move || {
        client
            .run(cfg)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    });

    println!("Waiting for peers to connect...");
    header_handle.wait_for_peers(4, Services::Chain)?;
    println!("Connected to 4 peers.");

    println!("Fetching tip height...");
    // Get the tip height of the blockchain
    let (tip_height, _) = header_handle.get_tip()?;
    println!("Current tip height: {}", tip_height);

    println!("Spawning block processing thread...");
    // Clone the necessary Arcs for the processing thread
    let out_clone = Arc::clone(&out);
    let db_clone = Arc::clone(&db);

    // Spawn the block processing thread
    let block_processor_rx = spawn_thread(move || {
        let mut p2pk_tx_count: i32 = p2pk_addresses;
        let mut p2pk_satoshis: i64 = p2pk_coins;

        println!("Starting block processing thread...");

        for (block, height) in block_handle.blocks() {
            println!(
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

                        // Update the CSV file
                        let content = {
                            let out_lock = out_clone.lock().unwrap();
                            out_lock.join("\n")
                        };
                        File::create("out.csv")?.write_all(content.as_bytes())?;
                    }
                }
            }

            println!(
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
        }

        Ok(())
    });

    println!(
        "Processing blocks from {} to {}...",
        resume_height, tip_height
    );

    for i in resume_height..=tip_height {
        println!("Fetching block at height {}...", i);
        let block_header = header_handle.get_block_by_height(i)?;
        let block_hash = match block_header {
            Some(h) => h.block_hash(),
            None => {
                eprintln!("No block found at height {}", i);
                continue;
            }
        };

        println!("Block {} hash: {:?}", i, block_hash);

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
                println!("Successfully processed block {}", height);
            }
            Err(e) => {
                eprintln!("Error waiting for block processing: {}", e);
                break;
            }
        }
    }

    println!("All blocks processed up to height {}.", tip_height);

    println!("Updating 'out.csv' with final data...");
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

    println!("Shutting down Nakamoto client...");
    // Ask the client to terminate.
    header_handle.shutdown()?;
    println!("Client shut down gracefully.");

    // Handle potential errors from the client thread
    match client_rx.recv() {
        Ok(Err(e)) => {
            eprintln!("Client encountered an error: {}", e);
            return Err(AppError::Other(e));
        }
        Ok(Ok(_)) => println!("Client thread terminated gracefully."),
        Err(e) => eprintln!("Failed to receive from client thread: {}", e),
    }

    // Handle potential errors from the block processing thread
    match block_processor_rx.recv() {
        Ok(Err(e)) => {
            eprintln!("Block processor encountered an error: {}", e);
            return Err(AppError::Other(e));
        }
        Ok(Ok(_)) => println!("Block processor thread terminated gracefully."),
        Err(e) => eprintln!("Failed to receive from block processor thread: {}", e),
    }

    println!("Program completed successfully.");
    Ok(())
}
