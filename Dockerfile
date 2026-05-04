# syntax=docker/dockerfile:1.7

# ------------------------------------------------------------------------------
# Global Build Arguments
# ------------------------------------------------------------------------------
ARG RUST_VERSION=1.88
ARG DISTROLESS_IMAGE=gcr.io/distroless/cc-debian12

# ------------------------------------------------------------------------------
# Stage 0: Toolchain Preparation (chef)
# ------------------------------------------------------------------------------
FROM rust:${RUST_VERSION}-slim AS chef

# Leverage BuildKit cache mounts to prevent redundant downloads across builds.
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*
# Note: libssl-dev is omitted assuming reqwest utilizes rustls-tls (pure-Rust). 
# Reintroduce if OpenSSL dynamic linking is explicitly required by a crate.

# Cache Cargo registry and git index for faster cargo-chef installation.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install cargo-chef --locked

WORKDIR /app

# ------------------------------------------------------------------------------
# Stage 1: Dependency Analysis (planner)
# ------------------------------------------------------------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ------------------------------------------------------------------------------
# Stage 2: Dependency & Binary Compilation (builder)
# ------------------------------------------------------------------------------
FROM chef AS builder

# Disable incremental compilation (unnecessary in CI/Docker, inflates target dir).
ENV CARGO_INCREMENTAL=0
# Enable SQLx offline mode to bypass database connections during compilation.
ENV SQLX_OFFLINE=true

COPY --from=planner /app/recipe.json recipe.json

# Compile dependencies into the cache mount to accelerate subsequent builds.
# This layer remains cached as long as the dependency graph (recipe.json) is unchanged.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release \
        --recipe-path recipe.json \
        -p gateway -p scheduler

COPY . .

# Build workspace binaries. Artifacts are extracted from the cache mount 
# to the persistent layer before stripping to prevent loss upon unmounting.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p gateway -p scheduler \
    && cp target/release/gateway /app/gateway \
    && cp target/release/scheduler /app/scheduler \
    && strip /app/gateway /app/scheduler

# ------------------------------------------------------------------------------
# Stage 3: Runtime Environment (scheduler)
# ------------------------------------------------------------------------------
FROM ${DISTROLESS_IMAGE} AS scheduler

# Ensure TLS certificate roots are available for external HTTPS requests.
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /app/scheduler /usr/local/bin/app

USER nonroot:nonroot
EXPOSE 8085
CMD ["/usr/local/bin/app"]

# ------------------------------------------------------------------------------
# Stage 4: Runtime Environment (gateway)
# ------------------------------------------------------------------------------
FROM ${DISTROLESS_IMAGE} AS gateway

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /app/gateway /usr/local/bin/app

USER nonroot:nonroot
EXPOSE 8085
CMD ["/usr/local/bin/app"]