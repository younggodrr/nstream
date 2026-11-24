#!/usr/bin/env bash
# Starts an ephemeral Cloudflare Quick Tunnel to the authenticated local worker.
# The bearer token still protects the API; do not expose a worker without it.
set -euo pipefail
DEPLOY_DIR="${NSTREAM_GPU_DEPLOY_DIR:-/content/nstream-gpu-deployment}"
BIN="${DEPLOY_DIR}/bin/cloudflared"
mkdir -p "$(dirname "${BIN}")"
if [[ ! -x "${BIN}" ]]; then
  curl --fail --location --retry 3 \
    --output "${BIN}" \
    https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64
  chmod 0755 "${BIN}"
fi
"${BIN}" tunnel --no-autoupdate --url http://127.0.0.1:8000 2>&1 | tee "${DEPLOY_DIR}/tunnel.log"
