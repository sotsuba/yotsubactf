use sqlx::PgPool;

#[sqlx::test(migrations = "../../migrations")]
async fn reminder_columns_exist(pool: PgPool) {
    // This query fails at compile time (via sqlx-data.json or DB check)
    // or runtime if the schema is mismatched.
    sqlx::query("SELECT id, user_id, kind FROM reminders LIMIT 0")
        .execute(&pool)
        .await
        .expect("reminders table schema mismatch");
}
