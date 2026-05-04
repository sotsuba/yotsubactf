-- Guard rail: ensure social_links can never be set to SQL NULL.
-- (The column already has NOT NULL DEFAULT '[]', this is belt-and-suspenders
-- in case any external tool or migration accidentally set it to NULL.)
UPDATE ctf_events
   SET social_links = '[]'::jsonb
 WHERE social_links IS NULL;

-- Re-confirm the constraint is in place.
ALTER TABLE ctf_events
    ALTER COLUMN social_links SET NOT NULL,
    ALTER COLUMN social_links SET DEFAULT '[]'::jsonb;
