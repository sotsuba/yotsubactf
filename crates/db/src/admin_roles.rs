use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::{AdminRole, AdminRoleAssignment, AdminRoleRepository, CtfError, CtfResult as Result};
use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow)]
struct DbAdminRole {
    guild_id: String,
    role_id: String,
    role: String,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
}

pub struct PostgresAdminRoleRepository {
    pool: PgPool,
}

impl PostgresAdminRoleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn parse_role(role: &str) -> Result<AdminRole> {
    role.parse::<AdminRole>().map_err(|_| {
        CtfError::Database(format!("Invalid admin role value in database: {role}"))
    })
}

#[async_trait]
impl AdminRoleRepository for PostgresAdminRoleRepository {
    async fn list_admin_roles(&self, guild_id: &str) -> Result<Vec<AdminRoleAssignment>> {
        let rows = sqlx::query_as::<_, DbAdminRole>(
            r#"SELECT guild_id, role_id, role, created_at
               FROM guild_admin_roles
               WHERE guild_id = $1
               ORDER BY role, role_id"#,
        )
        .bind(guild_id)
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        let mut roles = Vec::with_capacity(rows.len());
        for row in rows {
            roles.push(AdminRoleAssignment {
                guild_id: row.guild_id,
                role_id: row.role_id,
                role: parse_role(&row.role)?,
            });
        }

        Ok(roles)
    }

    async fn upsert_admin_role(
        &self,
        guild_id: &str,
        role_id: &str,
        role: AdminRole,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO guild_admin_roles (guild_id, role_id, role)
               VALUES ($1, $2, $3)
               ON CONFLICT (guild_id, role_id) DO UPDATE
               SET role = EXCLUDED.role"#,
        )
        .bind(guild_id)
        .bind(role_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn delete_admin_role(&self, guild_id: &str, role_id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"DELETE FROM guild_admin_roles WHERE guild_id = $1 AND role_id = $2"#,
        )
        .bind(guild_id)
        .bind(role_id)
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(result.rows_affected() > 0)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
