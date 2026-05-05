#!/bin/sh
set -e

# Configuration
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
BACKUP_NAME="pg_backup_${TIMESTAMP}.sql.gz"
BACKUP_DIR="/tmp/backups"
mkdir -p "${BACKUP_DIR}"

# Environment variables check
if [ -z "$POSTGRES_DB" ] || [ -z "$POSTGRES_USER" ] || [ -z "$POSTGRES_PASSWORD" ] || [ -z "$POSTGRES_HOST" ]; then
    echo "Error: Postgres environment variables (DB, USER, PASSWORD, HOST) are not set."
    exit 1
fi

if [ -z "$BACKUP_S3_BUCKET" ] || [ -z "$BACKUP_S3_ENDPOINT" ]; then
    echo "Error: S3 environment variables (BUCKET, ENDPOINT) are not set."
    exit 1
fi

echo "Starting backup of ${POSTGRES_DB} at ${TIMESTAMP}..."

# 1. Run pg_dump and compress
PGPASSWORD="${POSTGRES_PASSWORD}" pg_dump -h "${POSTGRES_HOST}" -U "${POSTGRES_USER}" "${POSTGRES_DB}" | gzip > "${BACKUP_DIR}/${BACKUP_NAME}"

echo "Backup created: ${BACKUP_DIR}/${BACKUP_NAME}"

# 2. Upload to S3
echo "Uploading to S3..."
aws s3 cp "${BACKUP_DIR}/${BACKUP_NAME}" "s3://${BACKUP_S3_BUCKET}/${BACKUP_NAME}" --endpoint-url "${BACKUP_S3_ENDPOINT}"

echo "Upload complete."

# 3. Retention Policy: Keep 7 daily and 4 weekly backups.
# Implementation: 
# - Keep all backups from the last 7 days.
# - Keep Sunday backups from the last 4 weeks.
# - Delete everything else.

echo "Pruning old backups from S3..."

# List backups on S3
BACKUPS=$(aws s3 ls "s3://${BACKUP_S3_BUCKET}/" --endpoint-url "${BACKUP_S3_ENDPOINT}" | grep "pg_backup_" | awk '{print $4}')

CURRENT_DATE=$(date +%s)
ONE_DAY=$((24 * 60 * 60))
SEVEN_DAYS=$((7 * ONE_DAY))
THIRTY_DAYS=$((30 * ONE_DAY))

for BACKUP in $BACKUPS; do
    # Extract date from filename: pg_backup_YYYYMMDD_HHMMSS.sql.gz
    # Format: YYYYMMDD
    B_DATE_STR=$(echo "$BACKUP" | cut -d'_' -f3)
    # Check if we can parse it
    if ! echo "$B_DATE_STR" | grep -qE '^[0-9]{8}$'; then
        continue
    fi
    
    # Convert to seconds since epoch (using date command)
    # Busybox date (alpine) is a bit limited, so we use a specific format
    # YYYY-MM-DD
    B_DATE_FMT="${B_DATE_STR:0:4}-${B_DATE_STR:4:2}-${B_DATE_STR:6:2}"
    B_SECONDS=$(date -d "$B_DATE_FMT" +%s)
    
    AGE=$((CURRENT_DATE - B_SECONDS))
    
    KEEP=false
    
    # 1. Keep if less than 7 days old
    if [ "$AGE" -lt "$SEVEN_DAYS" ]; then
        KEEP=true
    fi
    
    # 2. Keep if Sunday and less than 30 days old
    # Day of week (0=Sunday)
    DOW=$(date -d "$B_DATE_FMT" +%u)
    if [ "$DOW" -eq 7 ] && [ "$AGE" -lt "$THIRTY_DAYS" ]; then
        KEEP=true
    fi
    
    if [ "$KEEP" = false ]; then
        echo "Deleting old backup: $BACKUP"
        aws s3 rm "s3://${BACKUP_S3_BUCKET}/${BACKUP}" --endpoint-url "${BACKUP_S3_ENDPOINT}"
    else
        echo "Keeping backup: $BACKUP"
    fi
done

echo "Cleaning up local backup..."
rm "${BACKUP_DIR}/${BACKUP_NAME}"

echo "Backup process completed successfully."
