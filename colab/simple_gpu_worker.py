"""
Simple GPU Worker for NStream - Run this in Google Colab
This provides a simple HTTP API for GPU-accelerated audio transcription
"""

import os
import tempfile
from typing import Optional
from fastapi import FastAPI, UploadFile, File, HTTPException, Header
from fastapi.responses import JSONResponse
from pydantic import BaseModel
import uvicorn
import torch
import whisper
from contextlib import asynccontextmanager

# Global model variable
whisper_model = None

class TranscriptionRequest(BaseModel):
    language: Optional[str] = "en"

class TranscriptionSegment(BaseModel):
    start_ms: float
    end_ms: float
    text: str

class TranscriptionResponse(BaseModel):
    text: str
    language: str
    duration: float
    segments: list[TranscriptionSegment]

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Load the model on startup
    global whisper_model
    print("Loading Whisper model...")
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Using device: {device}")
    
    # Load base model (change to 'large' for more accuracy but slower)
    whisper_model = whisper.load_model("base", device=device)
    print("Model loaded successfully!")
    yield
    # Cleanup
    print("Shutting down...")

app = FastAPI(lifespan=lifespan)

@app.get("/health")
async def health_check():
    """Health check endpoint"""
    gpu_available = torch.cuda.is_available()
    return {
        "status": "healthy",
        "gpu_available": gpu_available,
        "model_loaded": whisper_model is not None,
        "device": "cuda" if gpu_available else "cpu"
    }

@app.post("/transcribe")
async def transcribe_audio(
    file: UploadFile = File(...),
    language: Optional[str] = Header(None, alias="X-NStream-Language")
):
    """
    Transcribe audio file using Whisper
    Accepts WAV audio files and returns transcription
    """
    if whisper_model is None:
        raise HTTPException(status_code=503, detail="Model not loaded")
    
    temp_file_path = None
    try:
        # Save uploaded file to temporary location
        with tempfile.NamedTemporaryFile(delete=False, suffix=".wav") as temp_file:
            content = await file.read()
            temp_file.write(content)
            temp_file_path = temp_file.name
        
        # Transcribe using Whisper
        result = whisper_model.transcribe(
            temp_file_path,
            language=language if language else None
        )
        
        # Calculate actual duration from segments if available
        duration = 0.0
        segments = []
        if "segments" in result and result["segments"]:
            for seg in result["segments"]:
                segments.append(TranscriptionSegment(
                    start_ms=seg.get("start", 0.0) * 1000,  # Convert to milliseconds
                    end_ms=seg.get("end", 0.0) * 1000,
                    text=seg.get("text", "").strip()
                ))
            if segments:
                duration = segments[-1].end_ms / 1000.0  # Convert back to seconds
        
        # Clean up temporary file
        os.unlink(temp_file_path)
        
        return TranscriptionResponse(
            text=result["text"],
            language=result["language"],
            duration=duration,
            segments=segments
        )
        
    except Exception as e:
        # Clean up file if it exists
        if temp_file_path and os.path.exists(temp_file_path):
            os.unlink(temp_file_path)
        raise HTTPException(status_code=500, detail=f"Transcription failed: {str(e)}")

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8000)