.PHONY: build check run-gateway run-scheduler run-scrape run-results run-writeups run-digest run-remind db-up db-down db-migrate up down dev hooks prepare

# Build all workspace crates
build:
	cargo build

# Run cargo check to verify code compiles
check:
	cargo check --workspace

# Run the Discord gateway bot
run-gateway:
	cargo run -p gateway

# Run the unified scheduler (all tasks in parallel)
run-scheduler:
	cargo run -p scheduler

# One-shot tasks (synchronized with --task flag)
run-scrape:
	cargo run -p scheduler -- --task scrape

run-results:
	cargo run -p scheduler -- --task results

run-writeups:
	cargo run -p scheduler -- --task writeups

run-digest:
	cargo run -p scheduler -- --task digest

run-remind:
	cargo run -p scheduler -- --task remind

# Start infrastructure (Postgres & Redis)
db-up:
	docker-compose up -d postgres redis

# Stop infrastructure
db-down:
	docker-compose stop postgres redis

# Run sqlx migrations
db-migrate:
	sqlx migrate run --source migrations

# Run everything (infra + scheduler) via Docker Compose
up:
	docker-compose up -d

# Stop everything
down:
	docker-compose down

# Development mode: run both gateway and scheduler in parallel
dev:
	make -j2 run-gateway run-scheduler

# Setup git hooks
hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/*

