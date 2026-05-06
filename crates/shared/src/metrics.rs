//! Central metric names and labels to ensure consistency between binaries.

pub const GATEWAY_COMMANDS_TOTAL: &str = "gateway_commands_total";
pub const GATEWAY_COMMAND_LATENCY: &str = "gateway_command_latency_seconds";
pub const GATEWAY_RATE_LIMIT_TOTAL: &str = "gateway_rate_limit_total";

pub const DB_REDIS_HITS_TOTAL: &str = "db_redis_hits_total";
pub const DB_REDIS_MISSES_TOTAL: &str = "db_redis_misses_total";

pub const SCHEDULER_EVENTS_SCRAPED: &str = "scheduler_events_scraped_total";
pub const SCHEDULER_REMINDERS_PENDING: &str = "scheduler_reminders_pending_count";
pub const SCHEDULER_REMINDERS_SKIPPED: &str = "scheduler_reminders_skipped_total";
pub const SCHEDULER_REMINDERS_FIRED: &str = "scheduler_reminders_fired_total";

pub const SCHEDULER_TASK_DURATION: &str = "scheduler_task_duration_seconds";
pub const SCHEDULER_TASKS_TOTAL: &str = "scheduler_tasks_total";
pub const SCHEDULER_ENRICH_FAIL_TOTAL: &str = "scheduler_enrich_fail_total";
pub const SCHEDULER_LLM_REQUESTS_TOTAL: &str = "scheduler_llm_requests_total";
pub const SCHEDULER_LLM_FAILURE_TOTAL: &str = "scheduler_llm_failure_total";
pub const SCHEDULER_LLM_LATENCY: &str = "scheduler_llm_latency_seconds";

pub const CTFTIME_API_REQUESTS_TOTAL: &str = "ctftime_api_requests_total";
pub const CTFTIME_API_LATENCY: &str = "ctftime_api_latency_seconds";

pub const DISCORD_DELIVERY_TOTAL: &str = "discord_delivery_total";
pub const DISCORD_DELIVERY_LATENCY: &str = "discord_delivery_latency_seconds";
