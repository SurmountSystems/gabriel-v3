use std::env;

use log::info;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Pool, Row, Sqlite};

use crate::util::BlockAggregateOutput;

#[derive(Debug)]
pub struct SQLitePersistence {
    pool: Pool<Sqlite>,
}

impl SQLitePersistence {
    pub async fn new(pool_max_size: u32) -> anyhow::Result<Self> {
        let sqlite_absolute_path = env::var("SQLITE_ABSOLUTE_PATH").map_err(|e| {
            anyhow::anyhow!("Missing SQLITE_ABSOLUTE_PATH environment variable: {}", e)
        })?;

        // Create parent directories if they don't exist
        if let Some(parent) = std::path::Path::new(&sqlite_absolute_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        if !sqlx::Sqlite::database_exists(&sqlite_absolute_path).await? {
            sqlx::Sqlite::create_database(&sqlite_absolute_path).await?;
        }

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(pool_max_size)
            .connect(&format!("sqlite:{}", sqlite_absolute_path))
            .await?;

        info!(
            "SQLite database pool created at {} with max connection pool size {}",
            sqlite_absolute_path, pool_max_size
        );

        // Create table if not exists
        sqlx::query(
            "create table if not exists p2pk_utxo_block_aggregates (
                block_height integer not null,
                block_hash_big_endian text primary key,
                date text not null,
                total_p2pk_addresses integer not null,
                total_p2pk_value real not null
            )",
        )
        .execute(&pool)
        .await?;

        // Add index on block_height
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_block_height ON p2pk_utxo_block_aggregates(block_height DESC)"
        )
        .execute(&pool)
        .await?;

        Ok(SQLitePersistence { pool })
    }

    pub async fn persist_block_aggregates(
        &self,
        block_aggregate: &BlockAggregateOutput,
    ) -> anyhow::Result<u64> {
        let result = sqlx::query("INSERT INTO p2pk_utxo_block_aggregates VALUES(?1,?2,?3,?4,?5)")
            .bind(block_aggregate.block_height as i64)
            .bind(&block_aggregate.block_hash_big_endian)
            .bind(&block_aggregate.date)
            .bind(block_aggregate.total_p2pk_addresses as i64)
            .bind(block_aggregate.total_p2pk_value)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn get_total_aggregates(&self) -> anyhow::Result<(i64, f64)> {
        let result = sqlx::query(
            "SELECT SUM(total_p2pk_addresses) as total_count, 
             SUM(total_p2pk_value) as total_value 
             FROM p2pk_utxo_block_aggregates",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((result.get::<i64, _>(0), result.get::<f64, _>(1)))
    }

    pub async fn get_block_by_hash(
        &self,
        hash: &str,
    ) -> anyhow::Result<Option<BlockAggregateOutput>> {
        let result = sqlx::query(
            "SELECT date, block_height, block_hash_big_endian, total_p2pk_addresses, total_p2pk_value 
             FROM p2pk_utxo_block_aggregates WHERE block_hash_big_endian = ?"
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(BlockAggregateOutput {
                date: row.get(0),
                block_height: row.get::<i64, _>(1) as usize,
                block_hash_big_endian: row.get(2),
                total_p2pk_addresses: row.get::<i64, _>(3) as u32,
                total_p2pk_value: row.get::<f64, _>(4),
            })),
            None => Ok(None),
        }
    }

    pub async fn get_block_by_height(
        &self,
        height: i64,
    ) -> anyhow::Result<Option<BlockAggregateOutput>> {
        let result = sqlx::query(
            "SELECT date, block_height, block_hash_big_endian, total_p2pk_addresses, total_p2pk_value 
             FROM p2pk_utxo_block_aggregates WHERE block_height = ?"
        )
        .bind(height)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(BlockAggregateOutput {
                date: row.get(0),
                block_height: row.get::<i64, _>(1) as usize,
                block_hash_big_endian: row.get(2),
                total_p2pk_addresses: row.get(3),
                total_p2pk_value: row.get(4),
            })),
            None => Ok(None),
        }
    }

    /* Returns the last block height in the database.
     * If the database is empty, returns None.
     */
    pub async fn get_last_block_height(&self) -> anyhow::Result<Option<i64>> {
        let result = sqlx::query("SELECT MAX(block_height) as max_height FROM p2pk_utxo_block_aggregates")
            .fetch_optional(&self.pool)
            .await?;

        // For an empty table, result.get(0) will return None because MAX() returns NULL
        Ok(result.and_then(|row| row.get::<Option<i64>, _>("max_height")))
    }
}
