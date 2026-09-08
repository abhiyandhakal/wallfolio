#!/usr/bin/env python3
"""Opt-in live Wallhaven check; downloads one image into a temporary catalog."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='wallfolio-live-') as directory:
    base = Path(directory)
    socket = base/'runtime/daemon.sock'
    env = dict(os.environ,WALLFOLIO_SOCKET=str(socket))
    daemon = subprocess.Popen([str(root/'target/debug/wallfoliod'),'--data-dir',str(base/'data')],env=env,stderr=subprocess.PIPE)
    def cli(*args):
        result = subprocess.run([str(root/'target/debug/wallfolio'),*args],env=env,capture_output=True,text=True,timeout=180)
        assert result.returncode == 0,result.stderr
        return json.loads(result.stdout)
    try:
        for _ in range(100):
            if socket.exists(): break
            if daemon.poll() is not None: raise RuntimeError(daemon.stderr.read().decode())
            time.sleep(.05)
        candidates = cli('provider','search','wallhaven','mountains')
        assert candidates
        candidate = candidates[0]
        details = cli('provider','get','wallhaven',candidate['external_id'])
        assert details['external_id'] == candidate['external_id']
        saved = cli('add',details['external_id'],'--provider','wallhaven')
        assert saved['local_path'] is None
        cached = cli('download',saved['id'])
        assert Path(cached['local_path']).is_file()
        assert cached['width'] > 0 and cached['height'] > 0
        cli('remove-local',saved['id'])
        assert not Path(cached['local_path']).exists()
        assert cli('get',saved['id'])['local_path'] is None
        print(f"PASS: live Wallhaven search, details, cataloguing, download ({cached['width']}x{cached['height']}), and local deletion")
    finally:
        daemon.terminate()
        daemon.wait(timeout=5)
