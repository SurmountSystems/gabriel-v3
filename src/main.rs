use std::{
    env,
    net,
    sync::{mpsc, Arc},
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
use log::{info, debug, error};

use tokio::sync::broadcast;
use tokio::signal;
use axum::{
    http::{HeaderValue, Method, header::HeaderName},
    routing::{get, Router} 
};
use std::net::SocketAddr;
use tower_http::services::ServeDir;

use api::AppState;
use crate::util::{BlockAggregateOutput, BtcAddressType};

mod persistence;
mod util;
mod api;

/// The network reactor we're going to use.
type Reactor = nakamoto::net::poll::Reactor<net::TcpStream>;

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

/// Processes blocks and persists data to SQLite database
async fn process_blocks(
    block_handle: impl Handle,
    db: Arc<sled::Db>,
    sqlite_persistence: persistence::SQLitePersistence,
    block_processed_tx: crossbeam_channel::Sender<u32>,
    sse_sender: broadcast::Sender<BlockAggregateOutput>,
    initial_p2pk_addresses: i32,
    initial_p2pk_coins: i64,
) -> Result<(), AppError> {
    let mut p2pk_tx_count: i32 = initial_p2pk_addresses;
    let mut p2pk_satoshis: i64 = initial_p2pk_coins;

    info!("Starting block processing...");

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
                    db.insert(
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
                if let Some(value_bytes) = db.get(input_key.as_bytes())? {
                    let value = i64::from_le_bytes(value_bytes.as_ref().try_into().unwrap());
                    p2pk_tx_count -= 1;
                    p2pk_satoshis -= value;
                    db.remove(input_key.as_bytes())?;
                }
            }
        }

        info!(
            "P2PK Transactions: {}, P2PK Satoshis: {}",
            p2pk_tx_count, p2pk_satoshis
        );

        // Persist the block data to the SQLite database
        let block_data = BlockAggregateOutput {
            date: Utc.timestamp_opt(block.header.time as i64, 0)
                .unwrap()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
            block_height: height as usize,
            block_hash_big_endian: block.block_hash().to_string(),
            total_utxos: p2pk_tx_count as u32,
            total_sats: p2pk_satoshis as f64,
        };

        sqlite_persistence.persist_block_aggregates(BtcAddressType::P2PK.as_str().to_string(), &block_data).await?;

        // Signal that we've processed this block
        block_processed_tx.send(height as u32)?;

        // Send SSE notification
        if let Err(err) = sse_sender.send(block_data.clone()) {
            error!("Failed to send SSE: {:?}", err);
        }
    }

    Ok(())
}

async fn run_apis_and_web_app(sender: broadcast::Sender<BlockAggregateOutput>) -> anyhow::Result<()> {
    // Create a SQLite persistence instance with a connection pool
    let sqlite_persistence = persistence::SQLitePersistence::new(5).await?;

    let app_state = Arc::new(AppState {
        db: sqlite_persistence,
        sender: sender,
    });

    // Determine socket that web_app will bind top
    let web_addr: SocketAddr = env::var("WEB_SOCKET_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()
        .expect("Failed to parse API_ADDR");
    info!("REST API listening on {}", web_addr);

    // Define common API routes
    let api_routes = Router::new()
        .route("/api/blocks/latest", get(api::get_latest_block_aggregates))
        .route("/api/block/hash/:hash", get(api::get_block_by_hash))
        .route("/api/block/height/:height", get(api::get_block_by_height))
        .route("/api/blocks/stream", get(api::stream_blocks));

    let web_app_router = if cfg!(debug_assertions) {
        // In debug mode, proxy requests to React dev server
        info!("debug_assertions = true; enabling CORS for localhost development");
        api_routes
            .nest_service("/", tower_http::services::ServeDir::new("web/public"))
            .layer(
                tower_http::cors::CorsLayer::new()
                    .allow_origin("*".parse::<HeaderValue>().unwrap())
                    .allow_methods(tower_http::cors::Any)
                    .allow_headers(tower_http::cors::Any)
            )
    } else {
        // In release mode, serve the built files
        api_routes
            .nest_service("/", ServeDir::new("web/build"))
    };

    // Spawn the web app server in the background
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(web_addr).await.unwrap();
        axum::serve(
            listener,
            web_app_router.with_state(app_state).into_make_service()
        )
        .await
        .unwrap();
    });

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("shutdown signal received, starting graceful shutdown");
}

