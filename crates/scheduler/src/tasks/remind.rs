use crate::SharedState;
use async_trait::async_trait;
use chrono::Utc;
use shared::CtfResult;
use shared::ReminderAdvanceResult;
use tracing::{error, info};

use super::SchedulerTask;

pub struct RemindTask;

#[async_trait]
impl SchedulerTask for RemindTask {
    fn name(&self) -> &'static str {
        "remind"
    }
    async fn run_once(&self, state: &SharedState) -> CtfResult<()> {
        run_once(state).await
    }
}

pub async fn run_once(state: &SharedState) -> CtfResult<()> {
    let now = Utc::now();
    let due = state.reminder_repo.fetch_due(now).await?;

    metrics::gauge!(shared::metrics::SCHEDULER_REMINDERS_PENDING).set(due.len() as f64);

    if due.is_empty() {
        return Ok(());
    }
    info!(count = due.len(), "Processing due reminders");

    for reminder in &due {
        let overdue_by = now - reminder.remind_at;
        let is_stale = overdue_by > shared::Reminder::STALENESS_THRESHOLD;

        if is_stale {
            info!(
                id = %reminder.id,
                overdue_minutes = overdue_by.num_minutes(),
                "Skipping stale reminder, advancing to next occurrence"
            );
            if let Err(e) = state.reminder_repo.advance_or_delete(reminder.id).await {
                error!(?e, id = %reminder.id, "advance_or_delete failed for stale reminder");
            }

            metrics::counter!(
                shared::metrics::SCHEDULER_REMINDERS_SKIPPED,
                "reason" => "stale"
            )
            .increment(1);
            continue;
        }

        match state.notifier.send_reminder_dm(reminder).await {
            Ok(_) => {
                metrics::counter!(
                    shared::metrics::SCHEDULER_REMINDERS_FIRED,
                    "kind"   => format!("{:?}", reminder.kind).to_lowercase(),
                    "result" => "ok"
                )
                .increment(1);

                match state.reminder_repo.advance_or_delete(reminder.id).await {
                    Ok(ReminderAdvanceResult::Deleted) => {
                        info!(id = %reminder.id, kind = ?reminder.kind, "Reminder sent and deleted");
                    }
                    Ok(ReminderAdvanceResult::Advanced {
                        next_remind_at,
                        sent_count,
                    }) => {
                        info!(
                            id             = %reminder.id,
                            sent_count,
                            next = %next_remind_at,
                            "Recurring reminder advanced"
                        );
                    }
                    Err(e) => {
                        error!(?e, id = %reminder.id, "advance_or_delete failed after successful send");
                    }
                }
            }
            Err(e) => {
                metrics::counter!(
                    shared::metrics::SCHEDULER_REMINDERS_FIRED,
                    "kind"   => format!("{:?}", reminder.kind).to_lowercase(),
                    "result" => "err"
                )
                .increment(1);

                error!(
                    ?e,
                    id         = %reminder.id,
                    user_id    = %reminder.user_id,
                    sent_count = reminder.sent_count,
                    remind_at  = %reminder.remind_at,
                    "Reminder DM failed — will retry"
                );
            }
        }
    }

    Ok(())
}
