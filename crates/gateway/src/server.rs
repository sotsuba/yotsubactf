use crate::state::AppState;
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

pub async fn run_server(state: Arc<AppState>, port: u16) {
    let recorder_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("gateway_command_latency_seconds".to_string()),
            // Optimized for Discord bot: most commands take 100ms-3s
            &[0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0],
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
    info!("Health/Metrics server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind health port");
    axum::serve(listener, app)
        .await
        .expect("failed to start axum server");
}

async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db_ok = state.guilds.check_health().await;

    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let shard_info = if state.shard_range.0 == state.shard_range.1 {
        format!("{}/{}", state.shard_range.0, state.shard_total)
    } else {
        format!(
            "{}-{}/{}",
            state.shard_range.0, state.shard_range.1, state.shard_total
        )
    };

    (
        status,
        Json(json!({
            "status": if db_ok { "ok" } else { "degraded" },
            "db": db_ok,
            "shard": shard_info
        })),
    )
}
