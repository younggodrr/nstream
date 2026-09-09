# Complete GPU-Accelerated Dubbing Pipeline Guide

This guide shows you how to use the complete GPU-accelerated dubbing pipeline with Google Colab for your NStream project.

## What's Included

The complete dubbing pipeline includes:

- **ASR (Automatic Speech Recognition)** - Whisper for transcribing source audio
- **TTS (Text-to-Speech)** - gTTS for generating target language audio
- **Translation** - LibreTranslate for translating text between languages
- **Lip-sync** - Basic lip-sync processing (placeholder for production models)
- **GPU Acceleration** - All ML tasks run on Colab's GPU

## Architecture

```
NStream Pipeline                          Colab GPU Worker
┌─────────────────────┐              ┌─────────────────────┐
│  Source Video       │              │  ASR (Whisper)      │
│         ↓           │              │  TTS (gTTS)         │
│  ASR Transcription  │──HTTPS───▶   │  Translation        │
│         ↓           │              │  Lip-sync           │
│  Translation        │◀─────────────│  (Wav2Lip/MuseTalk) │
│         ↓           │              │                     │
│  TTS Generation     │              │  GPU Processing     │
│         ↓           │              │                     │
│  Video Rendering    │              │                     │
│  (FFmpeg)           │              │                     │
└─────────────────────┘              └─────────────────────┘
```

## Setup Instructions

### Step 1: Start the Complete Dubbing Worker in Colab

```python
# Cell 1: Install dependencies
!pip install -r requirements.txt

# Cell 2: Upload these files:
# - dubbing_gpu_worker.py
# - run_dubbing_worker.py
# - run_tunnel_background.py
# - start_complete_pipeline.py (optional - for combined startup)
```

### Step 2: Start Worker and Tunnel (Two Options)

**Option A: Start separately (recommended for debugging)**

```python
# Cell 3: Start the worker in background
!python run_dubbing_worker.py

# Cell 4: Start the tunnel in background (NEW - doesn't block!)
!python run_tunnel_background.py
```

**Option B: Start together with combined script**

```python
# Cell 3: Start both worker and tunnel together
!python start_complete_pipeline.py
```

### Step 3: Get Your Public URL

The tunnel script will display your public URL:
```
🚀 Public URL: https://abc123.trycloudflare.com
```

Copy this URL.

### Step 3: Configure Local NStream

Add these to your local `.env` file:

```bash
# Full Pipeline Configuration
FULL_PIPELINE_URL=https://abc123.trycloudflare.com
FULL_PIPELINE_API_KEY=colab-dubbing-secret
```

### Step 4: Test the Connection

```bash
# Test health endpoint
curl https://abc123.trycloudflare.com/health
```

Expected response:
```json
{
  "status": "healthy",
  "gpu_available": true,
  "model_loaded": true,
  "device": "cuda",
  "capabilities": {
    "asr": true,
    "tts": true,
    "translation": true,
    "lip_sync": false
  }
}
```

## API Endpoints

### Health Check
```bash
GET /health
```

Returns the worker status and capabilities.

### ASR (Audio Transcription)
```bash
POST /transcribe
Content-Type: multipart/form-data
X-NStream-Language: en (optional)

file: <audio_file.wav>
```

Returns transcription with segments and timing.

### TTS (Text-to-Speech)
```bash
POST /tts
Content-Type: application/json

{
  "text": "Hello world",
  "language": "en",
  "output_format": "mp3"
}
```

Returns audio file (MP3 format).

### Translation
```bash
POST /translate
Content-Type: application/json

{
  "text": "Hello world",
  "source_lang": "en",
  "target_lang": "es"
}
```

Returns translated text.

### Lip-sync
```bash
POST /lipsync
Content-Type: multipart/form-data
X-NStream-Target-Language: es

file: <video_file.mp4>
```

Returns lip-synced video (currently placeholder implementation).

## Rust Integration

Use the enhanced `InferenceManager` in your Rust code:

```rust
use nstream_inference::InferenceManager;

// Create manager (automatically uses FULL_PIPELINE_URL if set)
let manager = InferenceManager::from_env()?;

// Health check
let health = manager.health_check().await?;
println!("Pipeline status: {}", health);

// Transcribe audio
let segments = manager.transcribe_pcm(audio_samples, Some("en".to_string())).await?;

// Generate speech
let audio_data = manager.text_to_speech("Hello world", "en").await?;

// Translate text
let translated = manager.translate_text("Hello", "en", "es").await?;

// Lip-sync video
let lip_synced_video = manager.lip_sync_video(video_data, "es").await?;
```

## Workflow Example

Complete dubbing workflow:

1. **Extract audio** from source video
2. **Transcribe** using ASR endpoint
3. **Translate** text to target language
4. **Generate speech** using TTS endpoint
5. **Render video** with new audio track using FFmpeg
6. **Lip-sync** (optional) using lip-sync endpoint

## Capabilities

- ✅ **ASR**: Full Whisper transcription with segments
- ✅ **TTS**: gTTS for multiple languages
- ✅ **Translation**: LibreTranslate integration
- ⚠️ **Lip-sync**: Placeholder implementation (needs production model)

## Limitations

- **Lip-sync**: Currently a placeholder. For production, integrate with Wav2Lip or MuseTalk
- **Translation**: Uses free LibreTranslate API. For production, use Google Translate API or similar
- **TTS**: Uses gTTS. For higher quality, consider using commercial TTS services

## Next Steps for Production

1. **Replace lip-sync** with a real model (Wav2Lip, MuseTalk)
2. **Upgrade translation** to a commercial service (Google Translate, DeepL)
3. **Upgrade TTS** to a higher-quality service (ElevenLabs, Azure TTS)
4. **Add caching** for repeated translations/TTS
5. **Implement rate limiting** for API calls
6. **Add monitoring** for GPU usage and performance

## Troubleshooting

**Worker not starting:**
- Check that all dependencies are installed
- Verify GPU is available in Colab
- Check logs for specific error messages

**Translation fails:**
- LibreTranslate API may be rate-limited
- Consider using a different translation service
- Check network connectivity

**TTS quality issues:**
- gTTS provides basic quality
- Consider upgrading to commercial TTS for production
- Test different language codes

**Lip-sync returns original video:**
- This is expected - current implementation is a placeholder
- Need to integrate with a real lip-sync model for production

## Files

- `dubbing_gpu_worker.py` - Complete dubbing worker
- `run_dubbing_worker.py` - Background runner
- `setup_cloudflare_tunnel.py` - Cloudflare tunnel setup
- `nstream-inference/src/full_pipeline.rs` - Rust client
- `nstream-inference/src/manager.rs` - Enhanced inference manager