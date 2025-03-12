use crate::AppError;
use log::{info, error};
use std::env;
use std::path::PathBuf;
use std::process::Command;
use anyhow::Result;

#[derive(Clone, Debug, serde::Serialize)]
pub struct BlockAggregateOutput {
    pub date: String,
    pub block_height: usize,
    pub block_hash_big_endian: String,
    pub total_utxos: u32,
    pub total_sats: f64,
}

pub enum BtcAddressType {
    P2PK,
    P2TR,
}

impl BtcAddressType {
    pub fn as_str(&self) -> &str {
        match self {
            BtcAddressType::P2PK => "p2pk",
            BtcAddressType::P2TR => "p2tr",
        }
    }
}

use std::str::FromStr;
use std::fmt;

impl FromStr for BtcAddressType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "p2pk" => Ok(BtcAddressType::P2PK),
            "p2tr" => Ok(BtcAddressType::P2TR),
            _ => Err(format!("Unknown address type: {}", s))
        }
    }
}

impl fmt::Display for BtcAddressType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub async fn capture_p2pk_blocks_graph(block_height: usize) -> Result<(), AppError> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let js_path = PathBuf::from(manifest_dir).join("web/scripts/captureChart.js");

    info!("Executing JavaScript script at {}", js_path.display());

    // Execute the command in fire-and-forget mode
    let child = Command::new("node")
        .arg(js_path)
        .arg(block_height.to_string())
        .spawn();

    match child {
        Ok(_) => info!("JavaScript execution started successfully"),
        Err(e) => error!("Failed to start JavaScript execution: {}", e),
    }

    Ok(())
}
