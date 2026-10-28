"""Compile portable Tropa offline and record inputs; never launch the output."""

import argparse
import json
import os
from pathlib import Path
import subprocess

from tropa_build import capture_build
from tropa_engine import runtime_pin, verify_engine_build


ROOT = Path(__file__).resolve().parent.parent
COMMAND = ['cargo', 'build', '--release', '--locked', '--offline', '-p', 'clash-verge',
           '--bin', 'clash-verge', '--features', 'portable', '--message-format=json']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', required=True)
    parser.add_argument('--engine-build', type=Path)
    args = parser.parse_args()
    if not args.name or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in args.name):
        parser.error('Name must contain ASCII letters, digits, hyphens or underscores')
    output = ROOT / '.artifacts/validation' / args.name
    output.mkdir(parents=True, exist_ok=False)
    binary = ROOT / 'target/release/clash-verge.exe'
    environment = os.environ.copy()
    environment.pop('TROPA_ENGINE_PIN', None)
    engine = (verify_engine_build(ROOT, args.engine_build, ROOT / '.tools/go1.26.3/go/bin/go.exe')
              if args.engine_build else None)
    if engine is not None:
        environment['TROPA_ENGINE_PIN'] = json.dumps(runtime_pin(engine), separators=(',', ':'))
    environment['TAURI_CONFIG'] = (ROOT / 'src-tauri/tauri.tropa.conf.json').read_text(encoding='utf-8')

    def compile_action():
        artifact = None
        with (output / 'cargo.jsonl').open('w', encoding='utf-8') as log:
            process = subprocess.Popen(COMMAND, cwd=ROOT, env=environment,
                                       stdout=subprocess.PIPE, text=True, encoding='utf-8')
            try:
                for line in process.stdout:
                    log.write(line)
                    log.flush()
                    message = json.loads(line)
                    if (message.get('reason') == 'compiler-artifact'
                            and message.get('target', {}).get('name') == 'clash-verge'
                            and 'bin' in message.get('target', {}).get('kind', [])
                            and message.get('executable')):
                        artifact = message
                code = process.wait()
                if code:
                    raise subprocess.CalledProcessError(code, COMMAND)
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait()
        if artifact is None:
            raise ValueError('Cargo did not report the requested executable')
        artifact['executable'] = str(Path(artifact['executable']).resolve())
        if engine is not None:
            if verify_engine_build(ROOT, args.engine_build, ROOT / '.tools/go1.26.3/go/bin/go.exe') != engine:
                raise ValueError('Worker build evidence changed during application compilation')
        return artifact

    summary = capture_build(ROOT, binary, output / 'source.zip', output / 'build.json', compile_action,
                            engine_pin=runtime_pin(engine) if engine else None)
    print(json.dumps({'record': str(output / 'build.json'), 'binary': summary['binary']}))


if __name__ == '__main__':
    main()
