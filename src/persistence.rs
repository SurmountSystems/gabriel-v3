use std::env;

use log::info;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Pool, Row, Sqlite};

use crate::util::{BlockAggregateOutput, BtcAddressType};

#[derive(Debug)]
pub struct SQLitePersistence {
    pool: Pool<Sqlite>,
}

impl SQLitePersistence {

    /// Initialize the SQLite database schema
    async fn initialize_schema(pool: &Pool<Sqlite>, btc_address_type: String) -> anyhow::Result<()> {
        let table_name = format!("{}_utxo_block_aggregates", btc_address_type);
        let index_name = format!("idx_{}_block_height", btc_address_type);

        // Create table if not exists
        sqlx::query(&format!(
            "create table if not exists {} (
                block_height integer not null,
                block_hash_big_endian text primary key,
                date text not null,
                total_utxos integer not null,
                total_sats real not null
            )",
            table_name
        ))
        .execute(pool)
        .await?;

        // Add index on block_height
        sqlx::query(&format!(
            "CREATE INDEX IF NOT EXISTS {} ON {}(block_height DESC)",
            index_name, table_name
        ))
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn new(pool_max_size: u32) -> anyhow::Result<Self> {
        let sqlite_absolute_path = env::var("SQLITE_ABSOLUTE_PATH")
            .unwrap_or_else(|_| String::from("/tmp/gabriel/gabriel_p2pk.db"));

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
            "SQLite database pool created (or reused) at {} with max connection pool size {}",
            sqlite_absolute_path, pool_max_size
        );

        // Initialize schema for p2pk addresses
        Self::initialize_schema(&pool, BtcAddressType::P2PK.as_str().to_string()).await?;

        Ok(SQLitePersistence { pool })
    }

    pub async fn persist_block_aggregates(
        &self,
        btc_address_type: String,
        block_aggregate: &BlockAggregateOutput,
    ) -> anyhow::Result<u64> {
        let table_name = format!("{}_utxo_block_aggregates", btc_address_type);
        let result = sqlx::query(&format!(
            "INSERT INTO {} VALUES(?1,?2,?3,?4,?5)",
            table_name
        ))
            .bind(block_aggregate.block_height as i64)
            .bind(&block_aggregate.block_hash_big_endian)
            .bind(&block_aggregate.date)
            .bind(block_aggregate.total_utxos as i64)
            .bind(block_aggregate.total_sats)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn get_latest_block_aggregates(
        &self,
        btc_address_type: Option<BtcAddressType>,
        num_blocks: Option<i64>
    ) -> anyhow::Result<Vec<BlockAggregateOutput>> {
        let btc_address_type = btc_address_type.unwrap_or(BtcAddressType::P2PK);
        let table_name = format!("{}_utxo_block_aggregates", btc_address_type.to_string().to_lowercase());
        let num_blocks = num_blocks.unwrap_or(10);
        
        let results = sqlx::query(&format!(
            "SELECT date, block_height, block_hash_big_endian, total_utxos, total_sats 
             FROM {} 
             WHERE block_height > (SELECT MAX(block_height) - $1 FROM {})
             ORDER BY block_height ASC",
            table_name, table_name
        ))
        .bind(num_blocks)
        .fetch_all(&self.pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|row| BlockAggregateOutput {
                date: row.get::<String, _>(0),
                block_height: row.get::<i64, _>(1) as usize,
                block_hash_big_endian: row.get::<String, _>(2),
                total_utxos: row.get::<i64, _>(3) as u32,
                total_sats: row.get::<f64, _>(4),
            })
            .collect())
    }

    pub async fn get_block_by_hash(
        &self,
        btc_address_type: String,
        hash: &str,
    ) -> anyhow::Result<Option<BlockAggregateOutput>> {
        let table_name = format!("{}_utxo_block_aggregates", btc_address_type);
        let result = sqlx::query(&format!(
            "SELECT date, block_height, block_hash_big_endian, total_utxos, total_sats 
             FROM {} WHERE block_hash_big_endian = ?",
            table_name
        ))
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(BlockAggregateOutput {
                date: row.get(0),
                block_height: row.get::<i64, _>(1) as usize,
                block_hash_big_endian: row.get(2),
                total_utxos: row.get::<i64, _>(3) as u32,
                total_sats: row.get::<f64, _>(4),
            })),
            None => Ok(None),
        }
    }

    pub async fn get_block_by_height(
        &self,
        btc_address_type: String,
        height: i64,
    ) -> anyhow::Result<Option<BlockAggregateOutput>> {
        let table_name = format!("{}_utxo_block_aggregates", btc_address_type);
        let result = sqlx::query(&format!(
            "SELECT date, block_height, block_hash_big_endian, total_utxos, total_sats 
             FROM {} WHERE block_height = ?",
            table_name
        ))
        .bind(height)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(BlockAggregateOutput {
                date: row.get(0),
                block_height: row.get::<i64, _>(1) as usize,
                block_hash_big_endian: row.get(2),
                total_utxos: row.get(3),
                total_sats: row.get(4),
            })),
            None => Ok(None),
        }
    }

    /* Returns the last block height in the database.
     * If the database is empty, returns None.
     */
    pub async fn get_last_block_height(&self, btc_address_type: String) -> anyhow::Result<Option<i64>> {
        let table_name = format!("{}_utxo_block_aggregates", btc_address_type);
        let result = sqlx::query(&format!(
            "SELECT MAX(block_height) as max_height FROM {}",
            table_name
        ))
        .fetch_optional(&self.pool)
        .await?;

        // For an empty table, result.get(0) will return None because MAX() returns NULL
        Ok(result.and_then(|row| row.get::<Option<i64>, _>("max_height")))
    }
}
