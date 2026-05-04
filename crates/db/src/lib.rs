// The bot only needs the concrete repositories to build AppState in main.rs.
// We duplicate the thin Postgres impls here rather than creating a cross-binary
// dependency between scheduler and gateway.

pub mod command_logs;
pub mod events;
pub mod guilds;
pub mod reminders;
pub mod teams;
pub mod writeups;

pub use redis;
pub use sqlx::PgPool;

pub use command_logs::PostgresCommandLogRepository;
pub use events::PostgresCtfRepository;
pub use guilds::PostgresGuildRepository;
pub use reminders::PostgresReminderRepository;
pub use teams::PostgresTeamRepository;
pub use writeups::PostgresWriteupRepository;

pub async fn connect_and_migrate(url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPool::connect(url).await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}

pub(crate) fn db_err(e: sqlx::Error) -> shared::CtfError {
    shared::CtfError::Database(e.to_string())
}

pub(crate) fn redis_err(e: redis::RedisError) -> shared::CtfError {
    shared::CtfError::Database(format!("Redis error: {e}"))
}
