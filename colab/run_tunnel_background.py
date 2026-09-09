"""
Run Cloudflare tunnel in background - use this in Colab to avoid blocking cells
"""

import subprocess
import time
import threading
import requests
import os
from pathlib import Path
import re

def install_cloudflared():
    """Install cloudflared binary"""
    print("Installing cloudflared...")
    
    # Check if already installed
    try:
        result = subprocess.run(["cloudflared", "--version"], capture_output=True, text=True)
        if result.returncode == 0:
            print("✅ cloudflared already installed")
            return True
    except:
        pass
    
    # Download cloudflared
    subprocess.run([
        "wget", "-q", 
        "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64",
        "-O", "/tmp/cloudflared"
    ], check=True)
    
    # Make executable
    subprocess.run(["chmod", "+x", "/tmp/cloudflared"], check=True)
    
    # Move to path
    subprocess.run(["sudo", "mv", "/tmp/cloudflared", "/usr/local/bin/cloudflared"], check=True)
    
    print("✅ cloudflared installed successfully")
    return True

def start_cloudflare_tunnel(port=8000):
    """Start Cloudflare tunnel in background"""
    print(f"Starting Cloudflare tunnel for port {port}...")
    
    # Start tunnel in background
    process = subprocess.Popen(
        ["cloudflared", "tunnel", "--url", f"http://localhost:{port}"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # Wait for tunnel to start and capture URL
    print("Waiting for tunnel to start...")
    time.sleep(5)
    
    # Try to get the URL from the output
    url = None
    for _ in range(15):
        # Check if process is still running
        if process.poll() is not None:
            print("❌ Tunnel process died")
            print("Stderr:", process.stderr.read())
            return None, None
        
        # Try to read output to find URL
        try:
            # Non-blocking read
            import select
            if select.select([process.stderr], [], [], 0.1)[0]:
                line = process.stderr.readline()
                print(f"Tunnel output: {line.strip()}")
                if "https://" in line:
                    # Extract URL from line like "https://xxxxx.trycloudflare.com"
                    url_match = re.search(r'https://[a-zA-Z0-9\-\.]+\.trycloudflare\.com', line)
                    if url_match:
                        url = url_match.group()
                        print(f"✅ Tunnel started successfully!")
                        print(f"🚀 Public URL: {url}")
                        return url, process
        except:
            pass
        
        time.sleep(1)
    
    # If we couldn't find URL in output, process might still be working
    print("⚠️ Could not find URL in output, but tunnel might be running")
    print("Check the stderr output above for the URL")
    return None, process

def stop_tunnel():
    """Stop the background tunnel"""
    try:
        result = subprocess.run(["pkill", "-f", "cloudflared"])
        print("✅ Stopped existing tunnel processes")
    except:
        print("No tunnel processes to stop")

if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "stop":
        stop_tunnel()
    else:
        # Install and start
        install_cloudflared()
        url, process = start_cloudflare_tunnel()
        
        if url:
            print(f"\n🎉 Setup complete!")
            print(f"Your GPU worker is accessible at: {url}")
            print(f"Health check: {url}/health")
            print(f"Transcription endpoint: {url}/transcribe")
            print(f"TTS endpoint: {url}/tts")
            print(f"Translation endpoint: {url}/translate")
            print(f"Lip-sync endpoint: {url}/lipsync")
            print(f"\nAdd this to your local .env:")
            print(f"FULL_PIPELINE_URL={url}")
            print(f"\nPress Ctrl+C to stop the tunnel")
            try:
                process.wait()
            except KeyboardInterrupt:
                print("\nStopping tunnel...")
                stop_tunnel()
                print("✅ Tunnel stopped")
        else:
            print("Could not determine tunnel URL")
            if process:
                print("Tunnel process is still running - check stderr for URL")
                print("Press Ctrl+C to stop")
                try:
                    process.wait()
                except KeyboardInterrupt:
                    stop_tunnel()