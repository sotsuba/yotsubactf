# Changelog

## Unreleased

### Bug Fixes

- **gateway:** address clippy warnings and dead code to unblock build
- **test:** update in-memory repository to match new creation trait signature
- **migrations:** remove unused vector extension
- **reminder:** add bounds check and checked arithmetic in set_timer
- **db:** make initial migration idempotent, drop unused raw_html column
- **metrics:** switch latency to histogram with explicit buckets
- **alerts:** use correct success label, add scheduler alerts

### CI/CD

- add GitHub Actions CI and security audit
- overhaul GitHub Actions pipeline
- add cache-to in release workflow
- parallelize gateway and scheduler Docker builds
- add cache-to in release workflow

### Features

- **gateway:** scaffold interactions service
- **shared:** add CompletedFilter and list_completed to ReadCtfRepository
- **db:** implement list_completed with paginated SQL query
- **event:** scaffold EventCommand module with stub handlers
- **event:** implement completed subcommand
- **event:** implement info subcommand
- **gateway:** extract shared option builders into event/validation
- **gateway:** set bot presence status on shard startup
- **event:** merge event command group refactor into dev
- **grafana:** rebuild gateway dashboard with meaningful panels
- **metrics:** track subscribed guild count gauge

### Refactoring

- overhaul workspace — replace bot/core/discord/scraper with shared/gateway/scheduler/db
- **event:** migrate upcoming subcommand
- **event:** migrate current subcommand
- **event:** migrate countdown subcommand
- **handler:** update ComponentAction to event:* routing
- **event:** update interaction routing for specific subcommands
- **reminder:** update creation to return structured outcome
- **gateway:** remove legacy top-level event commands

