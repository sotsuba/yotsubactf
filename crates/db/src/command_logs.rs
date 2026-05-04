use async_trait::async_trait;
use shared::contracts::CommandLogRepository;
use shared::error::CtfResult;
use sqlx::PgPool;

pub struct PostgresCommandLogRepository {
    pool: PgPool,
}

impl PostgresCommandLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CommandLogRepository for PostgresCommandLogRepository {
    async fn log_command(
        &self,
        user_id: &str,
        guild_id: Option<&str>,
        command_name: &str,
        kind: &str,
        success: bool,
        latency_ms: i64,
    ) -> CtfResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO command_logs (user_id, guild_id, command_name, kind, success, latency_ms)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            user_id,
            guild_id,
            command_name,
            kind,
            success,
            latency_ms
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
