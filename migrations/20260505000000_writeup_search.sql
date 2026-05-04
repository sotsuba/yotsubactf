-- Add columns to writeups table
ALTER TABLE writeups
    ADD COLUMN IF NOT EXISTS category     TEXT,
    ADD COLUMN IF NOT EXISTS event_name   TEXT,
    ADD COLUMN IF NOT EXISTS published_at TIMESTAMPTZ;

-- Full-text search vector (generated column)
ALTER TABLE writeups
    ADD COLUMN IF NOT EXISTS search_vector tsvector
    GENERATED ALWAYS AS (
        to_tsvector('english',
            coalesce(title,      '') || ' ' ||
            coalesce(event_name, '') || ' ' ||
            coalesce(category,   '')
        )
    ) STORED;

-- GIN index for FTS
CREATE INDEX IF NOT EXISTS writeups_fts_gin ON writeups USING GIN(search_vector);

-- Trigram index for fuzzy search (ILIKE)
-- IMPORTANT: This requires pg_trgm extension. 
-- On managed DBs, you might need to enable this manually.
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS writeups_title_trgm ON writeups USING GIN(title gin_trgm_ops);

-- Standard indexes for filtering and sorting
CREATE INDEX IF NOT EXISTS writeups_category ON writeups (category) WHERE category IS NOT NULL;
CREATE INDEX IF NOT EXISTS writeups_event_id ON writeups (event_id)  WHERE event_id > 0;
CREATE INDEX IF NOT EXISTS writeups_published ON writeups (published_at DESC);
