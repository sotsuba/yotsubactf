use crate::SharedState;
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

pub async fn run_server(state: Arc<SharedState>, port: u16) {
    let recorder_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("scheduler_task_duration_seconds".to_string()),
            // Scheduler tasks run longer (scraping, digest) — wider buckets
            &[0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0],
        )
        .unwrap()
        .set_buckets_for_metric(
            Matcher::Full("ctftime_api_latency_seconds".to_string()),
            // External HTTP calls
            &[0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0],
        )
        .unwrap()
        .install_recorder()
        .expect("failed to install prometheus recorder");

    let app = Router::new()
        .route("/health", get(health_handler))
        .route(
            "/metrics",
            get(move || async move { recorder_handle.render() }),
        )
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Scheduler Health/Metrics server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind health port");
    axum::serve(listener, app)
        .await
        .expect("failed to start axum server");
}

async fn health_handler(State(state): State<Arc<SharedState>>) -> impl IntoResponse {
    let db_ok = state.guild_repo.check_health().await;

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(json!({
            "status": if db_ok { "ok" } else { "degraded" },
            "db": db_ok,
            "service": "scheduler"
        })),
    )
}
