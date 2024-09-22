use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;

pub async fn create_connection(url: &str) -> Arc<sqlx::Pool<sqlx::Postgres>> {
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(url)
        .await
        .unwrap();

    Arc::new(db)
}
