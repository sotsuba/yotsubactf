use crate::ctftime::models::RawCtftimeEvent;
use reqwest_middleware::ClientWithMiddleware as Client;
use shared::{CtfError, CtfResult};

pub async fn fetch_upcoming(client: &Client) -> CtfResult<Vec<RawCtftimeEvent>> {
    let url = format!("{}/events/?limit=50", shared::CTFTIME_API_BASE);
    let start = std::time::Instant::now();
    let endpoint = "upcoming";

    let resp = client.get(&url).send().await.map_err(|err| {
        let err_msg = format!("{}", err);
        let status_label = if err_msg.contains("timeout") {
            "timeout"
        } else {
            "error"
        };
        metrics::counter!(
            shared::metrics::CTFTIME_API_REQUESTS_TOTAL,
            "endpoint" => endpoint,
            "status"   => status_label
        )
        .increment(1);

        if err_msg.contains("timeout") {
            CtfError::Timeout
        } else {
            CtfError::ExternalApi {
                status: 0,
                message: format!("CTFTime API request failed: {err}"),
            }
        }
    })?;

    let duration = start.elapsed();
    metrics::histogram!(
        shared::metrics::CTFTIME_API_LATENCY,
        "endpoint" => endpoint
    )
    .record(duration.as_secs_f64());

    let status = resp.status();
    metrics::counter!(
        shared::metrics::CTFTIME_API_REQUESTS_TOTAL,
        "endpoint" => endpoint,
        "status"   => status.as_u16().to_string()
    )
    .increment(1);

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(CtfError::RateLimit { retry_after: None });
    }

    if !status.is_success() {
        return Err(CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("CTFTime API returned error: {}", status),
        });
    }

    resp.json::<Vec<RawCtftimeEvent>>()
        .await
        .map_err(|e| CtfError::ExternalApi {
            status: status.as_u16(),
            message: format!("Failed to deserialize CTFTime JSON response: {e}"),
        })
}
