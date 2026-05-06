# Resilient HTTP Client with Circuit Breaker and Backoff

## Status
Implemented

## Context
The bot relies heavily on external APIs (CTFTime) and third-party CTF websites for data enrichment. Currently:
- Transient network errors or 5xx responses cause immediate task failure in some areas (HTML scraping).
- Other areas (`api.rs`, `discord.rs`) have duplicated custom retry logic that is hard to maintain.
- A slow or failing external host can block worker threads until timeout.
- Repeatedly hitting a failing host increases load on the target and wastes local resources.
- There is no **Circuit Breaker** to protect the bot or the external services during prolonged outages.

## Decision
Implement a unified, resilient HTTP client stack using `reqwest-middleware` to replace custom retry loops and add circuit breaking capabilities.

### 1. Unified Client Architecture
The `SharedState` will now expose a `reqwest_middleware::ClientWithMiddleware` instead of a raw `reqwest::Client`. This ensures every request made by the bot automatically inherits resilience policies.

### 2. Exponential Backoff (Retry)
- **Crate:** `reqwest-retry`
- **Strategy:** Exponential backoff with jitter.
- **Max Retries:** 3.
- **Criteria:** Retry on 5xx, 429 (Rate Limit), and connection timeouts.

### 3. Circuit Breaker
- **Crate:** `reqwest-circuit-breaker`
- **Threshold:** 5 consecutive failures opens the circuit for a specific domain.
- **Recovery:** 30 seconds in "Open" state before attempting a "Half-Open" probe.
- **Scope:** Per-domain (to avoid blocking CTFTime just because an individual CTF site is down).

### 4. Observability
- Add Prometheus counters:
    - `http_retries_total`: Total number of retry attempts.
    - `http_circuit_breaker_state`: Gauge (0=Closed, 1=Open, 2=HalfOpen) per domain.

## Consequences
- **Positive:** Increased reliability; the bot handles minor outages silently across all tasks (Scraping, Notifying, LLM).
- **Positive:** Cleaner code; removed duplicated `request_with_retry` loops from business logic.
- **Positive:** Protection against cascading failures via fail-fast circuit breaking.
- **Negative:** Slightly increased complexity in the initial client setup.
- **Negative:** We need to update all call sites from `Client` to `ClientWithMiddleware`.