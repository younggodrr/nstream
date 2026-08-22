# NStream

NStream is an AI-powered media platform built entirely in Rust. It provides a scalable pipeline for automated dubbing, translation, upscaling, and visual effects processing of video content — supporting studio batch workflows, live streaming, and creator-directed selective enhancement.

**Current scope:** The API and standalone worker use PostgreSQL atomic claims,
heartbeats, retries, tenant isolation, S3-compatible artifacts, Whisper ASR,
translation/TTS/lip-sync provider integrations, FFmpeg rendering and QC.
External model/service credentials must be provisioned before running a full
production pipeline.

---

## Architecture

NStream is a Cargo workspace composed of eight focused crates:

| Crate | Role |
|---|---|
| `nstream-core` | Shared types, error enum, `Node` trait, job metadata |
| `nstream-dag` | DAG pipeline executor using `petgraph` |
| `nstream-ingest` | RTMP/S3 ingest and multi-camera aggregation |
| `nstream-buffer` | 5-minute sliding ring buffer + disk overflow |
| `nstream-inference` | Whisper GGML ASR worker primitives |
| `nstream-media` | FFmpeg decoder/encoder/muxer traits and impls |
| `nstream-api` | Axum REST API — job submission, polling, webhooks |
| `nstream-observe` | Structured JSON tracing, Prometheus metrics, cost tracking |

---

## Prerequisites

- Rust 1.78+ with Cargo
- Docker and Docker Compose (for full-stack local dev)
- FFmpeg and FFprobe

---

## Running locally

### 1. Copy environment config

```bash
cp .env.example .env
```

Edit `.env` as needed (see [Environment Variables](#environment-variables) below).

### 2. Start backing services

```bash
docker-compose up postgres redis -d
```

### 3. Build the workspace

```bash
cargo build --workspace
```

### 4. Run the API server

```bash
cargo run -p nstream-api
```

The API listens on `http://0.0.0.0:8080` by default.

### 5. Full stack via Docker Compose

```bash
docker-compose up --build
```

This starts Postgres, Redis, and the NStream API container together.

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `postgres://nstream:nstream@localhost:5432/nstream` | PostgreSQL connection string |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection string |
| `AWS_REGION` | `us-east-1` | AWS region for S3 access |
| `S3_BUCKET` | `nstream-media` | Default S3 bucket for media ingestion |
| `GPU_WORKER_COUNT` | `4` | Number of inference worker goroutines to spawn |
| `API_PORT` | `8080` | Port for the HTTP API server |
| `NSTREAM_LOCAL_MEDIA_DIR` | `./media-input` | Development input root; sources live at `<root>/<bucket>/<key>` |
| `NSTREAM_OUTPUT_DIR` | `./media-output` | Development delivery root |
| `JOB_WORKER_COUNT` | `2` | Concurrent background delivery workers |
| `JOB_QUEUE_CAPACITY` | `128` | Maximum queued jobs before back-pressure applies |
| `RUST_LOG` | `info` | Tracing filter (e.g. `debug`, `nstream_api=trace`) |

---

## API Endpoints

### Health

```
GET /health
→ 200 {"status":"ok"}
```

### Jobs

```
POST /jobs
Body: {
  "source_lang": "en",
  "target_langs": ["es", "fr"],
  "pipeline_mode": "studio_batch",   // "studio_batch" | "live_stream" | "creator_selective"
  "s3_bucket": "my-bucket",
  "s3_key": "media/film.mov"
}
→ 201 { job_id, status, ..., output_files: [] }
```

```
GET /jobs/:id
→ 200 { job_id, status, ... }
→ 404 { error: "not_found", message: "..." }
```

```
GET /jobs
→ 200 [ { job_id, status, ... }, ... ]
```

```
GET /jobs/:id/outputs/:language
→ 200 application/octet-stream
```

`s3_bucket` and `s3_key` are currently required. In local development, put the source file at `NSTREAM_LOCAL_MEDIA_DIR/<bucket>/<key>`. Each requested target language receives a downloadable delivery artifact. This is a safe delivery adapter, not a claim that AI dubbing has occurred.

### Webhooks

```
POST /webhooks/register
Body: {
  "url": "https://your-server.com/hooks/nstream",
  "events": ["completed", "failed"]   // optional, defaults to both
}
→ 201 { id, url, registered_at }
```

---

## Pipeline Modes

- **`studio_batch`** — Maximum quality, GPU-intensive batch processing. Uses disk-backed overflow buffer.
- **`live_stream`** — Low-latency real-time processing. Uses in-memory 5-minute sliding ring buffer (9000 frames at 30 fps).
- **`creator_selective`** — Creator-directed selective enhancement. Uses disk-backed buffer.

---

## Observability

- **Structured logging**: JSON format via `tracing` + `tracing-subscriber`. Control verbosity with `RUST_LOG`.
- **Prometheus metrics**: Exported via `nstream-observe`. Counters for `jobs_submitted`, `jobs_completed`, `jobs_failed`; histogram for `job_duration_seconds`; gauge for `active_gpu_workers`.
- **Cost tracking**: `CostTracker` records GPU seconds per job and computes estimated USD cost given a rate per second.
- **Metrics endpoint**: Prometheus text is available at `GET /metrics`.

---

## Crate-level build

Each crate can be compiled independently:

```bash
cargo build -p nstream-core
cargo build -p nstream-dag
cargo build -p nstream-api
# etc.
```

---

## Notes

See [docs/MVP.md](docs/MVP.md) for the current runtime configuration and API
surface, and [docs/OPERATIONS.md](docs/OPERATIONS.md) for load/backup guidance.
