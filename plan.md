# PostgreSQL Backup Implementation Plan

## Problem
Currently, the `pgdata_prod` volume has zero backup. If the host dies, all data is gone permanently. 

## Goal
Implement a nightly `pg_dump` cron via a sidecar container in `docker-compose.prod.yml`. The backup must be `gzip`-compressed and uploaded to an object storage provider (S3/R2/Backblaze). Retention policies must enforce 7 daily and 4 weekly backups.

## Checklist

### 1. Environment Configuration
- [ ] Update `.env.example` to include required variables:
  - [ ] `BACKUP_S3_BUCKET`
  - [ ] `BACKUP_S3_REGION`
  - [ ] `BACKUP_S3_ENDPOINT`
  - [ ] `BACKUP_S3_ACCESS_KEY`
  - [ ] `BACKUP_S3_SECRET_KEY`
  - [ ] `POSTGRES_PASSWORD` (if not already present and accessible)

### 2. Backup Script Creation
- [ ] Create `scripts/backup/pg_backup.sh`
- [ ] Add `pg_dump` command piped to `gzip` for compression
- [ ] Add S3 upload logic (e.g., using `aws-cli` or `s3cmd`)
- [ ] Implement retention policy (keep 7 daily, 4 weekly):
  - [ ] Create logic to rotate/prune old backups from S3 directly or handle pruning before upload.
- [ ] Ensure the script surfaces proper exit codes on failure for monitoring.

### 3. Docker Compose Updates
- [ ] Modify `docker-compose.prod.yml` to include a sidecar container (`db-backup`).
- [ ] Use an image that includes `postgresql-client`, `aws-cli` (or equivalent), and `cron` (e.g., a custom `Dockerfile` or a prebuilt alpine-based image).
- [ ] Mount `scripts/backup/pg_backup.sh` into the container.
- [ ] Schedule the backup to run nightly via cron inside the container.

### 4. Verification & Testing
- [ ] Ensure the backup container starts and runs the nightly schedule successfully.
- [ ] Check the destination object storage to confirm uploads succeed.
- [ ] **Restore Drill:** Spin up `docker-compose.staging.yml`, fetch a fresh backup from S3, run `pg_restore`/`psql`, and confirm data integrity.
