#[cfg(test)]
mod tests {
    use crate::SharedState;
    use crate::tasks::SchedulerTask;
    use crate::tasks::remind::RemindTask;
    use chrono::{Duration, Utc};
    use shared::models::{Reminder, ReminderKind};

    #[tokio::test]
    async fn test_remind_fires_dm_and_deletes_oneshot() {
        let state = SharedState::new_mock();
        let now = Utc::now();

        let r1 = Reminder {
            user_id: "u1".to_string(),
            kind: ReminderKind::Timer,
            message: Some("Wake up!".to_string()),
            remind_at: now - Duration::minutes(1),
            ..Default::default()
        };
        state.reminder_repo.create(&r1).await.unwrap();

        let task = RemindTask;
        task.run_once(&state).await.unwrap();

        // Check notifier
        let notifier = state
            .notifier
            .as_any()
            .downcast_ref::<shared::testing::MockNotifier>()
            .unwrap();
        let sent = notifier.sent_reminders.read().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].user_id, "u1");

        // Check DB (should be deleted)
        let pending = state.reminder_repo.list_pending("u1", None).await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_remind_fires_dm_and_advances_recurring() {
        let state = SharedState::new_mock();
        let now = Utc::now();

        let r1 = Reminder {
            user_id: "u2".to_string(),
            kind: ReminderKind::Recurring,
            message: Some("Every hour".to_string()),
            remind_at: now - Duration::minutes(1),
            interval_secs: Some(3600),
            repeat_until: Some(now + Duration::days(1)),
            ..Default::default()
        };
        state.reminder_repo.create(&r1).await.unwrap();

        let task = RemindTask;
        task.run_once(&state).await.unwrap();

        // Check notifier
        let notifier = state
            .notifier
            .as_any()
            .downcast_ref::<shared::testing::MockNotifier>()
            .unwrap();
        let sent = notifier.sent_reminders.read().await;
        assert_eq!(sent.len(), 1);

        // Check DB (should be advanced)
        let pending = state.reminder_repo.list_pending("u2", None).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert!(pending[0].remind_at > now);
        assert_eq!(pending[0].sent_count, 1);
    }
}
