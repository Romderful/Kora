//! `PostgreSQL` storage layer.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Create a connection pool and run embedded migrations.
///
/// # Errors
///
/// Returns an error if the database is unreachable or migrations fail.
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
