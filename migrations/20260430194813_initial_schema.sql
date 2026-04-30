-- Add migration script here

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE ctf_events (
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
    raw_html    TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctf_events_start_time ON ctf_events (start_time);
CREATE INDEX idx_ctf_events_ctftime_id ON ctf_events (ctftime_id);

CREATE TABLE guilds (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    guild_id    BIGINT UNIQUE NOT NULL,
    channel_id  TEXT,
    prefs       JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


CREATE TABLE subscriptions (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    guild_id      UUID NOT NULL REFERENCES guilds(id) ON DELETE CASCADE,
    active        BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);