#!/usr/bin/env python3
"""Verify dispatch and daemon lifetime using an extracted AppImage (no FUSE needed)."""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time

image = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='wallfolio-appimage-') as directory:
    root = Path(directory)
    sock = root/'runtime/wallfoliod.sock'
    env = dict(os.environ, APPIMAGE_EXTRACT_AND_RUN='1', WALLFOLIO_SOCKET=str(sock),
               XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'),
               QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software',
               WALLFOLIO_SCREENSHOT=str(root/'preview.png'))
    subprocess.run([str(image),'cli','--version'],env=env,check=True,timeout=60)
    daemon_pid = None
    try:
        gui = subprocess.run([str(image)],env=env,capture_output=True,text=True,timeout=60)
        assert gui.returncode == 0, gui.stderr
        assert (root/'preview.png').is_file()
        # The GUI has exited. Its independently launched daemon must still serve IPC.
        for attempt in range(100):
            try:
                with socket.socket(socket.AF_UNIX) as client:
                    client.settimeout(5)
                    client.connect(str(sock))
                    client.sendall(b'{"version":1,"method":"device.info","params":{}}\n')
                    info = json.loads(client.makefile('rb').readline())
                    assert info['ok'],info
                    daemon_pid = info['result']['pid']
                    break
            except (OSError,KeyError):
                if attempt == 99: raise
                time.sleep(.1)
        result = subprocess.run([str(image),'cli','search'],env=env,capture_output=True,text=True,timeout=60)
        assert result.returncode == 0,result.stderr
        assert json.loads(result.stdout) == []
        print('PASS: AppImage GUI, CLI, and daemon remaining alive after GUI exit')
    finally:
        if daemon_pid is None and sock.exists():
            try:
                with socket.socket(socket.AF_UNIX) as client:
                    client.settimeout(2)
                    client.connect(str(sock))
                    client.sendall(b'{"version":1,"method":"device.info","params":{}}\n')
                    daemon_pid=json.loads(client.makefile('rb').readline())['result']['pid']
            except (OSError,KeyError,ValueError): pass
        if daemon_pid:
            os.kill(daemon_pid,signal.SIGTERM)
