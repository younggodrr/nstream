"""NStream's authenticated MuseTalk GPU adapter.

NStream POSTs a *dubbed* MP4 to /v1/lipsync.  This service normalises it to
MuseTalk's expected input, runs real MuseTalk 1.5 inference and returns the
generated MP4. It never returns the input file as a successful result.
"""

from __future__ import annotations

import asyncio
import hmac
import logging
import os
import shutil
import subprocess
import tempfile
import time
import uuid
from pathlib import Path
from typing import Annotated

import yaml
from fastapi import FastAPI, Header, HTTPException, Request, status
from fastapi.responses import FileResponse, JSONResponse
from starlette.background import BackgroundTask

logging.basicConfig(level=os.getenv("LOG_LEVEL", "INFO"))
LOG = logging.getLogger("nstream.gpu")

ROOT = Path(os.getenv("MUSE_TALK_ROOT", "/content/nstream-gpu-deployment/MuseTalk")).resolve()
MODEL_DIR = Path(os.getenv("MUSE_TALK_MODEL_DIR", str(ROOT / "models"))).resolve()
WORK_DIR = Path(os.getenv("NSTREAM_GPU_WORK_DIR", "/content/nstream-gpu-work")).resolve()
TOKEN = os.getenv("LIPSYNC_SERVICE_TOKEN", "")
MAX_BYTES = int(os.getenv("MAX_INPUT_BYTES", str(400 * 1024 * 1024)))
TIMEOUT_SECONDS = int(os.getenv("INFERENCE_TIMEOUT_SECONDS", "1800"))
MAX_CONCURRENT = int(os.getenv("MAX_CONCURRENT_INFERENCES", "1"))
ALLOWED_LANGUAGES = set(filter(None, os.getenv("ALLOWED_TARGET_LANGUAGES", "").split(",")))
INFERENCE_SEMAPHORE = asyncio.Semaphore(MAX_CONCURRENT)
# Quick Tunnels deliberately close long-running HTTP requests.  These records
# let clients upload once, then poll short-lived requests until inference has
# completed.  Colab runtimes are ephemeral, so this is intentionally in-memory.
JOBS: dict[str, dict[str, object]] = {}

app = FastAPI(title="NStream MuseTalk GPU Worker", version="1.0")


def paths_ready() -> tuple[bool, str]:
    required = [
        ROOT / "scripts" / "inference.py",
        ROOT / "configs" / "inference" / "test.yaml",
        MODEL_DIR / "musetalkV15" / "musetalk.json",
        MODEL_DIR / "musetalkV15" / "unet.pth",
    ]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        return False, "missing required MuseTalk files: " + ", ".join(missing)
    if not shutil.which("ffmpeg") or not shutil.which("ffprobe"):
        return False, "ffmpeg and ffprobe are required"
    if not TOKEN:
        return False, "LIPSYNC_SERVICE_TOKEN is not configured"
    return True, "MuseTalk 1.5 ready"


def command(
    args: list[str], timeout: int = 120, cwd: Path | None = None
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout,
        cwd=str(cwd) if cwd else None,
    )


def valid_video(path: Path) -> tuple[bool, str]:
    probe = command([
        "ffprobe", "-v", "error", "-show_entries", "format=duration",
        "-show_streams", "-of", "json", str(path),
    ])
    if probe.returncode != 0:
        return False, probe.stderr[-800:]
    if not any(stream.get("codec_type") == "video" for stream in __import__("json").loads(probe.stdout).get("streams", [])):
        return False, "no video stream"
    return True, "ok"


def authenticate(authorization: str | None) -> None:
    if not TOKEN:
        raise HTTPException(status.HTTP_503_SERVICE_UNAVAILABLE, "lip-sync service token is not configured")
    expected = f"Bearer {TOKEN}"
    if not authorization or not hmac.compare_digest(authorization, expected):
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "invalid or missing bearer token")


def prepare_request(request_dir: Path, source: Path) -> tuple[Path, Path, Path]:
    normalised = request_dir / "input-25fps.mp4"
    normalise = command([
        "ffmpeg", "-y", "-v", "error", "-i", str(source), "-map", "0:v:0", "-map", "0:a:0?",
        "-r", "25", "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-movflags", "+faststart", str(normalised),
    ], timeout=900)
    if normalise.returncode != 0:
        raise RuntimeError("FFmpeg normalisation failed: " + normalise.stderr[-1200:])

    # MuseTalk expects nested task blocks (task_0.video_path / audio_path). Writing
    # top-level keys leaves the stock demo tasks in place and MuseTalk ignores the
    # uploaded video — that is what returned the wrong anime clip for 07bd08bb.
    audio = request_dir / "input-audio.wav"
    extract = command([
        "ffmpeg", "-y", "-v", "error", "-i", str(normalised),
        "-vn", "-acodec", "pcm_s16le", "-ar", "16000", "-ac", "1", str(audio),
    ], timeout=300)
    if extract.returncode != 0:
        raise RuntimeError("FFmpeg audio extract failed: " + extract.stderr[-1200:])
    if not audio.is_file() or audio.stat().st_size == 0:
        raise RuntimeError("normalised input has no usable audio track for MuseTalk")

    config = {
        "task_0": {
            "video_path": str(normalised),
            "audio_path": str(audio),
        }
    }
    config_path = request_dir / "inference.yaml"
    with config_path.open("w", encoding="utf-8") as handle:
        yaml.safe_dump(config, handle, sort_keys=False)
    result_dir = request_dir / "result"
    return normalised, config_path, result_dir