/// Run the light-client.
#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize env_logger with default level of info
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .filter_module("p2p", log::LevelFilter::Warn)
        .init();

    // Create a broadcast channel for SSE events and start the API server
    let (tx, _rx) = broadcast::channel(100);
    run_apis_and_web_app(tx.clone()).await?;

    // Check if we should run the Nakamoto analysis (defaults to true)
    let run_analysis = env::var("RUN_NAKAMOTO_ANALYSIS")
        .map(|val| val.to_lowercase() != "false")
        .unwrap_or(true);

    if run_analysis {
        run_nakamoto_analysis(tx.clone()).await?;
    } else {
        // Wait for shutdown signal instead of pending forever
        shutdown_signal().await;
    }

    Ok(())
}

async fn run_nakamoto_analysis(sse_sender: broadcast::Sender<BlockAggregateOutput>) -> Result<(), AppError> {
    
    info!("Initializing sled key-value store to track P2PK transactions...");
    let db = sled::open("db")?;
    let db = Arc::new(db); // Wrap in Arc for thread-safe sharing

    info!("Initializing sqlite to store block data");
    let sqlite_persistence = persistence::SQLitePersistence::new(1).await.map_err(|e| AppError::SqliteError(e))?;

    // Get the last block height from the sqlite database
    let resume_height = {
        let last_height = sqlite_persistence.get_last_block_height(BtcAddressType::P2PK.as_str().to_string()).await?;
        debug!("Last height from database: {:?}", last_height);
        match last_height {
            Some(height) => height.checked_add(1).unwrap_or(1) as u64,
            None => 0 // If the database is empty, start from the first block
        }
    };

    // Get the total utxos and sats from the last processed block
    let (p2pk_addresses, p2pk_coins) = {
        if resume_height > 0 {
            let last_block = sqlite_persistence.get_block_by_height(BtcAddressType::P2PK.as_str().to_string(), (resume_height - 1) as i64).await?;
            match last_block {
                Some(block) => (
                    block.total_utxos as i32,
                    block.total_sats as i64
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
        match client.run(cfg) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Nakamoto client encountered an error: {:?}", e);
                Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    });

    info!("Waiting for peers to connect...");
    header_handle.wait_for_peers(4, Services::Chain)?;
    info!("Connected to 4 peers.");

    info!("Fetching initial tip height...");
    let (mut tip_height, _) = header_handle.get_tip()?;
    info!("Initial tip height: {}", tip_height);

    info!("Spawning block processing thread...");
    let db_clone = Arc::clone(&db);
    let block_processor_rx = spawn_thread(move || {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            process_blocks(
                block_handle,
                db_clone,
                sqlite_persistence,
                block_processed_tx,
                sse_sender,
                p2pk_addresses,
                p2pk_coins,
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
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
                error!("No block found at height {}", i);
                continue;
            }
        };

        info!("Block {} hash: {:?}", i, block_hash);

        // Request the block.
        header_handle.get_block(&block_hash)?;

        // Wait for the block thread to process a block.
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
                error!("Error waiting for block processing: {}", e);
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

    info!("Shutting down Nakamoto client...");
    // Ask the client to terminate.
    header_handle.shutdown()?;
    info!("Client shut down gracefully.");

    // Handle potential errors from both threads simultaneously
    let (client_result, block_processor_result) = (client_rx.recv(), block_processor_rx.recv());

    // Check client thread result
    if let Ok(Err(e)) = client_result {
        error!("Client encountered an error: {}", e);
        return Err(AppError::Other(e));
    } else if let Ok(Ok(_)) = client_result {
        info!("Client thread terminated gracefully.");
        return Err(AppError::CustomError(
            "Client thread terminated gracefully.".to_owned(),
        ));
    } else if let Err(e) = client_result {
        error!("Failed to receive from client thread: {}", e);
        return Err(AppError::CustomError(format!(
            "Failed to receive from client thread: {}",
            e
        )));
    }

    // Check block processor thread result
    if let Ok(Err(e)) = block_processor_result {
        error!("Block processor encountered an error: {}", e);
        return Err(AppError::Other(e));
    } else if let Ok(Ok(_)) = block_processor_result {
        info!("Block processor thread terminated gracefully.");
        return Err(AppError::CustomError(
            "Block processor thread terminated gracefully.".to_owned(),
        ));
    } else if let Err(e) = block_processor_result {
        error!("Failed to receive from block processor thread: {}", e);
        return Err(AppError::CustomError(format!(
            "Failed to receive from block processor thread: {}",
            e
        )));
    }

    info!("Program completed successfully.");
    Ok(())
}
