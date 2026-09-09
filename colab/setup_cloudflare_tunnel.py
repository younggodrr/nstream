"""
Set up Cloudflare tunnel (no account required) for Colab GPU worker
"""

import subprocess
import threading
import time
import requests
import os
from pathlib import Path

def install_cloudflared():
    """Install cloudflared binary"""
    print("Installing cloudflared...")
    
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
    for _ in range(10):
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
                if "https://" in line:
                    # Extract URL from line like "https://xxxxx.trycloudflare.com"
                    import re
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

def main():
    """Main setup function"""
    try:
        # Install cloudflared
        install_cloudflared()
        
        # Start tunnel
        url, process = start_cloudflare_tunnel()
        
        if url:
            print(f"\n🎉 Setup complete!")
            print(f"Your GPU worker is accessible at: {url}")
            print(f"Health check: {url}/health")
            print(f"Transcription endpoint: {url}/transcribe")
            print(f"\nAdd this to your local .env:")
            print(f"REMOTE_INFERENCE_URL={url}")
            
            print(f"\nPress Ctrl+C to stop the tunnel")
            try:
                process.wait()
            except KeyboardInterrupt:
                print("\nStopping tunnel...")
                process.terminate()
                process.wait()
                print("✅ Tunnel stopped")
        else:
            print("Could not determine tunnel URL")
            if process:
                print("Tunnel process is still running - check stderr for URL")
                print("Press Ctrl+C to stop")
                try:
                    process.wait()
                except KeyboardInterrupt:
                    process.terminate()
    
    except Exception as e:
        print(f"❌ Error: {e}")
        return 1
    
    return 0

if __name__ == "__main__":
    import sys
    sys.exit(main())