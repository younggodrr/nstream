"""
Start the complete dubbing pipeline - both worker and tunnel in background
"""

import subprocess
import time
import sys
from pathlib import Path

def start_worker():
    """Start the dubbing worker"""
    print("🎬 Starting dubbing GPU worker...")
    worker_script = Path(__file__).parent / "run_dubbing_worker.py"
    
    process = subprocess.Popen(
        ["python", str(worker_script)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # Wait for worker to start
    print("⏳ Waiting for worker to start...")
    time.sleep(5)
    
    # Check if worker is still running
    if process.poll() is None:
        print("✅ Worker started successfully")
        return process
    else:
        print("❌ Worker failed to start")
        print(process.stderr.read())
        return None

def start_tunnel():
    """Start the cloudflare tunnel"""
    print("🌐 Starting Cloudflare tunnel...")
    tunnel_script = Path(__file__).parent / "run_tunnel_background.py"
    
    process = subprocess.Popen(
        ["python", str(tunnel_script)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # Wait a moment for tunnel to start
    time.sleep(3)
    
    # Check if tunnel is still running
    if process.poll() is None:
        print("✅ Tunnel started successfully")
        return process
    else:
        print("❌ Tunnel failed to start")
        print(process.stderr.read())
        return None

def main():
    """Main setup function"""
    print("🚀 Starting Complete Dubbing Pipeline")
    print("=" * 50)
    
    # Start worker
    worker_process = start_worker()
    if not worker_process:
        print("❌ Failed to start worker. Exiting.")
        return 1
    
    # Give worker time to fully initialize
    print("⏳ Giving worker time to initialize...")
    time.sleep(3)
    
    # Start tunnel
    tunnel_process = start_tunnel()
    if not tunnel_process:
        print("❌ Failed to start tunnel. Stopping worker.")
        worker_process.terminate()
        return 1
    
    print("\n🎉 Complete pipeline started!")
    print("=" * 50)
    print("Both worker and tunnel are running in background.")
    print("You can now execute other cells in your notebook.")
    print("\n📋 Next steps:")
    print("1. Wait for the tunnel to display your public URL")
    print("2. Copy the URL shown above")
    print("3. Add it to your local .env file:")
    print("   FULL_PIPELINE_URL=https://your-url.trycloudflare.com")
    print("4. Test the connection from your local machine")
    print("\n💡 To stop everything:")
    print("   !python run_dubbing_worker.py stop")
    print("   !python run_tunnel_background.py stop")
    
    # Keep the script running to show output
    try:
        while True:
            # Check if processes are still alive
            if worker_process.poll() is not None:
                print("⚠️ Worker process died")
                break
            if tunnel_process.poll() is not None:
                print("⚠️ Tunnel process died")
                break
            
            # Print any output from tunnel (it contains the URL)
            import select
            if select.select([tunnel_process.stderr], [], [], 0.1)[0]:
                line = tunnel_process.stderr.readline()
                if line:
                    print(line.strip())
            
            time.sleep(1)
            
    except KeyboardInterrupt:
        print("\n🛑 Stopping pipeline...")
        worker_process.terminate()
        tunnel_process.terminate()
        print("✅ Pipeline stopped")
    
    return 0

if __name__ == "__main__":
    sys.exit(main())