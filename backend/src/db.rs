use sqlx::postgres::{PgPoolOptions, PgPool};
use std::time::Duration;

pub type DbPool = PgPool;

pub async fn create_pool(database_url: &str) -> Result<DbPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(30))
        .connect(database_url)
        .await
}
