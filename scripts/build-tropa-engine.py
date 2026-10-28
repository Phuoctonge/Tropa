"""Build or verify the owned Go worker offline; never launch the output."""

import argparse
import json
from pathlib import Path

from tropa_engine import build_engine, verify_engine_build


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['build', 'verify'])
    parser.add_argument('--name', required=True)
    parser.add_argument('--go', type=Path, default=root / '.tools/go1.26.3/go/bin/go.exe')
    args = parser.parse_args()
    if not args.name or any(character not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for character in args.name):
        parser.error('Name must contain ASCII letters, digits, hyphens or underscores')
    output = root / '.artifacts/engine-build' / args.name
    action = build_engine if args.operation == 'build' else verify_engine_build
    result = action(root, output, args.go)
    print(json.dumps({'output': str(output), 'buildId': result['buildId'], 'binary': result['binary']}))


if __name__ == '__main__':
    main()
