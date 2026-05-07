use dashmap::DashMap;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed,
    Open(Instant),
    HalfOpen,
}

struct CircuitState {
    state: State,
    failure_count: u32,
}

pub struct CircuitBreaker {
    states: DashMap<String, CircuitState>,
    failure_threshold: u32,
    cooldown: Duration,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            states: DashMap::new(),
            failure_threshold,
            cooldown,
        }
    }

    fn get_host(req: &Request) -> String {
        req.url().host_str().unwrap_or("unknown").to_string()
    }

    fn record_success(&self, host: &str) {
        if let Some(mut state) = self.states.get_mut(host) {
            state.state = State::Closed;
            state.failure_count = 0;
        }
    }

    fn record_failure(&self, host: &str) {
        let mut state = self.states.entry(host.to_string()).or_insert(CircuitState {
            state: State::Closed,
            failure_count: 0,
        });

        state.failure_count += 1;
        if state.failure_count >= self.failure_threshold {
            if !matches!(state.state, State::Open(_)) {
                error!(host, count = state.failure_count, "Circuit Breaker OPENED");
            }
            state.state = State::Open(Instant::now());
        }
    }

    fn check(&self, host: &str) -> Result<(), String> {
        if let Some(mut state) = self.states.get_mut(host) {
            match state.state {
                State::Open(opened_at) => {
                    if opened_at.elapsed() >= self.cooldown {
                        warn!(host, "Circuit Breaker HALF-OPEN (testing recovery)");
                        state.state = State::HalfOpen;
                        Ok(())
                    } else {
                        Err(format!("Circuit Breaker is OPEN for {host}"))
                    }
                }
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }
}

pub struct CircuitBreakerMiddleware {
    breaker: Arc<CircuitBreaker>,
}

impl CircuitBreakerMiddleware {
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            breaker: Arc::new(CircuitBreaker::new(failure_threshold, cooldown)),
        }
    }
}

#[async_trait::async_trait]
impl Middleware for CircuitBreakerMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        let host = CircuitBreaker::get_host(&req);

        // 1. Check if circuit is open
        if let Err(msg) = self.breaker.check(&host) {
            return Err(reqwest_middleware::Error::Middleware(anyhow::anyhow!(msg)));
        }

        // 2. Perform request
        let res = next.run(req, extensions).await;

        // 3. Update state based on result
        match &res {
            Ok(resp) => {
                if resp.status().is_server_error() {
                    self.breaker.record_failure(&host);
                } else {
                    self.breaker.record_success(&host);
                }
            }
            Err(_) => {
                self.breaker.record_failure(&host);
            }
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_logic() {
        let breaker = CircuitBreaker::new(2, Duration::from_millis(100));
        let host = "example.com";

        // Initial state
        assert!(breaker.check(host).is_ok());

        // First failure
        breaker.record_failure(host);
        assert!(breaker.check(host).is_ok());

        // Second failure -> OPEN
        breaker.record_failure(host);
        assert!(breaker.check(host).is_err());

        // Wait for cooldown
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be Half-Open (check returns Ok but changes state)
        assert!(breaker.check(host).is_ok());
        assert_eq!(breaker.states.get(host).unwrap().state, State::HalfOpen);

        // Success should CLOSE
        breaker.record_success(host);
        assert_eq!(breaker.states.get(host).unwrap().state, State::Closed);
    }
}
