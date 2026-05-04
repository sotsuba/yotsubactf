# ── Stage 0: Install cargo-chef once ──────────────────────────────────────────
FROM rust:1.88-slim AS chef
RUN cargo install cargo-chef --locked
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# ── Stage 1: Analyze dependency graph ─────────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: Build dependencies (cached) ──────────────────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
ENV SQLX_OFFLINE=true
RUN cargo chef cook --release --recipe-path recipe.json -p scheduler -p gateway

# ── Stage 3: Build actual binaries ────────────────────────────────────────────
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release -p scheduler -p gateway \
    && strip target/release/scheduler target/release/gateway

# ── Runtime: scheduler ────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12 AS scheduler
COPY --from=builder /app/target/release/scheduler /usr/local/bin/app
USER nonroot:nonroot
CMD ["/usr/local/bin/app"]

# ── Runtime: gateway ──────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12 AS gateway
COPY --from=builder /app/target/release/gateway /usr/local/bin/app
USER nonroot:nonroot
CMD ["/usr/local/bin/app"]