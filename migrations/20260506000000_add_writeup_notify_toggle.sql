-- Phase 2: Writeup Notification Toggle
-- Adds a boolean flag to guilds to allow opting in/out of writeup notifications.

ALTER TABLE guilds ADD COLUMN notify_writeups BOOLEAN NOT NULL DEFAULT TRUE;
