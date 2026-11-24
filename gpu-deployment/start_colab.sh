#!/usr/bin/env bash
# Starts a local-only authenticated service. Run the tunnel script separately.
set -euo pipefail

# The HTTP worker has no display. Avoid inheriting Colab's inline-notebook
# Matplotlib backend into the MuseTalk inference subprocess.
export MPLBACKEND=Agg
DEPLOY_DIR="${NSTREAM_GPU_DEPLOY_DIR:-/content/nstream-gpu-deployment}"
MAMBA_ROOT_PREFIX="${MAMBA_ROOT_PREFIX:-${DEPLOY_DIR}/mamba}"
MAMBA_BIN="${MAMBA_ROOT_PREFIX}/bin/micromamba"
ENVIRONMENT_NAME="${NSTREAM_GPU_ENV:-nstream-gpu}"
export MUSE_TALK_ROOT="${MUSE_TALK_ROOT:-${DEPLOY_DIR}/MuseTalk}"
export MUSE_TALK_MODEL_DIR="${MUSE_TALK_MODEL_DIR:-${MUSE_TALK_ROOT}/models}"
export NSTREAM_GPU_WORK_DIR="${NSTREAM_GPU_WORK_DIR:-/content/nstream-gpu-work}"
export MAX_CONCURRENT_INFERENCES="${MAX_CONCURRENT_INFERENCES:-1}"
export MAX_INPUT_BYTES="${MAX_INPUT_BYTES:-419430400}"
export INFERENCE_TIMEOUT_SECONDS="${INFERENCE_TIMEOUT_SECONDS:-1800}"

if [[ -z "${LIPSYNC_SERVICE_TOKEN:-}" ]]; then
  read -r -s -p "Set a long random LIPSYNC_SERVICE_TOKEN: " LIPSYNC_SERVICE_TOKEN
  echo
  export LIPSYNC_SERVICE_TOKEN
fi
if [[ ${#LIPSYNC_SERVICE_TOKEN} -lt 32 ]]; then
  echo "LIPSYNC_SERVICE_TOKEN must be at least 32 characters."
  exit 1
fi
cd "${DEPLOY_DIR}"
if [[ ! -x "${MAMBA_BIN}" ]]; then
  echo "GPU Python environment is missing. Run setup_colab.sh first."
  exit 1
fi
# Activate in this process before exec-ing Uvicorn.  `micromamba run` leaves a
# wrapper process, so a notebook PID file may stop the wrapper while Uvicorn
# continues serving an older server.py.  After `exec`, the recorded PID is the
# actual Uvicorn process and a restart reliably loads the updated worker.
eval "$("${MAMBA_BIN}" shell hook --shell bash)"
micromamba activate "${ENVIRONMENT_NAME}"
exec python -m uvicorn server:app --host 127.0.0.1 --port 8000
