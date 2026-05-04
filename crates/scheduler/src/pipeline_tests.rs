#[cfg(test)]
mod tests {
    use crate::SharedState;
    use crate::pipeline;
    use chrono::{Duration, Utc};
    use shared::contracts::{CtfEventRepository, GuildRepository, Notifier, WriteCtfRepository};
    use shared::models::{CtfEvent, UpsertStatus};
    use shared::testing::{InMemoryCtfRepository, InMemoryGuildRepository, MockNotifier};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_pipeline_new_event_notifies() {
        let event_repo = Arc::new(InMemoryCtfRepository::default());
        let guild_repo = Arc::new(InMemoryGuildRepository::default());
        let notifier = Arc::new(MockNotifier::default());

        // Subscribe a guild
        guild_repo.subscribe("guild1", "channel1").await.unwrap();

        let event = CtfEvent {
            ctftime_id: 1,
            title: "New CTF".to_string(),
            start_time: Utc::now() + Duration::days(1),
            end_time: Utc::now() + Duration::days(2),
            ..Default::default()
        };

        let stats = pipeline::process_events(
            &[event.clone()],
            event_repo.as_ref(),
            guild_repo.as_ref(),
            notifier.as_ref(),
        )
        .await
        .unwrap();

        assert_eq!(stats.inserted, 1);
        assert_eq!(stats.notified, 1);

        let sent = notifier.sent_events.read().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0.ctftime_id, 1);
        assert_eq!(sent[0].1, vec!["channel1".to_string()]);
    }

    #[tokio::test]
    async fn test_pipeline_updated_event_silent() {
        let event_repo = Arc::new(InMemoryCtfRepository::default());
        let guild_repo = Arc::new(InMemoryGuildRepository::default());
        let notifier = Arc::new(MockNotifier::default());

        guild_repo.subscribe("guild1", "channel1").await.unwrap();

        let event = CtfEvent {
            ctftime_id: 1,
            title: "Original Title".to_string(),
            start_time: Utc::now() + Duration::days(1),
            end_time: Utc::now() + Duration::days(2),
            ..Default::default()
        };

        // Initial insert
        pipeline::process_events(
            &[event.clone()],
            event_repo.as_ref(),
            guild_repo.as_ref(),
            notifier.as_ref(),
        )
        .await
        .unwrap();

        // Update title
        let mut updated_event = event.clone();
        updated_event.title = "Updated Title".to_string();

        let stats = pipeline::process_events(
            &[updated_event],
            event_repo.as_ref(),
            guild_repo.as_ref(),
            notifier.as_ref(),
        )
        .await
        .unwrap();

        assert_eq!(stats.updated, 1);
        assert_eq!(stats.notified, 0); // Updates should be silent

        let sent = notifier.sent_events.read().await;
        assert_eq!(sent.len(), 1); // Still only the first notification
    }

    #[tokio::test]
    async fn test_pipeline_notifier_fail_continues() {
        let event_repo = Arc::new(InMemoryCtfRepository::default());
        let guild_repo = Arc::new(InMemoryGuildRepository::default());
        let notifier = Arc::new(MockNotifier::default());

        guild_repo.subscribe("guild1", "channel1").await.unwrap();

        let e1 = CtfEvent {
            ctftime_id: 1,
            title: "E1".into(),
            ..Default::default()
        };
        let e2 = CtfEvent {
            ctftime_id: 2,
            title: "E2".into(),
            ..Default::default()
        };

        notifier
            .fail_next
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let stats = pipeline::process_events(
            &[e1, e2],
            event_repo.as_ref(),
            guild_repo.as_ref(),
            notifier.as_ref(),
        )
        .await
        .unwrap();

        assert_eq!(stats.inserted, 2);
        assert_eq!(stats.errors, 1); // One notification failed
        assert_eq!(stats.notified, 1); // The other succeeded
    }
}
