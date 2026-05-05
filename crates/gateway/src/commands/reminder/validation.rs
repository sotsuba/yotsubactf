use chrono::{DateTime, Duration, Utc};

pub const MAX_FIRE_COUNT: i32 = 60;
#[allow(dead_code)]
pub const MAX_ACTIVE_RECURRING: i64 = 10;
pub const MAX_REMINDER_DAYS: i64 = 90;
pub const MAX_PENDING_REMINDERS: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum ReminderValidationError {
    #[error("'every_minutes' must be at least 1.")]
    IntervalTooShort,
    #[error("'every_minutes' cannot exceed 43200 (30 days).")]
    IntervalTooLong,
    #[error("'for_hours' must be between 1 and 720 (30 days).")]
    DurationOutOfRange,
    #[error("Would fire {0} times — max is 60. Use a longer interval or shorter duration.")]
    TooManyFires(i64),
    #[error("Delay must be less than the total duration.")]
    DelayExceedsDuration,
    #[error("Delay must be non-negative.")]
    DelayNegative,
    #[error("Message must be 200 characters or less.")]
    MessageTooLong,
    #[error("Reminder time is too far in the future (max 90 days).")]
    #[allow(dead_code)]
    TimeTooFar,
}

pub struct RecurringParams {
    pub for_hours: i64,
    pub every_minutes: i64,
    pub delay_minutes: i64, // 0 = fire at the first interval
    pub message: Option<String>,
}

pub struct ValidatedRecurring {
    pub interval_secs: i64,
    pub repeat_until: DateTime<Utc>,
    pub first_remind_at: DateTime<Utc>,
    pub fire_count: i32,
    pub message: Option<String>,
}

pub fn validate_recurring(
    params: RecurringParams,
    now: DateTime<Utc>,
) -> Result<ValidatedRecurring, ReminderValidationError> {
    let interval_secs = params.every_minutes * 60;
    let duration_secs = params.for_hours * 3600;
    let delay_secs = params.delay_minutes * 60;

    if params.every_minutes < 1 {
        return Err(ReminderValidationError::IntervalTooShort);
    }
    if params.every_minutes > 43200 {
        return Err(ReminderValidationError::IntervalTooLong);
    }
    if !(1..=720).contains(&params.for_hours) {
        return Err(ReminderValidationError::DurationOutOfRange);
    }
    if params.delay_minutes < 0 {
        return Err(ReminderValidationError::DelayNegative);
    }
    if delay_secs >= duration_secs {
        return Err(ReminderValidationError::DelayExceedsDuration);
    }
    if params
        .message
        .as_ref()
        .map(|m| m.chars().count())
        .unwrap_or(0)
        > 200
    {
        return Err(ReminderValidationError::MessageTooLong);
    }

    let repeat_until = now + Duration::seconds(duration_secs);
    let first_remind_at = now
        + Duration::seconds(if delay_secs > 0 {
            delay_secs
        } else {
            interval_secs
        });

    // Count = floor((repeat_until - first_remind_at) / interval) + 1
    let window_secs = repeat_until.timestamp() - first_remind_at.timestamp();
    if window_secs < 0 {
        return Err(ReminderValidationError::DelayExceedsDuration);
    }
    let fire_count = (window_secs / interval_secs) + 1;

    if fire_count > MAX_FIRE_COUNT as i64 {
        return Err(ReminderValidationError::TooManyFires(fire_count));
    }

    Ok(ValidatedRecurring {
        interval_secs,
        repeat_until,
        first_remind_at,
        fire_count: fire_count as i32,
        message: params.message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn base_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 1, 10, 0, 0).unwrap()
    }

    #[test]
    fn test_validate_recurring_happy_path() {
        let params = RecurringParams {
            for_hours: 10,
            every_minutes: 60,
            delay_minutes: 0,
            message: Some("test".to_string()),
        };
        let res = validate_recurring(params, base_now()).unwrap();
        assert_eq!(res.fire_count, 10);
        assert_eq!(res.interval_secs, 3600);
    }

    #[test]
    fn test_validate_recurring_interval_too_short() {
        let params = RecurringParams {
            for_hours: 10,
            every_minutes: 0,
            delay_minutes: 0,
            message: None,
        };
        assert!(matches!(
            validate_recurring(params, base_now()),
            Err(ReminderValidationError::IntervalTooShort)
        ));
    }

    #[test]
    fn test_validate_recurring_interval_too_long() {
        let params = RecurringParams {
            for_hours: 10,
            every_minutes: 43201,
            delay_minutes: 0,
            message: None,
        };
        assert!(matches!(
            validate_recurring(params, base_now()),
            Err(ReminderValidationError::IntervalTooLong)
        ));
    }

    #[test]
    fn test_validate_recurring_duration_out_of_range() {
        let params = RecurringParams {
            for_hours: 0,
            every_minutes: 60,
            delay_minutes: 0,
            message: None,
        };
        assert!(matches!(
            validate_recurring(params, base_now()),
            Err(ReminderValidationError::DurationOutOfRange)
        ));

        let params = RecurringParams {
            for_hours: 721,
            every_minutes: 60,
            delay_minutes: 0,
            message: None,
        };
        assert!(matches!(
            validate_recurring(params, base_now()),
            Err(ReminderValidationError::DurationOutOfRange)
        ));
    }

    #[test]
    fn test_validate_recurring_delay_exceeds_duration() {
        let params = RecurringParams {
            for_hours: 1,
            every_minutes: 10,
            delay_minutes: 60,
            message: None,
        };
        assert!(matches!(
            validate_recurring(params, base_now()),
            Err(ReminderValidationError::DelayExceedsDuration)
        ));
    }

    #[test]
    fn test_validate_recurring_too_many_fires() {
        let params = RecurringParams {
            for_hours: 61,
            every_minutes: 60,
            delay_minutes: 0,
            message: None,
        };
        let res = validate_recurring(params, base_now());
        assert!(matches!(
            res,
            Err(ReminderValidationError::TooManyFires(61))
        ));
    }

    #[test]
    fn test_validate_recurring_message_too_long() {
        let params = RecurringParams {
            for_hours: 1,
            every_minutes: 10,
            delay_minutes: 0,
            message: Some("a".repeat(201)),
        };
        assert!(matches!(
            validate_recurring(params, base_now()),
            Err(ReminderValidationError::MessageTooLong)
        ));
    }

    #[test]
    fn test_validate_recurring_fire_count_exact_boundary() {
        let params = RecurringParams {
            for_hours: 60,
            every_minutes: 60,
            delay_minutes: 0,
            message: None,
        };
        let res = validate_recurring(params, base_now()).unwrap();
        assert_eq!(res.fire_count, 60);
    }
}
