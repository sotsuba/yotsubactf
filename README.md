# YotsubaCTF Discord Bot

A robust, observable Discord bot for CTF teams, built with Rust.

[![CI](https://github.com/sotsuba/yotsubactf/actions/workflows/ci.yml/badge.svg)](https://github.com/sotsuba/yotsubactf/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.95-orange.svg)](https://www.rust-lang.org)

## Features

- **Event Tracking**: Automatically scrapes upcoming CTFs and notifies channels.
- **Team Tracking**: Follow your team's performance and get notified of new results.
- **Reminders**: Configurable reminders for upcoming events (DM and Channel).
- **Writeup Search**: Find writeups for past CTFs directly from Discord.
- **Observability**: Built-in Prometheus metrics and Grafana dashboards.
- **Resilience**: Retries with exponential backoff for CTFtime API calls.

## Screenshots

### Event Commands
![Upcoming events command](docs/assets/event_upcoming_preview.png)

### Grafana Dashboard
![Grafana monitoring](docs/assets/grafana_preview.png)

## Tech Stack

- **Language**: Rust (2024 edition)
- **Database**: PostgreSQL (SQLx)
- **Caching**: Redis & Moka (In-memory)
- **Discord API**: Twilight
- **Monitoring**: Prometheus & Grafana
- **Deployment**: Docker Compose

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Docker](https://docs.docker.com/get-docker/) & [Docker Compose](https://docs.docker.com/compose/install/)
- A Discord Bot Token (from [Discord Developer Portal](https://discord.com/developers/applications))

### Installation

1.  **Clone the repository**:
    ```bash
    git clone https://github.com/sotsuba/ctftime-discord-bot.git
    cd ctftime-discord-bot
    ```

2.  **Configure environment**:
    ```bash
    cp .env.example .env
    # Edit .env and fill in DISCORD_TOKEN and DISCORD_APPLICATION_ID
    ```

3.  **Start the services**:
    ```bash
    docker compose up -d
    ```

### Development

- **Run migrations**: `sqlx migrate run`
- **Run the gateway**: `cargo run -p gateway`
- **Run the scheduler**: `cargo run -p scheduler`
- **Run tests**: `cargo test`

## CI/CD & Git Hooks

This project uses Git hooks to ensure code quality and conventional commits.

### Setup Git Hooks

Run the following command once after cloning the repository:

```bash
make hooks
```

This will enable:
- `pre-commit`: Runs `fmt`, `clippy`, and unit tests.
- `commit-msg`: Enforces [Conventional Commits](https://www.conventionalcommits.org/).
- `pre-push`: Runs full workspace checks and SQLx offline data validation.

### SQLx Offline Data

To build the Docker image without a live database, we use SQLx offline mode. If you modify any SQL queries, you must regenerate the offline data:

```bash
make prepare
```

Then commit the changes in the `.sqlx/` directory.

## Deployment

For local development and private hosting, use Docker Compose. You can run isolated Staging and Production environments on your local machine using separate compose files.

### 1. Standard Local Dev
```bash
docker compose up -d
```

### 2. Multi-Environment (Local)

This allows you to test changes on a development bot before applying them to your primary bot.

- **Staging**: `docker compose -f docker-compose.staging.yml --env-file .env.staging up -d`
    - Ports: Gateway (8185), Scheduler (8186)
- **Production**: `docker compose -f docker-compose.prod.yml --env-file .env.prod up -d`
    - Ports: Gateway (8085), Scheduler (8086)

### Setup Instructions
1.  **Configure environment files**:
    Create `.env.staging` and `.env.prod` by copying `.env.example` and filling in the respective bot tokens and application IDs.
2.  **Run migrations**:
    The bots automatically run migrations on startup within their isolated databases.

## Monitoring

By default, the monitoring stack is **disabled** to keep the footprint minimal. To opt-in and start the observability stack (Prometheus, Grafana, Alertmanager, etc.):

```bash
docker compose --profile monitoring up -d
```

Once running, you can access the dashboard:
- **Grafana**: [http://localhost:3030](http://localhost:3030) (Default login: `admin` / `admin`)
- **Prometheus**: [http://localhost:9090](http://localhost:9090)

Dashboards for the Gateway and Scheduler are pre-provisioned in Grafana.

## Project Structure

- `crates/gateway`: The Discord bot process (slash commands, events).
- `crates/scheduler`: Background tasks (scraping, results, reminders).
- `crates/db`: Shared PostgreSQL repository implementations.
- `crates/shared`: Common models, traits, and utilities.
- `monitoring/`: Configuration for Prometheus and Grafana.

## License

MIT
