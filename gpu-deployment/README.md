# NStream GPU deployment

This package deploys a real, authenticated MuseTalk 1.5 lip-sync adapter to a
temporary GPU runtime. It is designed for a free Colab T4 test—not production.
It implements the existing Rust endpoint contract exactly:

```text
POST /v1/lipsync
Content-Type: video/mp4
Authorization: Bearer <LIPSYNC_SERVICE_TOKEN>
X-NStream-Target-Language: sw
→ video/mp4
```

For a Colab **Quick Tunnel**, NStream uses the worker's asynchronous job
contract instead: it uploads to `POST /v1/lipsync/jobs`, then polls
`GET /v1/lipsync/jobs/{job_id}`. This prevents Cloudflare's short request
deadline from discarding a successful multi-minute MuseTalk render. Set
`LIPSYNC_ASYNC_POLLING=true` in NStream's `.env` when using this package.

The MP4 already contains NStream's synthesized dubbed audio. The worker uses
that audio for lip-sync. It performs real MuseTalk inference or returns a clear
error; it never returns the source video as a fake successful result.

## Important limitations

- Colab sessions, disk and public Quick Tunnel URLs are temporary.
- A free T4 is suitable for a short fixture test, not concurrent production
  work or real-time live transformation.
- Check the upstream MuseTalk licence and any model terms before commercial
  use. The setup script uses the official repository and its weight script.

## Colab use

1. Create a Colab notebook and choose **Runtime → Change runtime type → T4 GPU**.
2. Upload `gpu-deployment.zip` to Colab's `/content` using the Files panel,
   then open/upload `NStream_GPU_Colab.ipynb`. The notebook extracts the archive
   to `/content/nstream-gpu-deployment` before setup.
3. Run `bash /content/nstream-gpu-deployment/setup_colab.sh`. It creates an
   isolated Python 3.10 environment because current Colab base images may use
   Python 3.13, which is incompatible with MuseTalk's pinned NumPy release. It
   also pins Torch 2.0.1/CUDA 11.8 and the matching prebuilt MMCV wheel; this
   avoids a source compilation failure on newer Colab CUDA 13/Torch images.
4. Run `bash /content/nstream-gpu-deployment/start_colab.sh`, set a new 32+
   character service token and leave the cell running.
5. In a second cell run `bash /content/nstream-gpu-deployment/start_tunnel.sh`.
   It prints a temporary `https://…trycloudflare.com` URL.

On the machine running NStream, add the URL and the same token to `.env`:

```env
LIPSYNC_API_URL=https://your-subdomain.trycloudflare.com/v1/lipsync
LIPSYNC_API_KEY=the-same-long-random-token
LIPSYNC_REQUIRED=true
LIPSYNC_ASYNC_POLLING=true
```

Then restart **only `nstream-worker`**. The API does not need the GPU token.
Use `python check_connection.py` with `LIPSYNC_API_URL` and `LIPSYNC_API_KEY`
to verify `/health` and `/v1/capabilities` before submitting a job.

## Running NStream in Docker

Set the three variables above in the root `.env`, then run:

```bash
docker compose up -d --build nstream-worker
```

The worker will make the remote request only after it has rendered the dubbed
MP4. With `LIPSYNC_REQUIRED=true`, a lost tunnel or failed GPU worker fails the
job safely instead of delivering an unmodified lip-sync artifact.

## Security model

- The service listens only on `127.0.0.1`; the tunnel exposes it temporarily.
- A non-empty bearer token is mandatory for inference and capabilities calls.
- Inputs are bounded, streamed to disk, validated with FFprobe, then cleaned up
  after the output stream ends.
- No token, account password, OAuth credential, or model access token is stored
  in this repository.

## Production replacement

Move the same service into a container on a managed GPU node, put it behind an
authenticated private network or mTLS-enabled ingress, use a secret manager,
and run a real end-to-end fixture/QC test. Do not rely on Colab or Quick Tunnels
for production availability.
