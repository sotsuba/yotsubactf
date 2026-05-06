-- Admin role mappings for RBAC

CREATE TABLE IF NOT EXISTS guild_admin_roles (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    guild_id   TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
    role_id    TEXT NOT NULL,
    role       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS guild_admin_roles_unique
    ON guild_admin_roles (guild_id, role_id);

CREATE INDEX IF NOT EXISTS guild_admin_roles_guild_idx
    ON guild_admin_roles (guild_id);
