# Contributing to YotsubaCTF

Thank you for your interest in contributing! This document covers everything you need to get started.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Git Workflow](#git-workflow)
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

### Docker Compose Structure

We use a multi-file Docker Compose setup to manage different environments:

- **`docker-compose.yml`**: Base configuration (Services, Networks, Volumes).
- **`docker-compose.override.yml`**: Development overrides (Ports, debug logging, volume mounts). Loaded automatically by `docker compose up`.
- **`docker-compose.prod.yml`**: Production overrides (Restart policies, security settings).

To run with monitoring enabled in development:
```bash
docker compose --profile monitoring up -d
```

To simulate a production environment:
```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
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

## Git Workflow

This project uses a `dev -> main` workflow.

- `main` is the stable/release branch.
- `dev` is the active integration branch.
- Feature, fix, docs, chore, and security branches should be created from `dev`.

Before starting new work:

```bash
git checkout dev
git pull --ff-only origin dev
git checkout -b fix/example-change
```
Most pull requests should target:
```
your-branch -> dev
```
Only release pull requests should target:
```
dev -> main
```

Before opening a PR, double-check the base branch on GitHub. If a PR accidentally targets main, change the base branch to dev before merging.

After a PR is squash-merged, GitHub creates a new commit on the target branch. The original branch commits may still appear in Git graph tools until the branch is deleted.

After the PR is merged:
```bash
git checkout dev
git pull --ff-only origin dev
git branch -D your-branch
git fetch --prune
```
If the remote branch was not deleted automatically:
```bash
git push origin --delete your-branch
git fetch --prune
```

If a PR is accidentally merged into main but the change should be kept, merge main back into dev:
```bash
git checkout dev
git pull --ff-only origin dev
git fetch origin
git merge origin/main
git push origin dev
```
If the change should not be kept on main, revert it on main instead.

For a normal commit or squash merge:

```bash
git checkout main
git pull --ff-only origin main
git revert <commit_sha>
git push origin main
```

For a merge commit:

```bash
git checkout main
git pull --ff-only origin main
git revert -m 1 <merge_commit_sha>
git push origin main
```

Useful commands:
```bash 
git branch -a
git log --oneline --graph --decorate --all
git fetch --prune
git status
```

## Commit Convention

This project enforces [Conventional Commits](https://www.conventionalcommits.org/).
The `commit-msg` hook will reject non-conforming messages.

```
<type>(<scope>): <description>

Types: feat, fix, perf, refactor, docs, ci, chore, style, test, infra, security
Scope: gateway, scheduler, db, shared, migrations, monitoring (optional)

Examples:
  feat(gateway): add /leaderboard pagination
  fix(reminder): handle timezone edge case in set_timer
  infra(monitoring): add node-exporter to prod compose
  ci: skip workflow on doc-only changes
  security(scheduler): harden external HTML fetching
```

## Pull Request Process

1. Branch off `dev`, not `main`.
2. Keep PRs focused — one feature or fix per PR.
3. Make sure CI passes before requesting review.
4. Fill in the PR template checklist, especially the `make prepare` item if you touched SQL.
5. PRs are merged into `dev` first, then `dev` → `main` for releases.
6. After a squash merge, delete the working branch locally and remotely if it is no longer needed.