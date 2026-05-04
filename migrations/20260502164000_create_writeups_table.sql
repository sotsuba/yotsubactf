-- Create writeups table
CREATE TABLE IF NOT EXISTS writeups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ctftime_id BIGINT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    event_id BIGINT NOT NULL,
    notified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
