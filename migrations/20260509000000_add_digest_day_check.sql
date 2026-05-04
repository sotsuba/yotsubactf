ALTER TABLE subscriptions ADD CONSTRAINT chk_digest_day CHECK (digest_day_utc BETWEEN 0 AND 6);
