"""Inspect portable ZIP contents without extracting or executing bundled files."""

import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import zipfile


INVENTORY_BYTES = Path(__file__).with_name("tropa-components.json").read_bytes()
INVENTORY = json.loads(INVENTORY_BYTES)
PINNED = {component["file"]: component["sha256"] for component in INVENTORY["components"]}
NOTICE_PINS = {notice["file"]: notice["sha256"] for notice in INVENTORY["notices"]}
CONTENT_FILES = frozenset({
    "Tropa.exe", "README.txt", "licenses/GPL-3.0.txt", "licenses/WarpScout.txt",
    "THIRD_PARTY_NOTICES.md", "component-inventory.json", *PINNED, *NOTICE_PINS,
})


def verify_archive(path, *, pins=None):
    pins = {**(PINNED if pins is None else pins), **NOTICE_PINS}
    with zipfile.ZipFile(path) as archive:
        members = archive.infolist()
        names = [member.filename for member in members]
        if len(names) != len(set(name.casefold() for name in names)):
            raise ValueError("Duplicate portable ZIP entries")
        roots = set()
        by_name = {}
        for member in members:
            name = member.filename
            parts = PurePosixPath(name).parts
            if (len(parts) < 2 or any(part in {".", ".."} for part in parts)
                    or "/".join(parts) != name or "\\" in name or ":" in name
                    or not re.fullmatch(r"[A-Za-z0-9_-]+", parts[0])
                    or member.is_dir() or (member.external_attr >> 16) & 0o170000 == 0o120000):
                raise ValueError("Unsafe portable ZIP entry")
            if member.file_size > 128 * 1024 * 1024:
                raise ValueError("Oversized portable ZIP entry")
            roots.add(parts[0])
            by_name["/".join(parts[1:])] = member
        if len(roots) != 1 or set(by_name) != CONTENT_FILES | {"build-info.json"}:
            raise ValueError("Unexpected portable ZIP contents")
        if by_name["build-info.json"].file_size > 1024 * 1024:
            raise ValueError("Oversized portable manifest")
        info = json.loads(archive.read(by_name["build-info.json"]))
        if info.get("build") != "development-test" or not isinstance(info.get("files"), dict):
            raise ValueError("Unsupported portable manifest")
        if set(info["files"]) != CONTENT_FILES:
            raise ValueError("Manifest does not cover portable contents")
        for name in CONTENT_FILES:
            with archive.open(by_name[name]) as source:
                actual = hashlib.file_digest(source, "sha256").hexdigest()
            if actual != info["files"][name]:
                raise ValueError(f"Manifest checksum mismatch: {name}")
            if name in pins and actual != pins[name]:
                raise ValueError(f"Pinned component mismatch: {name}")
        if archive.read(by_name["component-inventory.json"]) != INVENTORY_BYTES:
            raise ValueError("Component inventory differs from trusted packaging inventory")
    return {"files": len(members), "build": "development-test", "publicReleaseReady": False}
