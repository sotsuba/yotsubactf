#[cfg(test)]
mod tests {
    use crate::commands::reminder::common::create_event_reminder;
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use shared::contracts::{ReminderRepository, WriteCtfRepository};
    use shared::models::CtfEvent;
    use shared::testing::{InMemoryCtfRepository, InMemoryReminderRepository};
    use twilight_model::channel::message::MessageFlags;
    use twilight_model::http::interaction::{InteractionResponse, InteractionResponseType};

    fn base_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 10, 0, 0).unwrap()
    }

    async fn setup_repos() -> (InMemoryCtfRepository, InMemoryReminderRepository) {
        let event_repo = InMemoryCtfRepository::default();
        let reminder_repo = InMemoryReminderRepository::default();

        let event = CtfEvent {
            ctftime_id: 1234,
            title: "Test CTF".to_string(),
            start_time: base_now() + Duration::hours(5),
            end_time: base_now() + Duration::hours(48),
            ..Default::default()
        };
        event_repo.upsert_event(&event).await.unwrap();

        (event_repo, reminder_repo)
    }

    #[tokio::test]
    async fn test_create_event_reminder_success() {
        let (event_repo, reminder_repo) = setup_repos().await;
        let res =
            create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 1234, 3600)
                .await
                .unwrap();

        if let InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        } = res
        {
            assert!(data.content.unwrap().contains("Reminder set!"));
            assert_eq!(data.flags, Some(MessageFlags::EPHEMERAL));
        } else {
            panic!("Unexpected response type");
        }

        let pending = reminder_repo.list_pending("user1", None).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].ctftime_id, Some(1234));
    }

    #[tokio::test]
    async fn test_create_event_reminder_not_found() {
        let (event_repo, reminder_repo) = setup_repos().await;
        let res =
            create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 9999, 3600)
                .await
                .unwrap();

        if let InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        } = res
        {
            assert!(data.content.unwrap().contains("Event not found"));
        } else {
            panic!("Unexpected response type");
        }
    }

    #[tokio::test]
    async fn test_create_event_reminder_already_ended() {
        let (event_repo, reminder_repo) = setup_repos().await;
        let event = CtfEvent {
            ctftime_id: 5555,
            title: "Ended CTF".to_string(),
            start_time: base_now() - Duration::hours(48),
            end_time: base_now() - Duration::hours(1),
            ..Default::default()
        };
        event_repo.upsert_event(&event).await.unwrap();

        let res =
            create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 5555, 3600)
                .await
                .unwrap();
        assert!(format!("{:?}", res).contains("already ended"));
    }

    #[tokio::test]
    async fn test_create_event_reminder_already_started() {
        let (event_repo, reminder_repo) = setup_repos().await;
        let event = CtfEvent {
            ctftime_id: 6666,
            title: "Started CTF".to_string(),
            start_time: base_now() - Duration::hours(1),
            end_time: base_now() + Duration::hours(24),
            ..Default::default()
        };
        event_repo.upsert_event(&event).await.unwrap();

        let res =
            create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 6666, 3600)
                .await
                .unwrap();
        assert!(format!("{:?}", res).contains("already started"));
    }

    #[tokio::test]
    async fn test_create_event_reminder_duplicate() {
        let (event_repo, reminder_repo) = setup_repos().await;

        // First one
        create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 1234, 3600)
            .await
            .unwrap();

        // Second one
        let res =
            create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 1234, 3600)
                .await
                .unwrap();
        assert!(format!("{:?}", res).contains("already have a reminder set"));
    }

    #[tokio::test]
    async fn test_create_event_reminder_quota_exceeded() {
        let (event_repo, reminder_repo) = setup_repos().await;

        for i in 0..10 {
            let event = CtfEvent {
                ctftime_id: i,
                title: format!("CTF {}", i),
                start_time: base_now() + Duration::hours(100),
                end_time: base_now() + Duration::hours(200),
                ..Default::default()
            };
            event_repo.upsert_event(&event).await.unwrap();
            create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", i, 3600)
                .await
                .unwrap();
        }

        let event_11 = CtfEvent {
            ctftime_id: 11,
            title: "CTF 11".to_string(),
            start_time: base_now() + Duration::hours(100),
            end_time: base_now() + Duration::hours(200),
            ..Default::default()
        };
        event_repo.upsert_event(&event_11).await.unwrap();

        let res = create_event_reminder(base_now(), &event_repo, &reminder_repo, "user1", 11, 3600)
            .await
            .unwrap();
        let res_str = format!("{:?}", res);
        assert!(
            res_str.contains("maximum of **10 pending reminders**"),
            "Response was: {}",
            res_str
        );
    }
}
