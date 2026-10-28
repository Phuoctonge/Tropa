"""Statically inspect a Tropa EXE; no process or Windows loader is invoked."""

import argparse
import json
from pathlib import Path

from tropa_pe import inspect_pe


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    print(json.dumps(inspect_pe(args.binary.read_bytes()), indent=2))
