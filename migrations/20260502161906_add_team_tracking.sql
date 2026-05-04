-- Phase 2: Team Result Tracking
-- Adds tables for following CTFTime teams and recording their results.

CREATE TABLE tracked_teams (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    guild_id        TEXT        NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    ctftime_team_id BIGINT      NOT NULL,
    team_name       TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- 1 team per guild for now; drop this constraint to allow multiple.
    UNIQUE (guild_id)
);

CREATE INDEX idx_tracked_teams_team_id ON tracked_teams (ctftime_team_id);

CREATE TABLE team_results (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    ctftime_team_id BIGINT      NOT NULL,
    ctf_event_id    BIGINT      NOT NULL,   -- ctftime_id of the event (not our UUID)
    place           INT,
    score           DOUBLE PRECISION,
    total_teams     INT,
    notified_at     TIMESTAMPTZ,            -- NULL = not yet notified
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Dedup: one result per (team, event)
    UNIQUE (ctftime_team_id, ctf_event_id)
);

CREATE INDEX idx_team_results_team_id    ON team_results (ctftime_team_id);
CREATE INDEX idx_team_results_event_id   ON team_results (ctf_event_id);
CREATE INDEX idx_team_results_unnotified ON team_results (ctftime_team_id) WHERE notified_at IS NULL;
