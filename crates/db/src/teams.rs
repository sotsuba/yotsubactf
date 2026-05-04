use async_trait::async_trait;
use shared::CtfResult as Result;
use sqlx::PgPool;
use uuid::Uuid;

use shared::{TeamRepository, TeamResult, TrackedTeam};

pub struct PostgresTeamRepository {
    pool: PgPool,
}

impl PostgresTeamRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TeamRepository for PostgresTeamRepository {
    async fn follow_team(&self, guild_id: &str, team_id: i64, team_name: &str) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO tracked_teams (guild_id, ctftime_team_id, team_name)
               VALUES ($1, $2, $3)
               ON CONFLICT (guild_id) 
               DO UPDATE SET ctftime_team_id = EXCLUDED.ctftime_team_id, 
                             team_name = EXCLUDED.team_name,
                             updated_at = NOW()"#,
            guild_id,
            team_id,
            team_name
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn unfollow_team(&self, guild_id: &str) -> Result<bool> {
        let res = sqlx::query!("DELETE FROM tracked_teams WHERE guild_id = $1", guild_id)
            .execute(&self.pool)
            .await
            .map_err(crate::db_err)?;
        Ok(res.rows_affected() > 0)
    }

    async fn get_followed_team(&self, guild_id: &str) -> Result<Option<TrackedTeam>> {
        let row = sqlx::query!(
            "SELECT id, guild_id, ctftime_team_id, team_name, created_at, updated_at FROM tracked_teams WHERE guild_id = $1",
            guild_id
        )
        .fetch_optional(&self.pool)
        .await.map_err(crate::db_err)?;

        Ok(row.map(|r| TrackedTeam {
            id: r.id,
            guild_id: r.guild_id,
            ctftime_team_id: r.ctftime_team_id,
            team_name: r.team_name,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    async fn upsert_result(&self, result: &TeamResult) -> Result<bool> {
        let res = sqlx::query!(
            r#"INSERT INTO team_results (ctftime_team_id, ctf_event_id, place, score, total_teams)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (ctftime_team_id, ctf_event_id) DO NOTHING"#,
            result.ctftime_team_id,
            result.ctf_event_id,
            result.place,
            result.score,
            result.total_teams,
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(res.rows_affected() > 0)
    }

    async fn mark_result_notified(&self, id: Uuid) -> Result<()> {
        sqlx::query!(
            "UPDATE team_results SET notified_at = NOW() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(())
    }

    async fn list_recent_results(&self, team_id: i64, limit: i64) -> Result<Vec<TeamResult>> {
        let rows = sqlx::query!(
            r#"SELECT id, ctftime_team_id, ctf_event_id, place, score, total_teams, notified_at, created_at
               FROM team_results WHERE ctftime_team_id = $1
               ORDER BY created_at DESC LIMIT $2"#,
            team_id, limit
        )
        .fetch_all(&self.pool)
        .await.map_err(crate::db_err)?;

        Ok(rows
            .into_iter()
            .map(|r| TeamResult {
                id: r.id,
                ctftime_team_id: r.ctftime_team_id,
                ctf_event_id: r.ctf_event_id,
                place: r.place,
                score: r.score,
                total_teams: r.total_teams,
                notified_at: r.notified_at,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn list_guilds_tracking_team(&self, team_id: i64) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT guild_id FROM tracked_teams WHERE ctftime_team_id = $1",
            team_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;
        Ok(rows.into_iter().map(|r| r.guild_id).collect())
    }

    async fn list_unnotified_results(&self) -> Result<Vec<(TeamResult, Vec<String>)>> {
        let rows = sqlx::query!(
            r#"SELECT r.id, r.ctftime_team_id, r.ctf_event_id, r.place, r.score, r.total_teams,
                      r.notified_at, r.created_at,
                      array_agg(t.guild_id) FILTER (WHERE t.guild_id IS NOT NULL) as guild_ids
               FROM team_results r
               LEFT JOIN tracked_teams t ON t.ctftime_team_id = r.ctftime_team_id
               WHERE r.notified_at IS NULL
               GROUP BY r.id"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(crate::db_err)?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let result = TeamResult {
                    id: r.id,
                    ctftime_team_id: r.ctftime_team_id,
                    ctf_event_id: r.ctf_event_id,
                    place: r.place,
                    score: r.score,
                    total_teams: r.total_teams,
                    notified_at: r.notified_at,
                    created_at: r.created_at,
                };
                let guilds: Vec<String> = r.guild_ids.unwrap_or_default();
                (result, guilds)
            })
            .collect())
    }

    async fn list_all_tracked_team_ids(&self) -> Result<Vec<i64>> {
        let rows = sqlx::query!("SELECT DISTINCT ctftime_team_id FROM tracked_teams")
            .fetch_all(&self.pool)
            .await
            .map_err(crate::db_err)?;
        Ok(rows.into_iter().map(|r| r.ctftime_team_id).collect())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
