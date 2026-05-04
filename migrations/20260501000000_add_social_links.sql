-- Add social_links as a JSONB array to ctf_events.
-- Each element has the shape: {"platform": "Discord", "url": "https://discord.gg/..."}
-- An empty array means no social links were found during enrichment.

ALTER TABLE ctf_events
    ADD COLUMN IF NOT EXISTS social_links JSONB NOT NULL DEFAULT '[]';

-- Partial index: quickly find events that have at least one social link.
CREATE INDEX IF NOT EXISTS idx_ctf_events_has_socials
    ON ctf_events ((social_links != '[]'::jsonb))
    WHERE social_links != '[]'::jsonb;
