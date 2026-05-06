# ADR-001: Reminder Scheduler — Polling vs Event-Driven

## Status
Proposed

## Context

The current scheduler uses a polling approach — it wakes up every 60 seconds,
queries the database for any due reminders, and fires them. This introduces
several problems:

- Reminders can be delayed by up to 59 seconds relative to the time the user set
- The database is queried on every tick regardless of whether any reminders are due
- Poor scalability as the number of active reminders grows

Additionally, this was identified as a potential CWE-400 (Uncontrolled Resource
Consumption) surface: a user could previously set a reminder arbitrarily far into
the future (e.g. year 2999), causing the scheduler to hold the job indefinitely.
A per-user quota of 50 active reminders has since been enforced, but the
underlying scheduling inefficiency remains.

## Decision

Replace the polling loop with an event-driven scheduler backed by an in-memory
min-heap and a `tokio::sync::mpsc` channel.

**How it works:**

1. On startup, load all pending reminders from the database and insert them into
   a min-heap sorted by `fire_at` timestamp.
2. The scheduler sleeps exactly until the earliest reminder in the heap is due,
   then fires and pops it.
3. When a user sets a new reminder, the gateway pushes it into the channel,
   waking the scheduler immediately to recalculate the next sleep duration.

```
heap: [10:00:01, 10:05:00, 11:00:00]
       ↓ sleep until 10:00:01
       → fire, pop
       ↓ sleep until 10:05:00
       → fire, pop
       ...
```

**On restart**, the heap is rebuilt by querying all pending reminders from the
database at startup.

## Consequences

**Benefits:**
- Reminders fire with second-level precision
- No wasted CPU or database queries during idle periods
- Scheduling logic is centralized and easier to test

**Trade-offs:**
- The heap lives in memory — a crash or restart requires a rebuild from the
  database on next startup
- Slightly more complex than a simple polling loop
- Does not support multiple scheduler instances without additional coordination

## Alternatives Considered

**Redis Sorted Set (`ZADD` / `BZPOPMIN`)**
A Redis ZSET with the reminder's `fire_at` as the score would support multiple
instances and survive restarts without a rebuild step. However, this adds
operational complexity that is not yet warranted. The project already includes
Redis in its stack, making this a straightforward upgrade path if multi-instance
support becomes necessary.

**Keep polling**
Simplest to reason about, but does not address the latency issue or the
unnecessary database load during quiet periods.