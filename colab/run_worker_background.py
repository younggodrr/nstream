"""
Run the GPU worker in background - use this in Colab to avoid blocking cells
"""

import subprocess
import time
import threading
import requests
import os
from pathlib import Path

def start_worker():
    """Start the worker in background"""
    worker_script = Path(__file__).parent / "simple_gpu_worker.py"
    
    print(f"Starting GPU worker from {worker_script}...")
    
    # Start the process
    process = subprocess.Popen(
        ["python", str(worker_script)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # Wait for it to start
    print("Waiting for worker to start...")
    for i in range(10):
        time.sleep(1)
        try:
            response = requests.get("http://localhost:8000/health", timeout=2)
            if response.status_code == 200:
                print("✅ Worker is running and healthy!")
                print(f"Health check: {response.json()}")
                return process
        except:
            print(f"Attempt {i+1}/10: Worker not ready yet...")
    
    print("❌ Worker failed to start within 10 seconds")
    print("Stderr:", process.stderr.read())
    process.terminate()
    return None

def stop_worker(process):
    """Stop the background worker"""
    if process and process.poll() is None:
        print("Stopping worker...")
        process.terminate()
        process.wait()
        print("✅ Worker stopped")
    else:
        print("Worker was not running")

if __name__ == "__main__":
    # For direct execution
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "stop":
        # Find and kill existing worker
        import signal
        try:
            result = subprocess.run(["pkill", "-f", "simple_gpu_worker.py"])
            print("Killed existing worker processes")
        except:
            pass
    else:
        process = start_worker()
        if process:
            print(f"Worker running with PID: {process.pid}")
            print("Press Ctrl+C to stop (or run with 'stop' argument to kill)")
            try:
                process.wait()
            except KeyboardInterrupt:
                stop_worker(process)