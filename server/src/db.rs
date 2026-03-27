use sqlx::PgPool;

pub async fn connect() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPool::connect(&url)
        .await
        .expect("failed to connect to database")
}
