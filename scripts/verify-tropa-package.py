"""Verify a local Tropa development ZIP without executing its contents."""

import argparse
import json

from tropa_bundle import verify_archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive")
    args = parser.parse_args()
    print(json.dumps(verify_archive(args.archive)))
