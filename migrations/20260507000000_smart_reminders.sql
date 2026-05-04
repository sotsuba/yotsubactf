-- Smart Reminders migration
-- Replaces the existing simple reminders table with a more flexible structure
-- supporting one-shot events, timers, and recurring reminders.

DROP TABLE IF EXISTS reminders;

CREATE TABLE reminders (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         TEXT NOT NULL,

    -- Discriminator
    kind            TEXT NOT NULL,
    CONSTRAINT chk_kind CHECK (kind IN ('event', 'timer', 'recurring')),

    -- Event-linked (kind = 'event')
    ctftime_id      BIGINT,
    event_title     TEXT,
    event_start_at  TIMESTAMPTZ,

    -- Human message (kind = 'timer' | 'recurring')
    message         TEXT,
    CONSTRAINT chk_message_length CHECK (char_length(message) <= 200),

    -- Scheduling
    remind_at       TIMESTAMPTZ NOT NULL,

    -- Recurring only
    interval_secs   BIGINT,
    repeat_until    TIMESTAMPTZ,
    fire_count_max  INT,        -- precomputed at creation, for display

    CONSTRAINT chk_recurring_fields CHECK (
        kind != 'recurring' OR (
            interval_secs IS NOT NULL AND
            repeat_until  IS NOT NULL AND
            fire_count_max IS NOT NULL
        )
    ),
    CONSTRAINT chk_interval_min CHECK (
        interval_secs IS NULL OR interval_secs >= 60   -- min 1 min
    ),
    CONSTRAINT chk_interval_max CHECK (
        interval_secs IS NULL OR interval_secs <= 2592000   -- max 30 days
    ),

    -- Lifecycle
    sent_count      INT NOT NULL DEFAULT 0,
    last_sent_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Scheduler poll — only rows that should fire
CREATE INDEX idx_reminders_due
    ON reminders (remind_at)
    WHERE last_sent_at IS NULL
       OR (interval_secs IS NOT NULL AND remind_at <= repeat_until);

-- List query per user
CREATE INDEX idx_reminders_user_pending
    ON reminders (user_id, remind_at ASC)
    WHERE last_sent_at IS NULL
       OR (interval_secs IS NOT NULL AND remind_at <= repeat_until);

-- Count active recurring per user (for cap enforcement)
CREATE INDEX idx_reminders_recurring_user
    ON reminders (user_id)
    WHERE kind = 'recurring'
      AND remind_at <= repeat_until;
