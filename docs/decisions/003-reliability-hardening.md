# ADR-003: Reliability Hardening (Scheduler + Gateway)

## Status
Implemented

## Context
A thorough audit of the `yotsubactf` codebase identified several reliability gaps that could lead to cascading failures, data loss (missed notifications), and poor observability during incidents. Key findings include:
- Lack of enforced timeouts on some HTTP clients.
- Absence of structured error classification (Transient vs Permanent).
- No Dead-Letter Queue (DLQ) for failed async tasks (enrichment/notifications).
- Limited health check depth (missing Redis and external probe status).
- Hardcoded retry/backoff parameters in several locations.

## Decision
Increase system resilience by implementing a comprehensive reliability hardening layer across the Scheduler and Gateway services.

### Proposed Changes:

1.  **Enforced Timeouts**: Audit and enforce strict timeouts on all HTTP clients (shared helper or per-client configuration) with specific metrics for timeout failures.
2.  **Error Classification**: Implement an error taxonomy to distinguish between **Transient** errors (e.g., 503, 429) and **Permanent** errors (e.g., 400, 404). Update retry logic to skip retries on permanent failures.
3.  **Dead-Letter Queue (DLQ)**: Create a `dead_letter_queue` table to store failed enrichment and notification tasks. 
    - Record payload summaries, retry counts, and the last error message.
    - Provide repository methods for inspection and manual recovery.
4.  **Deep Health Checks**: Extend the `/health` endpoint and Prometheus metrics to include:
    - Redis connectivity (PING).
    - Lightweight external HTTP probes (e.g., CTFTime connectivity).
5.  **Configurability**: Move task concurrency, retry limits, and backoff parameters into environment variables with sensible defaults.
6.  **Observability**: Introduce correlation IDs for pipeline stages to trace an event from ingestion through enrichment to notification in logs and metrics.

## Consequences

**Benefits:**
- **Cascasding Failure Mitigation**: Timeouts and error classification prevent a single failing dependency from exhausting system resources.
- **Recoverability**: The DLQ ensures that failed notifications or enrichments are not silently dropped and can be recovered.
- **Improved MTTR**: Better health checks and correlation IDs significantly reduce the time needed to diagnose and fix production issues.
- **Operational Flexibility**: Configurable backoff and concurrency allow tuning the system without code changes.

**Trade-offs:**
- **Increased Complexity**: Error classification and DLQ logic add more moving parts to the repository and task layers.
- **Maintenance Overhead**: The DLQ requires monitoring and periodic cleanup/re-processing.
- **Slight Latency**: Deeper health checks add a small amount of overhead to health monitoring.

## Alternatives Considered

**Automatic Retries for DLQ**
Automating DLQ retries was considered but deferred. Manual-first recovery with strong metrics/alerts allows for better understanding of failure patterns before introducing automated retry logic that could potentially cause "retry storms."

**Full Circuit Breaker Implementation**
While a full circuit breaker pattern (e.g., using a library like `resilience4j` equivalent in Rust) was considered, it was deemed too complex for the current scale. Structured backoff and error classification provide 80% of the benefit with significantly lower complexity.
