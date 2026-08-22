# NStream runtime guide

NStream is a Rust control plane and worker system for multilingual video
dubbing. Jobs are stored in PostgreSQL, claimed atomically, heartbeated during
work, and rendered into tenant-scoped MP4 artifacts.

## Processing path

1. A tenant creates a signed S3 upload and submits the resulting bucket/key.
2. A worker claims the durable job, stages the object, extracts bounded audio,
   and transcribes it with Whisper GGML.
3. It translates segments, synthesizes speech with Piper, mixes timestamped
   audio, optionally invokes a configured lip-sync GPU service, then validates
   the MP4 with FFprobe before storing it.
4. Stage state, events, outputs, retries, heartbeats, and cancellation are all
   recorded in PostgreSQL.

`creator_selective` jobs may supply `scene_ranges`; only fully contained
segments are dubbed, while original audio remains outside the selected ranges.

## Required production configuration

- `DATABASE_URL`
- `NSTREAM_AUTH_REQUIRED=true`
- `NSTREAM_BOOTSTRAP_API_KEY` (bootstrap only; issue tenant keys afterward)
- `WEBHOOK_ENCRYPTION_KEY`
- `NSTREAM_STORAGE_BACKEND=s3`, `S3_BUCKET`, `AWS_REGION`
- `WHISPER_MODEL_PATH` and a Piper binary/model mapping
- `TRANSLATION_API_URL`
- `LIPSYNC_API_URL` and `LIPSYNC_REQUIRED=true` when lip-sync is mandatory

## Local S3-compatible storage

Docker Compose starts MinIO at `http://localhost:9000` with its console at
`http://localhost:9001`. Set `MINIO_ROOT_USER` and `MINIO_ROOT_PASSWORD` in
your `.env`, start the stack, sign into the console, and create the bucket named
by `S3_BUCKET` (normally `nstream-media`). The API and worker use the same
credentials through the standard AWS environment variables.

Run API and media processing as separate processes: `nstream-api` and
`nstream-worker`. The Kubernetes manifests include non-root execution,
temporary writable volumes, worker GPU limits, probes, HPA/PDB, backup CronJob,
and network policy.

## Important endpoints

- `POST /uploads` creates a tenant-bound S3 upload URL.
- `POST /jobs`, `GET /jobs`, `GET /jobs/:id`, `DELETE /jobs/:id`
- `GET /jobs/:id/stages`, `GET /jobs/:id/events`, `GET /jobs/:id/transcript`
- `GET /jobs/:id/outputs/:language/url`
- `POST /live`, `GET /live/:id`, `DELETE /live/:id`
- `POST /webhooks/register`, `GET /metrics`, `GET /health`, `GET /readyz`

Use `load/k6-api.js` only against an approved non-production endpoint first.
See `docs/OPERATIONS.md` for load and backup guidance.
