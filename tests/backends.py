#!/usr/bin/env python3
"""Exercise real IPC and host-command adapters with isolated fake executables."""
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
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', 2, 1, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\0\x50\x80\xa0\x50\x80\xa0')) + chunk(b'IEND', b'')

with tempfile.TemporaryDirectory(prefix='wallfolio-backends-') as directory:
    tmp = Path(directory)
    tools = tmp / 'bin'
    tools.mkdir()
    log = tmp / 'commands.jsonl'
    mock = '''#!/usr/bin/python3
import json, os, sys, time
from pathlib import Path
name = Path(sys.argv[0]).name
with open(os.environ['TEST_LOG'], 'a') as f:
    f.write(json.dumps({'program':name,'pid':os.getpid(),'args':sys.argv[1:],'qt':os.environ.get('QT_PLUGIN_PATH'),'ld':os.environ.get('LD_LIBRARY_PATH')})+'\\n')
if name == 'swaybg':
    if Path(os.environ['TEST_LOG']).with_suffix('.fail').exists(): sys.exit(1)
    time.sleep(120)
if name == 'gsettings' and sys.argv[1] == 'list-keys':
    print('picture-uri\\npicture-uri-dark\\npicture-options')
if name == 'xfconf-query' and '-l' in sys.argv:
    print('/backdrop/screen0/monitorDP-1/workspace0/last-image\\n/backdrop/screen0/monitorDP-10/workspace0/last-image\\n/unrelated/last-image')
'''
    for program in ['swaybg', 'swww', 'hyprctl', 'feh', 'xwallpaper', 'nitrogen', 'gsettings', 'plasma-apply-wallpaperimage', 'xfconf-query']:
        path = tools / program
        path.write_text(mock)
        path.chmod(0o755)
    env = dict(os.environ, WALLFOLIO_SOCKET=str(tmp/'daemon.sock'), TEST_LOG=str(log),
               WALLFOLIO_HOST_PATH=str(tools), WALLFOLIO_HOST_QT_PLUGIN_PATH='', WALLFOLIO_HOST_LD_LIBRARY_PATH='',
               QT_PLUGIN_PATH='/fake-appimage-qt', WAYLAND_DISPLAY='test', XDG_CURRENT_DESKTOP='GNOME:KDE:XFCE')
    source = tmp / 'quoted image #1.png'
    source.write_bytes(png())
    daemon = None
    def start():
        global daemon
        daemon = subprocess.Popen([str(ROOT/'target/debug/wallfoliod'), '--data-dir', str(tmp/'data')], env=env, stderr=subprocess.PIPE)
        for _ in range(100):
            if daemon.poll() is not None:
                raise AssertionError(daemon.stderr.read().decode())
            try:
                rpc('device.info')
                return
            except (FileNotFoundError, ConnectionRefusedError):
                time.sleep(.05)
        raise AssertionError('daemon did not start')
    def rpc(method, params=None, ok=True):
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(20)
            client.connect(env['WALLFOLIO_SOCKET'])
            client.sendall(json.dumps({'version':1,'method':method,'params':params or {}}).encode()+b'\n')
            response = json.loads(client.makefile('rb').readline())
            assert response['ok'] == ok, response
            return response.get('result')
    def calls():
        return [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    def apply(backend, monitor=None):
        log.write_text('')
        rpc('wallpaper.apply', {'id':item['id'],'backend':backend,'monitor':monitor})
        result = calls()
        assert all(c['qt'] is None and c['ld'] is None for c in result), result
        return [(c['program'], c['args']) for c in result]
    try:
        start()
        item = rpc('catalog.add', {'provider':'local','external_id':str(source)})
        item = rpc('wallpaper.download', {'id':item['id']})
        file = item['local_path']
        assert apply('swww','DP-1') == [('swww',['img',file,'--outputs','DP-1'])]
        assert apply('hyprpaper','DP-1') == [('hyprctl',['hyprpaper','wallpaper','DP-1,'+file])]
        assert apply('feh') == [('feh',['--no-fehbg','--bg-fill',file])]
        assert apply('xwallpaper','DP-1') == [('xwallpaper',['--output','DP-1','--zoom',file])]
        assert apply('nitrogen','1') == [('nitrogen',['--set-zoom-fill','--head=1',file])]
        assert apply('nitrogen') == [('nitrogen',['--set-zoom-fill','--head=-1',file])]
        assert apply('kde') == [('plasma-apply-wallpaperimage',[file])]
        uri = Path(file).as_uri()
        assert apply('gnome') == [('gsettings',['list-keys','org.gnome.desktop.background'])] + [
            ('gsettings',['set','org.gnome.desktop.background',key,value]) for key,value in
            [('picture-uri',uri),('picture-uri-dark',uri),('picture-options','zoom')]]
        xfce = apply('xfce','DP-1')
        assert xfce == [('xfconf-query',['-c','xfce4-desktop','-l']),
                        ('xfconf-query',['-c','xfce4-desktop','-p','/backdrop/screen0/monitorDP-1/workspace0/last-image','-s',file])]
        for backend in ['gnome','kde','feh']:
            rpc('wallpaper.apply', {'id':item['id'],'backend':backend,'monitor':'DP-1'}, ok=False)
        rpc('wallpaper.apply', {'id':item['id'],'backend':'nitrogen','monitor':'DP-1'}, ok=False)
        rpc('wallpaper.apply', {'id':item['id'],'backend':'xfce','monitor':'missing'}, ok=False)
        apply('swaybg','DP-1')
        old_pid = calls()[0]['pid']
        assert apply('swaybg','DP-2') == [('swaybg',['-o','DP-1','-i',file,'-m','fill','-o','DP-2','-i',file,'-m','fill'])]
        new_pid = calls()[0]['pid']
        assert not Path(f'/proc/{old_pid}').exists(), 'replaced swaybg was not reaped'
        log.with_suffix('.fail').touch()
        rpc('wallpaper.apply', {'id':item['id'],'backend':'swaybg'}, ok=False)
        assert Path(f'/proc/{new_pid}').exists(), 'failed replacement killed active swaybg'
        log.with_suffix('.fail').unlink()
        apply('swww')
        assert not Path(f'/proc/{new_pid}').exists(), 'switching backend left swaybg running'
        apply('swaybg')
        orphan_pid = calls()[0]['pid']
        daemon.terminate()
        daemon.wait(timeout=5)
        for _ in range(50):
            stat = Path(f'/proc/{orphan_pid}/stat')
            if not stat.exists() or stat.read_text().split()[2] == 'Z': break
            time.sleep(.05)
        else: raise AssertionError('swaybg survived daemon termination')
        start()
        rpc('device.settings.update', {'preferred_backend':'swww'})
        rpc('rotation.configure', {'enabled':True,'interval_seconds':10})
        log.write_text('')
        # No requests during this wait: the daemon must wake its own schedule.
        time.sleep(11)
        assert len(calls()) == 1, calls()
        daemon.terminate()
        daemon.wait(timeout=5)
        start()
        assert rpc('rotation.status')['enabled']
        rpc('rotation.stop')
        assert not rpc('rotation.status')['enabled']
        assert rpc('wallpaper.random')['applied']
        # Local thumbnails are generated by the daemon worker and reused.
        result = rpc('catalog.search')
        assert result[0]['cached_thumbnail'] and Path(result[0]['cached_thumbnail']).is_file()
        assert rpc('cache.status')['bytes'] > 0
        assert rpc('cache.lookup', {'keys':[result[0]['thumbnail_key']]})[result[0]['thumbnail_key']] == result[0]['cached_thumbnail']
        assert rpc('cache.lookup', {'keys':['../../source.png']}) == {}
        print('PASS: host adapters, AppImage environment, idle rotation, restart, random, cache')
    finally:
        if daemon is not None and daemon.poll() is None:
            daemon.terminate()
            daemon.wait(timeout=5)
