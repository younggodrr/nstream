# Google Colab GPU Worker Setup for NStream

This guide shows you how to use Google Colab as a GPU worker for your local NStream project.

## Prerequisites

- Local NStream project running with Docker (PostgreSQL + MinIO)
- Google account for Colab access
- No account required for Cloudflare tunnel (recommended)

## Quick Setup (5 minutes)

### Step 1: Create Colab Notebook

1. Go to [colab.research.google.com](https://colab.research.google.com/)
2. Create new notebook
3. **Enable GPU**: Runtime → Change runtime type → Hardware accelerator → GPU

### Step 2: Upload the Worker Scripts

Upload these files from the `colab/` folder to your Colab notebook:
- `simple_gpu_worker.py` 
- `run_worker_background.py`
- `setup_cloudflare_tunnel.py`

### Step 3: Install Dependencies and Start Worker

Run these cells in your Colab notebook:

```python
# Cell 1: Install dependencies
!pip install -r requirements.txt

# Cell 2: Upload the worker scripts (simple_gpu_worker.py, run_worker_background.py, setup_cloudflare_tunnel.py)

# Cell 3: Start the worker in background (IMPORTANT - doesn't block other cells)
!python run_worker_background.py
```

### Step 4: Create Secure Tunnel

**Option 1: Cloudflare Tunnel (Recommended - No Account Required)**

```python
# Cell 4: Create Cloudflare tunnel
!python setup_cloudflare_tunnel.py
```

**Option 2: Ngrok (Requires Account)**

```python
# Cell 4: Create ngrok tunnel
from pyngrok import ngrok

# Get your auth token from https://ngrok.com/
ngrok.set_auth_token("YOUR_NGROK_TOKEN")

public_url = ngrok.connect(8000)
print(f"GPU Worker accessible at: {public_url}")
print(f"Health check: {public_url}/health")
```

### Step 5: Configure Local NStream

Update your local `.env` file:

```bash
# Add these GPU worker configuration
REMOTE_INFERENCE_URL=https://your-cloudflare-url.trycloudflare.com
REMOTE_INFERENCE_API_KEY=your-secure-token-here
```

### Step 6: Test the Connection

```bash
# From your local machine (replace with your actual URL)
curl https://your-cloudflare-url.trycloudflare.com/health
```

You should see:
```json
{
  "status": "healthy",
  "gpu_available": true,
  "model_loaded": true,
  "device": "cuda"
}
```

## Integration with NStream

The worker provides these endpoints:

- `GET /health` - Health check and GPU status
- `POST /transcribe` - Audio transcription using Whisper

To integrate with your NStream project, you'll need to modify the inference module to call these endpoints instead of running local GPU operations.

## Troubleshooting

**GPU not available in Colab:**
- Make sure you selected GPU in runtime settings
- Restart runtime after changing hardware accelerator

**Connection issues:**
- Check that ngrok/cloudflare tunnel is running
- Verify your local machine can access the public URL
- Check firewall settings

**Model loading issues:**
- First run will download Whisper model (takes time)
- Subsequent runs will be faster
- Consider using smaller model if GPU memory is limited

## Quick Start (Non-blocking Approach)

**Problem:** Running `!python simple_gpu_worker.py` blocks the cell and prevents other cells from executing.

**Solution:** Use the background runner:

```python
# Cell 1: Install dependencies
!pip install -r requirements.txt

# Cell 2: Upload both simple_gpu_worker.py and run_worker_background.py

# Cell 3: Start worker in background (doesn't block!)
!python run_worker_background.py

# Cell 4: Create Cloudflare tunnel (can run while worker is running)
!python setup_cloudflare_tunnel.py

# Cell 5: Continue with other work - worker keeps running!
print("You can execute other cells now")
```

To stop the worker when done:
```python
!python run_worker_background.py stop
```

## Advantages of This Approach

✅ **Easy setup** - No complex Rust compilation in Colab
✅ **Local database** - Your data stays on your local machine
✅ **Cost effective** - Use free Colab GPU tier
✅ **Flexible** - Can easily switch between different GPU models
✅ **Secure** - Your main application logic stays local

## Next Steps

1. Test the basic worker setup
2. Integrate the transcription endpoint with your NStream workflow
3. Add additional GPU services as needed (lip-sync, video processing, etc.)
4. Consider setting up multiple Colab workers for scaling