def run_musetalk(source: Path, request_dir: Path) -> Path:
    _, config_path, result_dir = prepare_request(request_dir, source)
    process = command(
        [
            "python3", "-m", "scripts.inference", "--inference_config", str(config_path),
            "--result_dir", str(result_dir), "--unet_model_path", str(MODEL_DIR / "musetalkV15" / "unet.pth"),
            "--unet_config", str(MODEL_DIR / "musetalkV15" / "musetalk.json"), "--version", "v15",
            "--ffmpeg_path", shutil.which("ffmpeg") or "ffmpeg",
        ],
        timeout=TIMEOUT_SECONDS,
        cwd=ROOT,
    )
    if process.returncode != 0:
        raise RuntimeError("MuseTalk inference failed: " + process.stderr[-2000:])
    # Prefer the lip-synced MP4; MuseTalk also writes a side-by-side *_concat.mp4.
    candidates = sorted(
        (
            path
            for path in result_dir.rglob("*.mp4")
            if not path.name.endswith("_concat.mp4")
        ),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not candidates:
        raise RuntimeError("MuseTalk completed without producing an MP4")
    generated = candidates[0]
    valid, detail = valid_video(generated)
    if not valid:
        raise RuntimeError("MuseTalk output validation failed: " + detail)
    return generated


@app.get("/health")
async def health() -> dict[str, object]:
    ready, detail = paths_ready()
    return {
        "status": "healthy" if ready else "unhealthy",
        "ready": ready,
        "model": "MuseTalk 1.5",
        "max_concurrent_inferences": MAX_CONCURRENT,
        "detail": detail,
    }


@app.get("/v1/capabilities")
async def capabilities(authorization: Annotated[str | None, Header()] = None) -> dict[str, object]:
    authenticate(authorization)
    ready, _ = paths_ready()
    return {
        "lip_sync": ready,
        "async_jobs": True,
        "upscale": False,
        "vfx": False,
        "input": "video/mp4",
        "output": "video/mp4",
    }


async def receive_video(request: Request, language: str) -> tuple[Path, Path]:
    length = request.headers.get("content-length")
    if length and (not length.isdigit() or int(length) > MAX_BYTES):
        raise HTTPException(status.HTTP_413_REQUEST_ENTITY_TOO_LARGE, "input exceeds MAX_INPUT_BYTES")
    WORK_DIR.mkdir(parents=True, exist_ok=True)
    request_dir = Path(tempfile.mkdtemp(prefix="nstream-lipsync-", dir=WORK_DIR))
    source = request_dir / "source.mp4"
    received = 0
    try:
        with source.open("wb") as handle:
            async for chunk in request.stream():
                received += len(chunk)
                if received > MAX_BYTES:
                    raise HTTPException(status.HTTP_413_REQUEST_ENTITY_TOO_LARGE, "input exceeds MAX_INPUT_BYTES")
                handle.write(chunk)
        if not received:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "empty MP4 request")
        valid, detail = valid_video(source)
        if not valid:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "invalid input MP4: " + detail)
        return request_dir, source
    except Exception:
        shutil.rmtree(request_dir, ignore_errors=True)
        raise


async def run_job(job_id: str, request_dir: Path, source: Path, language: str) -> None:
    record = JOBS[job_id]
    record["status"] = "running"
    started = time.monotonic()
    try:
        async with INFERENCE_SEMAPHORE:
            generated = await asyncio.to_thread(run_musetalk, source, request_dir)
        record.update(status="completed", output=generated)
        LOG.info("lip-sync job completed id=%s language=%s seconds=%.2f", job_id, language, time.monotonic() - started)
    except Exception as error:
        LOG.exception("lip-sync job failed id=%s", job_id)
        record.update(status="failed", error=str(error))


def clean_job(job_id: str, request_dir: Path) -> None:
    JOBS.pop(job_id, None)
    shutil.rmtree(request_dir, ignore_errors=True)


