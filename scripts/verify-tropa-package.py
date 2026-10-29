"""Verify a local Tropa development ZIP without executing its contents."""

import argparse
import json
from pathlib import Path

from tropa_bundle import verify_archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive")
    parser.add_argument("--engine-build", type=Path, required=True)
    parser.add_argument("--build-record", type=Path, required=True)
    parser.add_argument("--source-snapshot", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify_archive(args.archive, engine_build=args.engine_build,
                                   build_record=args.build_record, source_snapshot=args.source_snapshot)))
