#!/usr/bin/env python3
"""Stage built release artifacts under /tmp and validate them; installs no service."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='wallfolio-install-') as directory:
    prefix = Path(directory)/'usr'
    subprocess.run(['make','install',f'PREFIX={prefix}'],cwd=root,check=True,stdout=subprocess.DEVNULL)
    for binary in ['wallfolio','wallfoliod','wallfolio-gui']:
        assert (prefix/'bin'/binary).is_file()
    for shellfile in ['bash-completion/completions/wallfolio','zsh/site-functions/_wallfolio','fish/vendor_completions.d/wallfolio.fish']:
        assert (prefix/'share'/shellfile).stat().st_size > 100
    subprocess.run(['bash','-n',str(prefix/'share/bash-completion/completions/wallfolio')],check=True)
    subprocess.run(['zsh','-n',str(prefix/'share/zsh/site-functions/_wallfolio')],check=True)
    subprocess.run(['desktop-file-validate',str(prefix/'share/applications/io.wallfolio.Wallfolio.desktop')],check=True)
    subprocess.run(['systemd-analyze','--user','verify',str(prefix/'lib/systemd/user/wallfoliod.service')],check=True)
    print('PASS: staged install, shell completions, desktop entry, and user service')
