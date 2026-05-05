use sqlx::PgPool;

pub async fn connect(url: &str) -> PgPool {
    PgPool::connect(url)
        .await
        .expect("failed to connect to database")
}
