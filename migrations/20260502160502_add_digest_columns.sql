-- Add digest columns to subscriptions table

ALTER TABLE subscriptions
  ADD COLUMN digest_enabled    BOOLEAN  NOT NULL DEFAULT false,
  ADD COLUMN digest_channel_id TEXT,
  ADD COLUMN digest_day_utc    SMALLINT NOT NULL DEFAULT 1;  -- 0=Sun..6=Sat
