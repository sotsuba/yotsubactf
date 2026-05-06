FROM alpine:3.20

# Install dependencies
RUN apk add --no-cache \
    postgresql16-client \
    aws-cli \
    curl

# Create backup script directory
RUN mkdir -p /scripts

# Set up cron
# Run at 02:00 every day
RUN echo "0 2 * * * /scripts/pg_backup.sh >> /var/log/backup.log 2>&1" > /var/spool/cron/crontabs/root

# Start crond in foreground
CMD ["crond", "-f", "-L", "/dev/stdout"]
