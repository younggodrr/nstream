"""Run locally or in Colab to check an authenticated NStream GPU worker."""
from __future__ import annotations

import os
import sys
import requests

url = os.environ.get("LIPSYNC_API_URL", "").rstrip("/")
token = os.environ.get("LIPSYNC_API_KEY") or os.environ.get("LIPSYNC_SERVICE_TOKEN")
if not url or not token:
    sys.exit("Set LIPSYNC_API_URL and LIPSYNC_API_KEY (or LIPSYNC_SERVICE_TOKEN).")
base = url.removesuffix("/v1/lipsync")
response = requests.get(base + "/health", timeout=20)
response.raise_for_status()
print(response.json())
response = requests.get(base + "/v1/capabilities", headers={"Authorization": f"Bearer {token}"}, timeout=20)
response.raise_for_status()
print(response.json())
