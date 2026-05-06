-- Add enrichment and notification tracking to ctf_events
ALTER TABLE ctf_events
    ADD COLUMN IF NOT EXISTS enriched_at TIMESTAMP WITH TIME ZONE,
    ADD COLUMN IF NOT EXISTS notified_at TIMESTAMP WITH TIME ZONE;

-- Add enrichment tracking to writeups (it already has notified_at)
ALTER TABLE writeups
    ADD COLUMN IF NOT EXISTS enriched_at TIMESTAMP WITH TIME ZONE;

-- Create indexes to make the queue processing fast
CREATE INDEX IF NOT EXISTS idx_ctf_events_unenriched ON ctf_events (enriched_at) WHERE enriched_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_ctf_events_unnotified ON ctf_events (notified_at) WHERE notified_at IS NULL AND enriched_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_writeups_unenriched ON writeups (enriched_at) WHERE enriched_at IS NULL;
