-- Command analytics log
-- Stores every interaction for historical analysis in Metabase.

CREATE TABLE command_logs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         TEXT NOT NULL,
    guild_id        TEXT,
    command_name    TEXT NOT NULL,
    kind            TEXT NOT NULL, -- slash, component
    success         BOOLEAN NOT NULL,
    latency_ms      BIGINT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_command_logs_created_at ON command_logs (created_at);
CREATE INDEX idx_command_logs_user_id ON command_logs (user_id);
CREATE INDEX idx_command_logs_command_name ON command_logs (command_name);
