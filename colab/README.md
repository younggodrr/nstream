# Colab GPU test helper

1. Create a new GPU-enabled Google Colab notebook.
2. Upload `gemini_prepare_nstream_gpu.py` from this folder.
3. Run `!python gemini_prepare_nstream_gpu.py`.
4. Enter a restricted Gemini API key only in Colab's secret/runtime prompt.
5. Review `/content/nstream-gpu-worker/gemini_setup.sh` before running it.

The generator now requires an actual MuseTalk 1.5 inference implementation. If
Gemini produces a `Mock Mode`, a `NotImplementedError`, or an endpoint that
copies the submitted MP4, discard that output and do not run it.

The generated worker must return a real lip-synchronised MP4 or a clear error;
it must never silently return the source video as a successful result. Once it
is running behind a temporary authenticated HTTPS tunnel, set the resulting
`LIPSYNC_API_URL` and matching `LIPSYNC_API_KEY` in NStream's `.env`.
