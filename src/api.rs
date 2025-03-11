use crate::{persistence::SQLitePersistence, util::{self, BlockAggregateOutput, BtcAddressType}};
use axum::{
    extract::{Path, Query, State}, response::{sse::Event, Sse}, Json
};
use futures::{stream, Stream};

use serde::Serialize;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use std::collections::HashMap;
use serde_json::json;
use crate::ApiError;

#[derive(Serialize)]
pub struct AggregateResponse {
    total_utxos: i64,
    total_sats: f64,
}

#[derive(Serialize)]
pub struct BlockResponse {
    date: String,
    block_height: usize,
    block_hash: String,
    total_utxos: u32,
    total_sats: f64,
}

pub struct AppState {
    pub(crate) db: SQLitePersistence,
    pub(crate) sender: broadcast::Sender<BlockAggregateOutput>
}

pub(crate) async fn stream_blocks(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sender.subscribe();

    let stream = stream::unfold(rx, move |mut rx| async move {
        let msg = rx.recv().await.ok()?;
        let event = Event::default().data(serde_json::to_string(&msg).unwrap());
        Some((Ok(event), rx))
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

pub async fn get_latest_block_aggregates(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<BlockAggregateOutput>> {
    // Parse address_type from query params, default to None (which will be P2PK)
    let address_type = params.get("address_type")
        .and_then(|s| s.parse::<BtcAddressType>().ok());
    
    // Parse num_blocks from query params, default to None (which will be 10)
    let num_blocks = params.get("num_blocks")
        .and_then(|s| s.parse::<i64>().ok());

    let aggregates = state.db
        .get_latest_block_aggregates(address_type, num_blocks)
        .await
        .unwrap_or_default();

    Json(aggregates)
}

pub async fn get_block_by_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Json<Option<BlockResponse>> {
    let block = state.db.get_block_by_hash(BtcAddressType::P2PK.as_str().to_string(), &hash).await.unwrap();

    Json(block.map(|b| BlockResponse {
        date: b.date,
        block_height: b.block_height,
        block_hash: b.block_hash_big_endian,
        total_utxos: b.total_utxos as u32,
        total_sats: b.total_sats,
    }))
}

pub async fn get_block_by_height(
    State(state): State<Arc<AppState>>,
    Path(height): Path<i64>,
) -> Json<Option<BlockResponse>> {
    let block = state.db.get_block_by_height(BtcAddressType::P2PK.as_str().to_string(), height).await.unwrap();

    Json(block.map(|b| BlockResponse {
        date: b.date,
        block_height: b.block_height,
        block_hash: b.block_hash_big_endian,
        total_utxos: b.total_utxos as u32,
        total_sats: b.total_sats,
    }))
}

pub async fn generate_latest_p2pk_chart(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {

    util::capture_p2pk_blocks_graph(0).await.unwrap();

    // Create a JSON object with a single element
    let response = json!({ "Result": "Check logs for status of chart generation" });

    Ok(Json(response))
}
