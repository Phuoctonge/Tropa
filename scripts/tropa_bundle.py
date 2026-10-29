"""Inspect portable ZIP contents without extracting or executing bundled files."""

import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tempfile
import zipfile


INVENTORY_BYTES = Path(__file__).with_name("tropa-components.json").read_bytes()
INVENTORY = json.loads(INVENTORY_BYTES)
PINNED = {component["file"]: component["sha256"] for component in INVENTORY["components"]}
NOTICE_PINS = {notice["file"]: notice["sha256"] for notice in INVENTORY["notices"]}
CONTENT_FILES = frozenset({
    "Tropa.exe", "README.txt", "licenses/GPL-3.0.txt", "licenses/WarpScout.txt",
    "THIRD_PARTY_NOTICES.md", "component-inventory.json", "resources/tropa-probe.exe",
    *PINNED, *NOTICE_PINS,
})


def inventory_bytes(engine_pin=None):
    if engine_pin is None:
        return INVENTORY_BYTES
    inventory = json.loads(INVENTORY_BYTES)
    inventory["components"].append({
        "name": "Tropa probe worker",
        "file": "resources/tropa-probe.exe",
        "sha256": engine_pin["sha256"],
        "buildId": engine_pin["buildId"],
        "sourceSnapshotId": engine_pin["snapshotId"],
        "protocolVersion": engine_pin["protocolVersion"],
        "licenseFile": "licenses/GPL-3.0.txt",
        "noticeFiles": ["licenses/WarpScout.txt", "licenses/Go-modules.txt"],
    })
    return json.dumps(inventory, ensure_ascii=False, indent=2).encode("utf-8")


def verify_archive(path, *, pins=None, engine_build=None, build_record=None, source_snapshot=None, root=None):
    root = Path(__file__).resolve().parent.parent if root is None else Path(root)
    base_pins = PINNED if pins is None else pins
    pins = {**base_pins, **NOTICE_PINS}
    if bool(build_record) != bool(source_snapshot):
        raise ValueError("Build record and source snapshot must be supplied together")
    if build_record is None and "Tropa.exe" not in pins:
        raise ValueError("Application executable must be pinned via trusted app build evidence")
    engine_pin = None
    if engine_build is not None:
        from tropa_engine import runtime_pin, verify_engine_build
        go = root / ".tools/go1.26.3/go/bin/go.exe"
        engine = verify_engine_build(root, Path(engine_build), go)
        engine_pin = runtime_pin(engine)
        pins["resources/tropa-probe.exe"] = engine_pin["sha256"]
    if "resources/tropa-probe.exe" not in pins:
        raise ValueError("Worker executable must be pinned via trusted engine build")
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
        if archive.read(by_name["component-inventory.json"]) != inventory_bytes(engine_pin):
            raise ValueError("Component inventory differs from trusted packaging inventory")
        if build_record is not None:
            from tropa_build import verify_build
            with tempfile.TemporaryDirectory(prefix="tropa-package-verify-") as directory:
                binary = Path(directory) / "Tropa.exe"
                binary.write_bytes(archive.read(by_name["Tropa.exe"]))
                provenance = verify_build(root, binary, Path(source_snapshot), Path(build_record))
            if engine_pin is None or provenance.get("engine") != engine_pin:
                raise ValueError("Packaged worker does not match recorded compilation")
            if info.get("localBuild") != provenance:
                raise ValueError("Packaged app evidence differs from trusted build record")
    return {"files": len(members), "build": "development-test", "publicReleaseReady": False}
