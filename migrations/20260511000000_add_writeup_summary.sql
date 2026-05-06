-- Add summary column to writeups table
ALTER TABLE writeups
    ADD COLUMN IF NOT EXISTS summary TEXT;
