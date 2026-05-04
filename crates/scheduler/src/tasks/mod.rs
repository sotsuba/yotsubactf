pub mod digest;
pub mod remind;
#[cfg(test)]
mod remind_tests;
pub mod results;
pub mod scrape;
pub mod writeups;

use crate::SharedState;
use async_trait::async_trait;
use shared::{CtfError, CtfResult};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

#[async_trait]
pub trait SchedulerTask: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    async fn run_once(&self, state: &SharedState) -> CtfResult<()>;
}

pub async fn run_task_loop(
    task: Arc<dyn SchedulerTask>,
    state: Arc<SharedState>,
    interval: Duration,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut current_interval = interval;
    let max_interval = Duration::from_secs(3600);

    info!(task = task.name(), "Task loop started");
    loop {
        ticker.tick().await;
        info!(task = task.name(), "Starting task cycle");

        let start = std::time::Instant::now();
        let result = task.run_once(&state).await;
        let duration = start.elapsed();

        metrics::histogram!(
            shared::metrics::SCHEDULER_TASK_DURATION,
            "task" => task.name()
        )
        .record(duration.as_secs_f64());

        match result {
            Ok(_) => {
                metrics::counter!(
                    shared::metrics::SCHEDULER_TASKS_TOTAL,
                    "task"   => task.name(),
                    "result" => "ok"
                )
                .increment(1);

                if current_interval != interval {
                    info!(
                        task = task.name(),
                        "Task successful, resetting interval to {:?}", interval
                    );
                    current_interval = interval;
                    ticker = tokio::time::interval(current_interval);
                    ticker.tick().await;
                }
            }
            Err(e) => {
                let error_kind = if matches!(e, CtfError::RateLimit { .. }) {
                    "ratelimit"
                } else {
                    "error"
                };

                metrics::counter!(
                    shared::metrics::SCHEDULER_TASKS_TOTAL,
                    "task"   => task.name(),
                    "result" => error_kind
                )
                .increment(1);

                if let CtfError::RateLimit { .. } = e {
                    warn!(task = task.name(), "Rate limit hit, backing off");
                    current_interval = std::cmp::min(current_interval * 2, max_interval);
                    ticker = tokio::time::interval(current_interval);
                    ticker.tick().await;
                } else {
                    error!(task = task.name(), ?e, "Task cycle failed");
                }
            }
        }
    }
}
