# Contributing to YotsubaCTF

Thank you for your interest in contributing! This document covers everything you need to get started.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Commit Convention](#commit-convention)
- [Pull Request Process](#pull-request-process)

## Development Setup

### Prerequisites

- Rust (latest stable)
- Docker & Docker Compose
- `sqlx-cli`: `cargo install sqlx-cli --no-default-features --features postgres`
- `git-cliff` (optional, for changelog): `cargo install git-cliff`

### First-time setup

```bash
# 1. Fork and clone the repo
git clone https://github.com/sotsuba/yotsubactf.git
cd yotsubactf

# 2. Install git hooks (required)
make hooks

# 3. Copy and fill in environment variables
cp .env.example .env

# 4. Start infrastructure (Postgres + Redis)
make db-up

# 5. Run migrations
make db-migrate
```

### Running the bot locally

```bash
# Run both services in parallel
make dev

# Or individually
make run-gateway
make run-scheduler
```

### Running tests

```bash
cargo test --workspace
```

## Project Structure

```
crates/
├── gateway/    # Discord bot — slash commands, interaction handler
├── scheduler/  # Background tasks — scraping, reminders, results
├── db/         # PostgreSQL repository implementations (SQLx)
└── shared/     # Common models, traits, error types, utilities
migrations/     # SQL migration files (applied in order on startup)
monitoring/     # Prometheus alerts and Grafana dashboard JSON
```

When adding a new slash command, the entry point is `crates/gateway/src/commands/`.
Implement the `SlashCommand` trait and register the command in `CommandRegistry::new()`.

### Admin roles (RBAC)

Admin-only commands can declare a required admin level via `required_admin_role()`.
The gateway enforces this on top of `MANAGE_GUILD`.

Admin levels (highest to lowest):

- owner
- admin
- moderator
- analyst

If a guild has no admin role mappings configured, the bot falls back to
`MANAGE_GUILD` only.

Use `/adminrole` to manage mappings (add/remove/list). The command expects
Discord role IDs and maps them to an admin level.

## Making Changes

### SQL queries

If you add or modify any `sqlx::query!` macro calls, you **must** regenerate
the offline data before committing:

```bash
make prepare
git add .sqlx/
```

Forgetting this will cause CI to fail on the "Verify sqlx offline data is synced" step.

### Adding a migration

Create a new file in `migrations/` following the existing naming pattern:

```
migrations/YYYYMMDDHHMMSS_describe_change.sql
```

Migrations run automatically on bot startup. Make them idempotent where possible (`CREATE TABLE IF NOT EXISTS`, etc.).

### Metrics

If you add a new background task or command, consider adding a Prometheus counter or histogram. Existing patterns are in `crates/shared/src/metrics.rs`.

## Commit Convention

This project enforces [Conventional Commits](https://www.conventionalcommits.org/).
The `commit-msg` hook will reject non-conforming messages.

```
<type>(<scope>): <description>

Types: feat, fix, perf, refactor, docs, ci, chore, style, test
Scope: gateway, scheduler, db, shared, migrations, monitoring (optional)

Examples:
  feat(gateway): add /leaderboard pagination
  fix(reminder): handle timezone edge case in set_timer
  ci: skip workflow on doc-only changes
```

## Pull Request Process

1. Branch off `dev`, not `main`.
2. Keep PRs focused — one feature or fix per PR.
3. Make sure CI passes before requesting review.
4. Fill in the PR template checklist, especially the `make prepare` item if you touched SQL.
5. PRs are merged into `dev` first, then `dev` → `main` for releases.