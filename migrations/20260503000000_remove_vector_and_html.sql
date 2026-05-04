-- Drop dead weight from the initial schema

ALTER TABLE ctf_events DROP COLUMN IF EXISTS raw_html;

DROP EXTENSION IF EXISTS vector;
