#[derive(Clone, Debug, serde::Serialize)]
pub struct BlockAggregateOutput {
    pub date: String,
    pub block_height: usize,
    pub block_hash_big_endian: String,
    pub total_p2pk_addresses: u32,
    pub total_p2pk_value: f64,
}
