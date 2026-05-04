-- Stores "remind me before this CTF" requests from Discord users.
--
-- remind_at  = event start_time − 1 hour (set by the application).
-- sent_at    = NULL until the scheduler DMs the user; then stamped NOW().
-- Unique constraint on (user_id, ctftime_id) so clicking the button twice
-- is idempotent — the second click just gets the ephemeral "already set" reply.

CREATE TABLE reminders (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      TEXT        NOT NULL,                   -- Discord snowflake
    ctftime_id   BIGINT      NOT NULL REFERENCES ctf_events(ctftime_id) ON DELETE CASCADE,
    event_title  TEXT        NOT NULL,
    remind_at    TIMESTAMPTZ NOT NULL,                   -- start_time − 1 hour
    sent_at      TIMESTAMPTZ,                            -- NULL ⟹ pending
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Enforce exactly one pending reminder per (user, event).
CREATE UNIQUE INDEX reminders_one_per_user_event
    ON reminders (user_id, ctftime_id);

-- The scheduler polls this index every minute.
CREATE INDEX reminders_due_idx
    ON reminders (remind_at)
    WHERE sent_at IS NULL;
