use sqlx::PgPool;

#[tokio::test]
async fn reminder_columns_exist() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&url).await.expect("Failed to connect");

    // This query fails at compile time (via sqlx-data.json or DB check)
    // or runtime if the schema is mismatched.
    sqlx::query("SELECT id, user_id, kind FROM reminders LIMIT 0")
        .execute(&pool)
        .await
        .expect("reminders table schema mismatch");
}
