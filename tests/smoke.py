#!/usr/bin/env python3
"""Isolated CLI/daemon lifecycle test. Never touches the user's catalog or desktop."""
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]

def png():
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', 1, 1, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\0\x50\x80\xa0')) + chunk(b'IEND', b'')

with tempfile.TemporaryDirectory(prefix='wallfolio-test-') as tmp:
    tmp = Path(tmp)
    source = tmp / 'mountain.png'
    source.write_bytes(png())
    duplicate = tmp / 'another.png'
    duplicate.write_bytes(png())
    sock = tmp / 'runtime' / 'daemon.sock'
    tools = tmp / 'bin'
    tools.mkdir()
    setter = tools / 'swww'
    setter.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$WALLFOLIO_TEST_ARGS"\n')
    setter.chmod(0o755)
    (tools/"hyprctl").write_text(setter.read_text())
    (tools/"hyprctl").chmod(0o755)
    env = dict(os.environ, WALLFOLIO_SOCKET=str(sock), PATH=str(tools)+os.pathsep+os.environ['PATH'], WAYLAND_DISPLAY='test', WALLFOLIO_TEST_ARGS=str(tmp/'args'))
    daemon = subprocess.Popen([str(ROOT / 'target/debug/wallfoliod'), '--data-dir', 'data'], cwd=tmp, env=env, stderr=subprocess.PIPE)
    def cli(*args, ok=True):
        result = subprocess.run([str(ROOT/'target/debug/wallfolio'), *args], env=env, cwd=tmp, capture_output=True, text=True, timeout=10)
        if ok:
            assert result.returncode == 0, result.stderr
            return json.loads(result.stdout)
        assert result.returncode != 0, result.stdout
        return result.stderr
    def rpc(method, params=None, ok=True):
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(10)
            client.connect(str(sock))
            client.sendall(json.dumps({'version':1,'method':method,'params':params or {}}).encode()+b'\n')
            result = json.loads(client.makefile('rb').readline())
            assert result['ok'] == ok, result
            return result.get('result')
    def discovered(external_id):
        return next(item for item in cli('provider','search','local',str(tmp)) if item['external_id']==external_id)
    try:
        for _ in range(100):
            if sock.exists(): break
            if daemon.poll() is not None: raise RuntimeError(daemon.stderr.read().decode())
            time.sleep(.05)
        assert cli('search') == []
        assert len(cli('provider', 'search', 'local', str(tmp))) == 2
        a = cli('add', str(source))
        assert a['id'] == cli('add', 'mountain.png')['id']
        assert a['local_path'] is None
        assert discovered(a['external_id'])['id'] == a['id']
        assert discovered(a['external_id'])['local_path'] is None
        a = cli('download', a['id'])
        managed = Path(a['local_path'])
        assert managed.read_bytes() == source.read_bytes()
        assert (a['width'],a['height']) == (1,1)
        assert discovered(a['external_id'])['local_path'] == str(managed)
        b = cli('add', str(duplicate))
        b = cli('download', b['id'])
        assert b['local_path'] == str(managed)
        cli('set', a['id'], '--backend', 'swww', '--monitor', 'DP-1')
        assert (tmp/'args').read_text().splitlines() == ['img', str(managed), '--outputs', 'DP-1']
        cli('set', a['id'], '--backend', 'hyprpaper', '--monitor', 'DP-1')
        assert (tmp/'args').read_text().splitlines() == ['hyprpaper', 'wallpaper', 'DP-1,'+str(managed)]
        cli('set', a['id'], '--backend', 'hyprpaper', '--monitor', 'bad,name', ok=False)
        assert rpc('device.settings')['preferred_backend'] == 'hyprpaper'
        rpc('device.settings.update',{'preferred_backend':'missing'},ok=False)
        assert rpc('device.settings')['preferred_backend'] == 'hyprpaper'
        rpc('device.settings.update',{'preferred_backend':'swww'})
        cli('set',a['id'])
        assert (tmp/'args').read_text().splitlines() == ['img',str(managed)]
        rpc('device.settings.update',{'preferred_backend':'hyprpaper'})
        competing = subprocess.run([str(ROOT/'target/debug/wallfoliod'), '--data-dir', str(tmp/'data'), '--socket',str(tmp/'other.sock')],env=env,capture_output=True,timeout=5)
        assert competing.returncode != 0, 'two daemons must not own one catalog' 
        cli('favorite', a['id'])
        cli('tags', a['id'], 'landscape', 'dark')
        assert len(cli('search','DARK','--favorite')) == 1
        assert discovered(a['external_id'])['favorite']
        if os.environ.get('WALLFOLIO_TEST_GUI'):
            gui_env = dict(env, QT_QPA_PLATFORM='offscreen', QT_QUICK_BACKEND='software', WALLFOLIO_SCREENSHOT=str(ROOT/'build/library-preview.png'))
            rendered = subprocess.run([os.environ['WALLFOLIO_TEST_GUI']],env=gui_env,capture_output=True,text=True,timeout=10)
            assert rendered.returncode == 0, rendered.stderr
            assert not rendered.stderr, rendered.stderr
        cli('remove-local', a['id'])
        assert managed.exists(), 'a shared original must be retained'
        assert cli('get', a['id'])['favorite']
        assert discovered(a['external_id'])['local_path'] is None
        cli('remove-local', b['id'])
        assert not managed.exists()
        assert source.exists() and duplicate.exists(), 'user files must be preserved'
        cli('set', a['id'], ok=False)
        # Removing catalog metadata intentionally preserves the managed file.
        b = cli('download', b['id'])
        cli('remove', b['id'])
        assert Path(b['local_path']).exists()
        cli('get', b['id'], ok=False)
        assert 'id' not in discovered(b['external_id'])
        with socket.socket(socket.AF_UNIX) as client:
            client.connect(str(sock))
            client.sendall(b'{"version":999,"method":"catalog.search"}\n')
            response = json.loads(client.makefile('rb').readline())
            assert not response['ok']
        with socket.socket(socket.AF_UNIX) as client:
            client.connect(str(sock))
            client.sendall(b'not json\n')
            assert not json.loads(client.makefile('rb').readline())['ok']
        assert len(cli('search')) == 1, 'malformed clients must not kill daemon'
        daemon.terminate(); daemon.wait(timeout=5)
        daemon = subprocess.Popen([str(ROOT/'target/debug/wallfoliod'), '--data-dir',str(tmp/'data')],env=env,stderr=subprocess.PIPE)
        time.sleep(.3)
        assert cli('get',a['id'])['tags'] == ['dark','landscape']
        assert rpc('device.settings')['preferred_backend'] == 'hyprpaper'
        cli('download',a['id'])
        cli('set',a['id'])
        assert (tmp/'args').read_text().splitlines() == ['hyprpaper','wallpaper',','+str(managed)]
        assert discovered(a['external_id'])['id'] == a['id']
        print('PASS: discovery, stable identity, download, metadata, deduplication, favorites, tags, deletion, protocol errors, restart persistence')
    finally:
        daemon.terminate()
        daemon.wait(timeout=5)
