use super::SchedulerTask;
use crate::SharedState;
use async_trait::async_trait;
use shared::CtfResult;
use tracing::info;

pub struct ScrapeTask;

#[async_trait]
impl SchedulerTask for ScrapeTask {
    fn name(&self) -> &'static str {
        "scrape"
    }
    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        run_once(state).await
    }
}

pub async fn run_once(state: &SharedState) -> CtfResult<()> {
    let stats = crate::pipeline::run_once(
        &state.http,
        &*state.event_repo,
        &*state.guild_repo,
        &*state.notifier,
    )
    .await?;
    info!(?stats, "Scrape complete");
    Ok(())
}
