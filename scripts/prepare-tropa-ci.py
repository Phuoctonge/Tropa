"""Verify the pinned CI dependency assets before preparing application resources."""

import hashlib
import json
from pathlib import Path
import shutil


ROOT = Path(__file__).resolve().parent.parent
inventory = json.loads((ROOT / 'scripts/tropa-components.json').read_text(encoding='utf-8'))
core = next(component for component in inventory['components'] if component['name'] == 'mihomo')
inputs = ROOT / '.artifacts/build-inputs'
binary = inputs / 'verge-mihomo-x86_64-pc-windows-msvc.exe'
source = inputs / 'mihomo-source.zip'
for path, expected in ((binary, core['sha256']), (source, core['sourceArchiveSha256'])):
    with path.open('rb') as stream:
        if hashlib.file_digest(stream, 'sha256').hexdigest() != expected:
            raise ValueError(f'Pinned build input checksum mismatch: {path.name}')
sidecar = ROOT / 'src-tauri/sidecar'
sidecar.mkdir(parents=True, exist_ok=True)
shutil.copyfile(binary, sidecar / binary.name)
print('Pinned mihomo binary and corresponding source verified.')
