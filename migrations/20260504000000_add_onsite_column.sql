-- Add is_onsite column to ctf_events table
ALTER TABLE ctf_events ADD COLUMN is_onsite BOOLEAN NOT NULL DEFAULT FALSE;
