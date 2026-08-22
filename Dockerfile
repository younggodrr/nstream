# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.94-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first for layer-cached dependency compilation.
COPY Cargo.toml ./
COPY nstream-core/Cargo.toml        ./nstream-core/
COPY nstream-dag/Cargo.toml         ./nstream-dag/
COPY nstream-ingest/Cargo.toml      ./nstream-ingest/
COPY nstream-buffer/Cargo.toml      ./nstream-buffer/
COPY nstream-inference/Cargo.toml   ./nstream-inference/
COPY nstream-media/Cargo.toml       ./nstream-media/
COPY nstream-api/Cargo.toml         ./nstream-api/
COPY nstream-observe/Cargo.toml     ./nstream-observe/

# Create stub lib.rs files so Cargo can resolve the workspace without full source.
RUN for crate in nstream-core nstream-dag nstream-ingest nstream-buffer \
      nstream-inference nstream-media nstream-api nstream-observe; do \
    mkdir -p $crate/src && echo "// stub" > $crate/src/lib.rs; \
  done
RUN mkdir -p nstream-api/src && echo "fn main() {}" > nstream-api/src/main.rs

# Pre-build dependencies only.
RUN cargo build --release -p nstream-api 2>/dev/null || true

# Now copy the real source and do the full build.
COPY . .

# Touch source files to force recompilation after stub files.
RUN find . -name "*.rs" -exec touch {} +

RUN cargo build --release -p nstream-api --bins

# ── Runtime stage ───────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/nstream-api /usr/local/bin/nstream-api
COPY --from=builder /app/target/release/nstream-worker /usr/local/bin/nstream-worker
COPY --from=builder /app/models /app/models

RUN useradd --system --create-home --uid 10001 nstream
USER nstream

EXPOSE 8080

CMD ["nstream-api"]
