# Admin Roles (RBAC)

## Status
Implemented

## Context
The bot has grown beyond a single admin-only permission gate. Relying only on
Discord's MANAGE_GUILD makes it hard to delegate responsibilities safely in
larger guilds (e.g., allow analytics access without allowing configuration
changes). We need a lightweight role-based access control layer that still
respects Discord's native permission model.

## Decision
Introduce admin role mappings at the guild level:

- Define admin levels: owner, admin, moderator, analyst.
- Map Discord role IDs to an admin level via a new table.
- Commands can declare a required level using `required_admin_role()`.
- The gateway enforces `MANAGE_GUILD` first, then checks the mapped admin
  roles for the required level.
- If no mappings exist for a guild, fall back to `MANAGE_GUILD` only to
  avoid lockouts.

## Consequences

**Benefits:**
- Delegated permissions without granting full MANAGE_GUILD power.
- Consistent, centralized checks across gateway commands.
- Backward-compatible rollout with a safe fallback path.

**Trade-offs:**
- Extra configuration step per guild to enable RBAC.
- Slightly more complex permission checks and data storage.

## Alternatives Considered
- Use only Discord permissions and add more permission types. This is limited
  by Discord's permission surface and does not allow fine-grained delegation.
- Store role names instead of role IDs. Names are mutable and ambiguous across
  guilds, so IDs are safer.
