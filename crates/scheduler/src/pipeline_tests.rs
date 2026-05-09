#[cfg(test)]
mod tests {
    use crate::pipeline;
    use chrono::{Duration, Utc};
    use shared::models::CtfEvent;
    use shared::testing::{InMemoryCtfRepository, InMemoryGuildRepository};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_pipeline_new_event_inserts() {
        let event_repo = Arc::new(InMemoryCtfRepository::default());
        let guild_repo = Arc::new(InMemoryGuildRepository::default());

        let event = CtfEvent {
            ctftime_id: 1,
            title: "New CTF".to_string(),
            start_time: Utc::now() + Duration::days(1),
            end_time: Utc::now() + Duration::days(2),
            ..Default::default()
        };

        let stats = pipeline::process_events(
            std::slice::from_ref(&event),
            event_repo.as_ref(),
            guild_repo.as_ref(),
        )
        .await
        .unwrap();

        assert_eq!(stats.inserted, 1);
    }

    #[tokio::test]
    async fn test_pipeline_updated_event_updates() {
        let event_repo = Arc::new(InMemoryCtfRepository::default());
        let guild_repo = Arc::new(InMemoryGuildRepository::default());

        let event = CtfEvent {
            ctftime_id: 1,
            title: "Original Title".to_string(),
            start_time: Utc::now() + Duration::days(1),
            end_time: Utc::now() + Duration::days(2),
            ..Default::default()
        };

        // Initial insert
        pipeline::process_events(
            std::slice::from_ref(&event),
            event_repo.as_ref(),
            guild_repo.as_ref(),
        )
        .await
        .unwrap();

        // Update title
        let mut updated_event = event.clone();
        updated_event.title = "Updated Title".to_string();

        let stats = pipeline::process_events(
            std::slice::from_ref(&updated_event),
            event_repo.as_ref(),
            guild_repo.as_ref(),
        )
        .await
        .unwrap();

        assert_eq!(stats.updated, 1);
    }
}
