use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, Write},
    net,
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::{bounded, Sender};
use nakamoto::{
    client::{
        network::{Network, Services},
        traits::Handle,
        Client, Config,
    },
    common,
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

fn run_with_error_channel<F>(task: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(Sender<Box<dyn std::error::Error + Send + Sync>>) + Send + 'static,
{
    let (err_sender, err_receiver) = bounded::<Box<dyn std::error::Error + Send + Sync>>(1);

    // Spawn a new thread to run the task
    thread::spawn(move || {
        task(err_sender);
    });

    // Wait for an error from the child thread
    match err_receiver.recv() {
        Ok(err) => Err(err),
        Err(_) => Ok(()), // No errors reported
    }
}

/// Run the light-client.
fn main() -> Result<(), AppError> {
    // Initialize sled database
    let db = sled::open("db")?;
    let db = Arc::new(db); // Wrap in Arc for thread-safe sharing

    // Wrap the 'out' vector in Arc and Mutex for thread-safe sharing
    let out = Arc::new(Mutex::new(Vec::<String>::new()));

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

    // If the file only contains the header, set the resume height to 1
    let resume_height = if resume_height == 0 { 1 } else { resume_height };

    // Get the last line of the CSV file and parse the P2PK addresses and coins from it
    let (p2pk_addresses, p2pk_coins) = {
        let out_lock = out.lock().unwrap();
        if let Some(last_line) = out_lock.last() {
            let fields: Vec<&str> = last_line.split(',').collect();
            let p2pk_addresses: i32 = if fields.len() >= 3 {
                fields[2].parse().unwrap_or(0)
            } else {
                0
            };
            let p2pk_coins: i64 = if fields.len() >= 4 {
                fields[3].parse().unwrap_or(0)
            } else {
                0
            };
            (p2pk_addresses, p2pk_coins)
        } else {
            (0, 0)
        }
    };

    let cfg = Config::new(Network::Mainnet);

    // Create a client using the above network reactor.
    let client = Client::<Reactor>::new()?;
    let header_handle = client.handle();
    let block_handle = client.handle();

    // Create a channel to signal when a block has been processed.
    let (block_processed_tx, block_processed_rx) = bounded::<u32>(1);

    // Run the client on a different thread, to not block the main thread.
    run_with_error_channel(move |err_sender| match client.run(cfg) {
        Ok(_) => (),
        Err(e) => {
            let boxed_err: Box<dyn std::error::Error + Send + Sync> = Box::new(e);
            if let Err(e) = err_sender.send(boxed_err) {
                eprintln!("Failed to send error: {}", e);
            }
        }
    })?;

    // Wait for the client to be connected to peers.
    header_handle.wait_for_peers(8, Services::Chain)?;

    // Loop through the first n blocks and print the hash and txs.
    let (tip_height, _) = header_handle.get_tip()?;

    // Clone the Arc to move into the thread
    let out_clone = Arc::clone(&out);
    // Get blocks from the client
    run_with_error_channel(move |err_sender| {
        let res = {
            let mut p2pk_tx_count: i32 = p2pk_addresses;
            let mut p2pk_satoshis: i64 = p2pk_coins;

            println!("Starting block thread");
            let blocks = block_handle.blocks();
            while let Ok((block, height)) = blocks.recv() {
                println!("Block {} txs: {:?}", height, block.txdata.len());

                // Scan the block for P2PK transactions
                for tx in block.txdata.iter() {
                    let txid = tx.txid();

                    for (i, output) in tx.output.iter().enumerate() {
                        if output.script_pubkey.is_p2pk() {
                            db.insert(
                                format!("{}:{}", txid, i).as_bytes(),
                                output.value.to_le_bytes().to_vec(),
                            )
                            .unwrap();

                            p2pk_tx_count += 1;
                            p2pk_satoshis += output.value as i64;
                        }
                    }

                    for input in tx.input.iter() {
                        let input_txid = input.previous_output.txid;
                        let input_vout = input.previous_output.vout;
                        let input_key = format!("{}:{}", input_txid, input_vout);
                        if let Some(value_bytes) = db.get(input_key.as_bytes()).unwrap() {
                            let value =
                                i64::from_le_bytes(value_bytes.as_ref().try_into().unwrap());
                            p2pk_tx_count -= 1;
                            p2pk_satoshis -= value;
                            db.remove(input_key.as_bytes()).unwrap();
                            let content = {
                                let out_lock = out_clone.lock().unwrap();
                                out_lock.join("\n")
                            };
                            File::create("out.csv")
                                .and_then(|mut file| file.write_all(content.as_bytes()))
                                .unwrap_or_else(|e| {
                                    eprintln!("Failed to write to file: {}", e);
                                });
                        }
                    }
                }

                println!("P2PK: {:?}, {:?}", p2pk_tx_count, p2pk_satoshis);

                // Signal that we've processed this block
                if let Err(e) = block_processed_tx.send(height as u32) {
                    let boxed_err: Box<dyn std::error::Error + Send + Sync> =
                        Box::new(AppError::ChannelSend(e));
                    if let Err(send_err) = err_sender.send(boxed_err) {
                        eprintln!("Failed to send error: {}", send_err);
                    }
                }
            }

            Ok(())
        };
        match res {
            Ok(_) => (),
            Err(e) => {
                let boxed_err: Box<dyn std::error::Error + Send + Sync> =
                    Box::new(AppError::ChannelSend(e));
                if let Err(send_err) = err_sender.send(boxed_err) {
                    eprintln!("Failed to send error: {}", send_err);
                }
            }
        }
    })?;

    for i in resume_height..tip_height {
        let block_header = header_handle.get_block_by_height(i)?;

        let block_hash = block_header.map(|h| h.block_hash()).ok_or_else(|| {
            nakamoto::client::Error::Chain(common::block::tree::Error::InvalidBlockHeight(i))
        })?;

        println!("Block {} hash: {:?}", i, block_hash);

        // Request the block.
        header_handle.get_block(&block_hash)?;

        // Wait for the block thread to process this block.
        match block_processed_rx.recv() {
            Ok(height) => {
                assert_eq!(
                    height, i as u32,
                    "Received block height doesn't match requested height"
                );
            }
            Err(e) => {
                println!("Error waiting for block processing: {}", e);
                break;
            }
        }
    }

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

    // Ask the client to terminate.
    header_handle.shutdown()?;
    Ok(())
}
