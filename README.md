# ctftime-discort-bot

Rust bot that tracks CTFtime events and posts updates to Discord.

## Setup

1. Copy `.env.example` to `.env` and fill in values.
2. Start dependencies with Docker Compose.
3. Run the worker or the gateway service (added in later steps).

## Environment Variables

### Discord

- `DISCORD_TOKEN`: bot token.
- `DISCORD_APPLICATION_ID`: application ID for registering slash commands.
- `DISCORD_CHANNEL_ID`: channel ID for event notifications.
- `DISCORD_GUILD_ID`: staging guild for guild-scoped command registration.

### Sharding

- `DISCORD_SHARD_TOTAL`: total shards to use. If empty, use Discord recommended count.
- `DISCORD_SHARD_START`: first shard (0-based, inclusive) for this process.
- `DISCORD_SHARD_END`: last shard (0-based, inclusive) for this process.

### Database and Cache

- `DATABASE_URL`: Postgres connection string.
- `REDIS_URL`: Redis connection string.

### Other

- `SCRAPER_DELAY_MS`: delay between HTML scrape requests.
- `RUST_LOG`: logging filter.
