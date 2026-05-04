-- Fix team upsert: don't reset created_at, add updated_at
ALTER TABLE tracked_teams ADD COLUMN updated_at TIMESTAMPTZ DEFAULT NOW();
UPDATE tracked_teams SET updated_at = created_at;
