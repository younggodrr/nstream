"""Run this file in a Google Colab cell:

    !python /content/gemini_prepare_nstream_gpu.py

It asks Gemini to produce a reviewable GPU-worker setup script for NStream.
It does not execute Gemini-generated shell commands automatically.
"""

import os
import subprocess
import sys
from getpass import getpass
from pathlib import Path


def install_sdk() -> None:
    subprocess.check_call(
        [sys.executable, "-m", "pip", "install", "-q", "-U", "google-genai"]
    )


PROMPT = r"""
You are preparing a temporary Google Colab GPU worker for an existing Rust media
platform named NStream. Generate exactly ONE reviewable bash script, enclosed in
a single ```bash code fence and nothing else. The script must create all files
under /content/nstream-gpu-worker, but must not start a public tunnel or expose
any secret.

The worker contract is strict:
  * GET /health returns JSON status.
  * POST /v1/lipsync accepts a raw video/mp4 request body.
  * It receives the target language in X-NStream-Target-Language.
  * If LIPSYNC_SERVICE_TOKEN is nonempty, require Authorization: Bearer token.
  * It returns the processed MP4 as video/mp4, or a structured 4xx/5xx error.
  * Its smoke test must prove that /health reports model-ready. A successful
    /v1/lipsync test is permitted only after real model inference, never by
    copying the input video.

Requirements:
  * Use FastAPI/Uvicorn and a pluggable LipSyncAdapter class.
  * Implement, not merely stub, a MuseTalk 1.5 adapter. Clone only the official
    repository https://github.com/TMElyralab/MuseTalk.git, install its declared
    Python dependencies, and invoke its real inference command. The adapter
    must use its documented v1.5 normal inference entry point
    (python -m scripts.inference with --version v15, models/musetalkV15/unet.pth
    and models/musetalkV15/musetalk.json). It must create a per-request
    inference YAML with the received MP4 as both video_path and audio_path,
    run the subprocess with a timeout, locate MuseTalk's generated MP4, verify
    it with ffprobe, and return that generated file.
  * Download model weights only through the repository's own
    download_weights.sh command after displaying the required disk-space warning.
    If a download requires an account, licence acceptance, or fails, stop and
    explain the manual step. Never invent download URLs or fake weight files.
  * If the repository, dependencies, GPU/CUDA, weights, or generated output are
    incomplete, /health must be unhealthy and /v1/lipsync must return a clear
    503 or 500. Never offer a mock mode and never copy the submitted video as a
    successful response.
  * Store all model paths and service tokens only in environment variables.
  * Include requirements.txt, server.py, README.md, and run_colab.sh.
  * Pin Python package versions where practical.
  * Do not modify NStream Rust code. Do not include API keys in files.
  * Clean per-request input/output files only after FileResponse has finished
    streaming: use a Starlette BackgroundTask; do not unlink the response file
    in a finally block before it is sent. Enforce a maximum input size through
    Content-Length when present and while streaming.
  * Require LIPSYNC_SERVICE_TOKEN to be nonempty before binding the service to
    0.0.0.0. Bind to 127.0.0.1 by default. Do not create an ngrok, Cloudflare,
    or other public tunnel.

The resulting service will later be reached by NStream using:
  LIPSYNC_API_URL=https://temporary-host/v1/lipsync
  LIPSYNC_API_KEY=<the same LIPSYNC_SERVICE_TOKEN>
"""


def main() -> None:
    install_sdk()
    if not os.environ.get("GEMINI_API_KEY"):
        os.environ["GEMINI_API_KEY"] = getpass("Gemini API key (not saved): ")

    from google import genai

    client = genai.Client()
    model = os.environ.get("GEMINI_MODEL", "gemini-3.5-flash")
    response = client.interactions.create(model=model, input=PROMPT)
    text = response.output_text
    if "```bash" not in text:
        raise RuntimeError("Gemini did not return the required bash script")

    output_dir = Path("/content/nstream-gpu-worker")
    output_dir.mkdir(parents=True, exist_ok=True)
    plan = output_dir / "gemini_setup.sh"
    script = text.split("```bash", 1)[1].split("```", 1)[0].strip() + "\n"
    plan.write_text(script, encoding="utf-8")
    plan.chmod(0o700)
    print(f"Saved Gemini's setup script to {plan}")
    print("Review it first, then run: bash /content/nstream-gpu-worker/gemini_setup.sh")


if __name__ == "__main__":
    main()
