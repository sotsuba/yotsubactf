# API Endpoints (Docker Compose)

This project does not expose a public REST API. The only HTTP endpoints are for health checks, metrics, and monitoring tools.

## Core Services

### Gateway

| Purpose | Path | Dev URL | Prod URL |
| --- | --- | --- | --- |
| Health check | `/health` | http://localhost:8085/health | http://127.0.0.1:8085/health |
| Prometheus metrics | `/metrics` | http://localhost:8085/metrics | http://127.0.0.1:8085/metrics |

Notes:
- Port comes from `HEALTH_PORT` (default `8085` in compose).

### Scheduler

| Purpose | Path | Dev URL | Prod URL |
| --- | --- | --- | --- |
| Health check | `/health` | http://localhost:8086/health | http://127.0.0.1:8086/health |
| Prometheus metrics | `/metrics` | http://localhost:8086/metrics | http://127.0.0.1:8086/metrics |

Notes:
- Scheduler listens on `HEALTH_PORT=8085` internally, but is mapped to host `8086` in compose.

## Monitoring (profile: `monitoring`)

### Web UIs

| Service | Dev URL | Prod URL | Notes |
| --- | --- | --- | --- |
| Prometheus | http://localhost:9090 | http://127.0.0.1:9090 | Metrics scrape + alert rules |
| Grafana | http://localhost:3030 | http://127.0.0.1:3030 | Default login `admin` / `admin` (dev) |
| Alertmanager | http://localhost:9093 | http://127.0.0.1:9093 | Receives alerts from Prometheus |
| Metabase (prod only) | - | http://127.0.0.1:3001 | SQL dashboards |

### Internal-Only Services

These run without host ports in compose:
- Loki (log store)
- Promtail (log shipper)
- Node exporter
- Redis exporter
- Postgres exporter

## Data Stores (dev only)

These are exposed in `docker-compose.override.yml` for local development:

| Service | URL |
| --- | --- |
| Postgres | postgres://ctfbot:ctfbot@localhost:5432/ctfbot |
| Redis | redis://localhost:6379 |

## Security Notes

- Production compose binds monitoring and core endpoints to `127.0.0.1` only.
- If you publish these ports externally, put them behind auth or a VPN.
