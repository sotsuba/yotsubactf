-- Track when the last digest was sent to prevent duplicates
ALTER TABLE subscriptions
  ADD COLUMN last_digest_sent_at DATE;
