use crate::{util::BlockAggregateOutput, persistence::SQLitePersistence};
use axum::{
    extract::{Path, State},
    response::{sse::Event, Sse},
    Json,
};
use futures::{stream, Stream};
use serde::Serialize;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::broadcast;

#[derive(Serialize)]
pub struct AggregateResponse {
    total_p2pk_utxo_count: i64,
    total_p2pk_utxo_value: f64,
}

#[derive(Serialize)]
pub struct BlockResponse {
    date: String,
    block_height: usize,
    block_hash: String,
    total_p2pk_addresses: u32,
    total_p2pk_value: f64,
}

pub struct AppState {
    pub(crate) db: SQLitePersistence,
    pub(crate) sender: broadcast::Sender<BlockAggregateOutput>,
}

pub(crate) async fn stream_blocks(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.sender.subscribe();

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

pub async fn get_aggregates(State(state): State<Arc<AppState>>) -> Json<AggregateResponse> {
    let (count, value) = state.db.get_total_aggregates().await.unwrap();

    Json(AggregateResponse {
        total_p2pk_utxo_count: count,
        total_p2pk_utxo_value: value,
    })
}

pub async fn get_block_by_hash(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Json<Option<BlockResponse>> {
    let block = state.db.get_block_by_hash(&hash).await.unwrap();

    Json(block.map(|b| BlockResponse {
        date: b.date,
        block_height: b.block_height,
        block_hash: b.block_hash_big_endian,
        total_p2pk_addresses: b.total_p2pk_addresses as u32,
        total_p2pk_value: b.total_p2pk_value,
    }))
}

pub async fn get_block_by_height(
    State(state): State<Arc<AppState>>,
    Path(height): Path<i64>,
) -> Json<Option<BlockResponse>> {
    let block = state.db.get_block_by_height(height).await.unwrap();

    Json(block.map(|b| BlockResponse {
        date: b.date,
        block_height: b.block_height,
        block_hash: b.block_hash_big_endian,
        total_p2pk_addresses: b.total_p2pk_addresses as u32,
        total_p2pk_value: b.total_p2pk_value,
    }))
}
