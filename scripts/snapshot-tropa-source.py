"""Create/verify a source snapshot, without committing or publishing anything."""

import argparse
import json
from pathlib import Path

from tropa_source import create_snapshot, selected_paths, verify_snapshot


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["create", "verify"])
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    names = selected_paths(root)
    result = (create_snapshot(root, names, args.archive) if args.operation == "create"
              else verify_snapshot(root, names, args.archive))
    print(json.dumps(result, ensure_ascii=False, indent=2))
