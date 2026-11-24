#!/usr/bin/env bash
# Run once in a Google Colab T4 runtime. Downloads real MuseTalk 1.5 model
# weights through its upstream repository; it does not create any public URL.
set -euo pipefail

# Colab exposes its notebook-only inline Matplotlib backend through the
# environment. MuseTalk is a headless HTTP worker, so force the portable
# renderer before importing OpenMMLab/xtcocotools.
export MPLBACKEND=Agg

DEPLOY_DIR="${NSTREAM_GPU_DEPLOY_DIR:-/content/nstream-gpu-deployment}"
MUSE_TALK_ROOT="${MUSE_TALK_ROOT:-${DEPLOY_DIR}/MuseTalk}"
MAMBA_ROOT_PREFIX="${MAMBA_ROOT_PREFIX:-${DEPLOY_DIR}/mamba}"
MAMBA_BIN="${MAMBA_ROOT_PREFIX}/bin/micromamba"
ENVIRONMENT_NAME="${NSTREAM_GPU_ENV:-nstream-gpu}"
SUCCESS_MARKER="${MAMBA_ROOT_PREFIX}/envs/${ENVIRONMENT_NAME}/.nstream-musetalk-ready"

if ! command -v nvidia-smi >/dev/null 2>&1; then
  echo "No NVIDIA GPU found. In Colab choose Runtime → Change runtime type → T4 GPU, then reconnect."
  exit 1
fi
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader
df -h /content

if [[ ! -f "${DEPLOY_DIR}/server.py" ]]; then
  echo "Expected ${DEPLOY_DIR}/server.py. Upload/copy the gpu-deployment folder first."
  exit 1
fi

# Current Colab images may use Python 3.13. MuseTalk pins NumPy 1.23.5, which
# does not support Python 3.13, so never install its dependencies into Colab's
# base interpreter. Use a dedicated Python 3.10 environment instead.
mkdir -p "${MAMBA_ROOT_PREFIX}/bin"
if [[ ! -x "${MAMBA_BIN}" ]]; then
  curl --fail --location --retry 3 https://micro.mamba.pm/api/micromamba/linux-64/latest \
    | tar -xvj -C "${MAMBA_ROOT_PREFIX}/bin" --strip-components=1 bin/micromamba
fi
export MAMBA_ROOT_PREFIX
# A failed install can leave incompatible current-Colab Torch/MMCV packages in
# the environment. Only reuse a prior environment after this script completed
# successfully; otherwise recreate it from the known MuseTalk compatibility
# matrix below.
if [[ ! -f "${SUCCESS_MARKER}" ]]; then
  rm -rf "${MAMBA_ROOT_PREFIX}/envs/${ENVIRONMENT_NAME}"
  "${MAMBA_BIN}" create --yes --name "${ENVIRONMENT_NAME}" python=3.10 pip
fi
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install --upgrade pip
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install -r "${DEPLOY_DIR}/requirements.txt"

if [[ ! -d "${MUSE_TALK_ROOT}/.git" ]]; then
  git clone --depth 1 https://github.com/TMElyralab/MuseTalk.git "${MUSE_TALK_ROOT}"
fi
cd "${MUSE_TALK_ROOT}"

# MuseTalk's official compatibility set is Python 3.10, Torch 2.0.1 and CUDA
# 11.8. Pin it before the rest of the packages so modern Colab Torch/CUDA 13
# packages cannot make OpenMMLab fall back to compiling MMCV from source.
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install --upgrade "setuptools>=77.0.3" "rich>=13.8.0"
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install \
  torch==2.0.1 torchvision==0.15.2 torchaudio==2.0.2 \
  --index-url https://download.pytorch.org/whl/cu118

# The upstream repository declares the remaining model dependencies.
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install -r requirements.txt
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install --no-cache-dir -U openmim
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install mmengine
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install "mmcv==2.0.1" \
  -f https://download.openmmlab.com/mmcv/dist/cu118/torch2.0/index.html
# mmpose 1.1 declares `chumpy`, an unmaintained optional 3-D body-model
# package. Its legacy build metadata is rejected by current packaging tools,
# but MuseTalk uses the 2-D face-landmark path and does not import it. Install
# the required pose stack explicitly, then install mmpose without that unused
# extra so Colab does not attempt a source build of chumpy.
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install "mmdet==3.1.0"
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install \
  "json_tricks>=3.17.3" "munkres" "pycocotools" "xtcocotools>=1.12"
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install --no-deps "mmpose==1.1.0"
"${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -c \
  "import torch, mmcv, mmdet; from mmpose.apis import init_model; print('OpenMMLab stack ready:', torch.__version__, mmcv.__version__)"

if [[ ! -f "models/musetalkV15/unet.pth" ]]; then
  echo "Downloading official MuseTalk weights. This is several GB and may require accepting upstream model terms."
  # Pin the downloader to the last compatible Hugging Face CLI. Newer 2.x
  # releases conflict with MuseTalk's Transformers pin and retired its old CLI
  # entry point. Current gdown removed --id, so amend only that invocation.
  "${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" python -m pip install --upgrade \
    "huggingface_hub[cli]==0.30.2" "gdown>=5,<7"
  sed -i.bak \
    -e 's/"huggingface_hub\[cli\]"/"huggingface_hub[cli]==0.30.2"/' \
    -e 's/gdown --id /gdown /' \
    -e 's|^export HF_ENDPOINT=.*|export HF_ENDPOINT=https://huggingface.co|' \
    ./download_weights.sh
  "${MAMBA_BIN}" run --name "${ENVIRONMENT_NAME}" bash ./download_weights.sh
fi

test -f models/musetalkV15/unet.pth
test -f models/musetalkV15/musetalk.json
test -f models/sd-vae/config.json
test -f models/sd-vae/diffusion_pytorch_model.bin
test -f models/whisper/config.json
test -f models/whisper/pytorch_model.bin
test -f models/whisper/preprocessor_config.json
test -f models/dwpose/dw-ll_ucoco_384.pth
test -f models/syncnet/latentsync_syncnet.pt
test -f models/face-parse-bisent/79999_iter.pth
test -f models/face-parse-bisent/resnet18-5c106cde.pth
mkdir -p /content/nstream-gpu-work
touch "${SUCCESS_MARKER}"
echo "MuseTalk installation complete. Next run start_colab.sh with a fresh service token."
