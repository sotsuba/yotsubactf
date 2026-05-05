-- Add migration script here

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS ctf_events (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    ctftime_id  BIGINT UNIQUE NOT NULL,
    title       TEXT NOT NULL,  
    url         TEXT NOT NULL,
    start_time  TIMESTAMPTZ NOT NULL,
    end_time    TIMESTAMPTZ NOT NULL,
    weight      DOUBLE PRECISION, 
    format      TEXT,
    organiser   TEXT,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ctf_events_start_time ON ctf_events (start_time);
CREATE INDEX IF NOT EXISTS idx_ctf_events_ctftime_id ON ctf_events (ctftime_id);

-- ── Guilds ────────────────────────────────────────────────────────────────────
-- One row per Discord guild that has ever interacted with the bot.
-- prefs is a free-form JSONB bag; typed columns can be promoted out of it
-- later without a schema migration.

CREATE TABLE IF NOT EXISTS guilds (
    guild_id    TEXT        PRIMARY KEY,                 -- Discord snowflake stored as text
    prefs       JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Keep updated_at current automatically.
DROP FUNCTION IF EXISTS touch_updated_at CASCADE;
CREATE OR REPLACE FUNCTION touch_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS guilds_touch_updated_at ON guilds;
CREATE TRIGGER guilds_touch_updated_at
    BEFORE UPDATE ON guilds
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- ── Subscriptions ─────────────────────────────────────────────────────────────
-- A subscription means "post new CTF events into this channel".
-- Rows are never hard-deleted; deleted_at marks a soft-delete.
-- The partial unique index enforces at most one *active* subscription per guild.

CREATE TABLE IF NOT EXISTS subscriptions (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    guild_id    TEXT        NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    channel_id  TEXT        NOT NULL,                    -- Discord channel snowflake
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at  TIMESTAMPTZ                              -- NULL ⟹ active
);

-- One active subscription per guild at a time.
CREATE UNIQUE INDEX IF NOT EXISTS subscriptions_one_active_per_guild
    ON subscriptions (guild_id)
    WHERE deleted_at IS NULL;

-- Efficient lookup of all active subscriptions (used by the notifier fan-out).
CREATE INDEX IF NOT EXISTS subscriptions_active_idx
    ON subscriptions (guild_id)
    WHERE deleted_at IS NULL;