# NStream + Colab GPU Worker Integration Guide

## Status
✅ GPU worker running in Colab with CUDA support  
✅ Remote inference client added to nstream-inference  
✅ Environment configuration prepared  

## Next Steps

### 1. Start Colab Worker in Background

**Important:** Run the worker in background so it doesn't block other cells:

```python
# Cell 1: Start worker in background
!python run_worker_background.py
```

This will start the GPU worker and keep it running while allowing you to execute other cells.

If you need to stop it later:
```python
# Cell to stop worker
!python run_worker_background.py stop
```

### 2. Create Cloudflare Tunnel (No Account Required)

```python
# Cell 2: Create Cloudflare tunnel to access worker from your local machine
!python setup_cloudflare_tunnel.py
```

This will:
- Install cloudflared (no account needed)
- Start a secure tunnel
- Display your public URL

Copy the URL shown - you'll need it for the environment configuration.

**Alternative: If you prefer ngrok with authentication:**
```python
from pyngrok import ngrok
ngrok.set_auth_token("YOUR_NGROK_TOKEN")  # Get from ngrok.com
public_url = ngrok.connect(8000)
print(f"🚀 Public URL: {public_url}")
```

### 3. Add Environment Variables to Your Local Project

Add these to your local `.env` file:

```bash
# Remote GPU Worker Configuration
REMOTE_INFERENCE_URL=https://your-cloudflare-url.trycloudflare.com
REMOTE_INFERENCE_API_KEY=your-secure-token-here
```

**Note**: 
- Replace `your-cloudflare-url.trycloudflare.com` with the actual URL from your Colab Cloudflare tunnel
- For the API key, you can use any secure string if you're not implementing authentication on the worker side

### 4. Update NStream Code to Use Remote Inference

The new `InferenceManager` can automatically switch between local and remote inference based on environment variables.

In your code where you currently use `WhisperAsr`, replace it with:

```rust
use nstream_inference::InferenceManager;

// Create manager (automatically uses REMOTE_INFERENCE_URL if set)
let manager = InferenceManager::from_env()?;

// Health check
let health = manager.health_check().await?;
println!("Inference backend: {}", health);

// Transcribe audio
let segments = manager.transcribe_pcm(audio_samples, Some("en".to_string())).await?;
```

### 5. Test the Integration

Test from your local machine:

```bash
# Test health endpoint (replace with your actual URL)
curl https://your-cloudflare-url.trycloudflare.com/health

# Expected response:
# {
#   "status": "healthy",
#   "gpu_available": true,
#   "model_loaded": true,
#   "device": "cuda"
# }
```

### 6. Rebuild Your Rust Project

```bash
cd /Volumes/Raincorpfiles/nstream_mvp
cargo build
```

The new dependencies (reqwest, serde) will be automatically downloaded.

## Architecture

```
Local Machine                          Colab (GPU)
┌─────────────────────┐              ┌─────────────────────┐
│  NStream API        │              │  GPU Worker         │
│  (Rust)             │──Cloudflare──▶│  (Python/FastAPI)   │
│                     │      tunnel   │                     │
│  InferenceManager   │◀─────────────│  Whisper + CUDA     │
│  - Remote client    │   HTTPS      │  - GPU accel        │
│                     │              │                     │
│  PostgreSQL         │              │                     │
│  MinIO              │              │                     │
└─────────────────────┘              └─────────────────────┘
```

## Benefits

✅ **Local data stays local** - Your database and media files remain on your machine  
✅ **GPU acceleration** - Leverage Colab's free GPU for heavy ML tasks  
✅ **Easy fallback** - Switch between local and remote inference via env vars  
✅ **Cost effective** - No need for local GPU hardware  
✅ **Scalable** - Can add multiple Colab workers if needed  

## Troubleshooting

**Connection refused from local machine:**
- Verify Cloudflare tunnel is running in Colab
- Check that the URL is correct
- Try accessing the health endpoint from your browser
- Make sure the GPU worker is still running in background

**Rust build fails:**
- Make sure you have the latest Rust toolchain: `rustup update`
- Clean build: `cargo clean && cargo build`

**Slow transcription:**
- Check that Colab is actually using GPU (not CPU)
- Verify network connection between local machine and Colab
- Consider using a smaller Whisper model in Colab for faster processing

## Next Steps for Production

1. **Add authentication** to the GPU worker to secure the endpoint
2. **Implement retry logic** for network failures
3. **Add monitoring** to track GPU usage and transcription performance  
4. **Consider multiple workers** for parallel processing
5. **Add caching** for repeated transcriptions