@app.post("/v1/lipsync/jobs", status_code=status.HTTP_202_ACCEPTED, response_model=None)
async def create_lip_sync_job(
    request: Request,
    authorization: Annotated[str | None, Header()] = None,
    x_nstream_target_language: Annotated[str | None, Header()] = None,
) -> JSONResponse:
    authenticate(authorization)
    ready, detail = paths_ready()
    if not ready:
        raise HTTPException(status.HTTP_503_SERVICE_UNAVAILABLE, detail)
    if request.headers.get("content-type", "").split(";", 1)[0].lower() != "video/mp4":
        raise HTTPException(status.HTTP_415_UNSUPPORTED_MEDIA_TYPE, "expected Content-Type: video/mp4")
    language = (x_nstream_target_language or "").strip().lower()
    if not language:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "X-NStream-Target-Language is required")
    if ALLOWED_LANGUAGES and language not in ALLOWED_LANGUAGES:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "target language is not enabled")
    request_dir, source = await receive_video(request, language)
    job_id = str(uuid.uuid4())
    JOBS[job_id] = {"status": "queued", "request_dir": request_dir}
    asyncio.create_task(run_job(job_id, request_dir, source, language))
    return JSONResponse({"job_id": job_id, "status": "queued"}, status_code=status.HTTP_202_ACCEPTED)


@app.get("/v1/lipsync/jobs/{job_id}", response_model=None)
async def get_lip_sync_job(
    job_id: str,
    authorization: Annotated[str | None, Header()] = None,
):
    authenticate(authorization)
    record = JOBS.get(job_id)
    if not record:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "lip-sync job not found")
    job_status = str(record["status"])
    if job_status == "completed":
        output = record.get("output")
        request_dir = record["request_dir"]
        if not isinstance(output, Path) or not isinstance(request_dir, Path):
            raise HTTPException(status.HTTP_500_INTERNAL_SERVER_ERROR, "invalid completed job state")
        return FileResponse(output, media_type="video/mp4", background=BackgroundTask(clean_job, job_id, request_dir))
    if job_status == "failed":
        return JSONResponse({"job_id": job_id, "status": "failed", "error": record.get("error", "inference failed")}, status_code=status.HTTP_500_INTERNAL_SERVER_ERROR)
    return JSONResponse({"job_id": job_id, "status": job_status}, status_code=status.HTTP_202_ACCEPTED)


@app.post("/v1/lipsync")
async def lip_sync(
    request: Request,
    authorization: Annotated[str | None, Header()] = None,
    x_nstream_target_language: Annotated[str | None, Header()] = None,
) -> FileResponse:
    authenticate(authorization)
    ready, detail = paths_ready()
    if not ready:
        raise HTTPException(status.HTTP_503_SERVICE_UNAVAILABLE, detail)
    if request.headers.get("content-type", "").split(";", 1)[0].lower() != "video/mp4":
        raise HTTPException(status.HTTP_415_UNSUPPORTED_MEDIA_TYPE, "expected Content-Type: video/mp4")
    language = (x_nstream_target_language or "").strip().lower()
    if not language:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "X-NStream-Target-Language is required")
    if ALLOWED_LANGUAGES and language not in ALLOWED_LANGUAGES:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "target language is not enabled")
    length = request.headers.get("content-length")
    if length and (not length.isdigit() or int(length) > MAX_BYTES):
        raise HTTPException(status.HTTP_413_REQUEST_ENTITY_TOO_LARGE, "input exceeds MAX_INPUT_BYTES")

    WORK_DIR.mkdir(parents=True, exist_ok=True)
    request_dir = Path(tempfile.mkdtemp(prefix="nstream-lipsync-", dir=WORK_DIR))
    source = request_dir / "source.mp4"
    received = 0
    try:
        with source.open("wb") as handle:
            async for chunk in request.stream():
                received += len(chunk)
                if received > MAX_BYTES:
                    raise HTTPException(status.HTTP_413_REQUEST_ENTITY_TOO_LARGE, "input exceeds MAX_INPUT_BYTES")
                handle.write(chunk)
        if not received:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "empty MP4 request")
        valid, detail = valid_video(source)
        if not valid:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "invalid input MP4: " + detail)
        started = time.monotonic()
        async with INFERENCE_SEMAPHORE:
            generated = await asyncio.to_thread(run_musetalk, source, request_dir)
        LOG.info("lip-sync completed bytes=%s language=%s seconds=%.2f", received, language, time.monotonic() - started)
        # FileResponse streams asynchronously; cleanup must run after the body
        # is sent, not in a finally block before the client receives it.
        return FileResponse(generated, media_type="video/mp4", background=BackgroundTask(shutil.rmtree, request_dir, True))
    except HTTPException:
        shutil.rmtree(request_dir, ignore_errors=True)
        raise
    except subprocess.TimeoutExpired:
        shutil.rmtree(request_dir, ignore_errors=True)
        raise HTTPException(status.HTTP_504_GATEWAY_TIMEOUT, "MuseTalk inference timed out")
    except Exception as error:
        LOG.exception("lip-sync failed")
        shutil.rmtree(request_dir, ignore_errors=True)
        raise HTTPException(status.HTTP_500_INTERNAL_SERVER_ERROR, f"MuseTalk inference failed: {error}") from error
