"""Capture selected working-tree source bytes; never copy user/runtime data."""

import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import zipfile


ROOT_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml",
    "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "tsconfig.json",
    "vite.config.mts", "vitest.config.mts", "eslint.config.ts", "biome.json",
    "LICENSE", "README.md", "THIRD_PARTY_NOTICES.md", ".gitignore", ".gitattributes",
}
SOURCE_ROOTS = ["src/", "crates/", "scripts/", "src-tauri/src/", "src-tauri/capabilities/",
                "src-tauri/assets/", "src-tauri/icons/", "src-tauri/images/", "src-tauri/packages/"]
EXTENSIONS = {".rs", ".toml", ".lock", ".json", ".yaml", ".yml", ".ts", ".tsx", ".mts",
              ".js", ".jsx", ".mjs", ".css", ".scss", ".html", ".svg", ".png", ".jpg",
              ".webp", ".ico", ".icns", ".car", ".ttf", ".woff", ".woff2", ".md", ".txt",
              ".xml", ".py", ".ps1", ".sh", ".c", ".h", ".nsi", ".nsh", ".cmd", ".bat"}


def include_path(name):
    path = PurePosixPath(name)
    if ("/".join(path.parts) != name or "\\" in name or ":" in name
            or any(part in {"..", ".", "secrets", "node_modules", "__pycache__", ".git"} for part in path.parts)
            or any(part.startswith(".env") for part in path.parts)):
        return False
    if name in ROOT_FILES:
        return True
    if path.parts and path.parts[0] == 'engine':
        return (name in {'engine/go.mod', 'engine/go.sum', 'engine/dependencies.json',
                         'engine/source-origins.json', 'engine/THIRD_PARTY_NOTICES.md'}
                or (len(path.parts) > 2 and path.parts[1] in {'cmd', 'internal'} and path.suffix == '.go')
                or (len(path.parts) > 2 and path.parts[1] == 'licenses'
                    and (path.suffix in {'.md', '.txt'} or path.name in {'LICENSE', 'NOTICE'})))
    if path.parts and path.parts[0] == 'protocol':
        return path.suffix == '.json'
    if len(path.parts) == 2 and path.parts[0] == "src-tauri" and path.suffix in {".json", ".toml", ".rs", ".xml"}:
        return True
    return any(name.startswith(root) for root in SOURCE_ROOTS) and (
        path.suffix in EXTENSIONS or path.name in {"LICENSE", "NOTICE"})


def selected_paths(root):
    result = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
                            cwd=root, check=True, stdout=subprocess.PIPE)
    names = result.stdout.decode("utf-8").split("\0")
    return sorted({name for name in names if include_path(name) and (root / name).is_file()})


def _inputs(root, names):
    root = root.resolve()
    result = {}
    for name in sorted(set(names)):
        if not include_path(name):
            raise ValueError("Source path is outside the snapshot allowlist")
        path = root / name
        if not path.resolve(strict=True).is_relative_to(root) or path.is_symlink():
            raise ValueError("Source file resolves outside the workspace or is a symlink")
        result[name] = path
    return result


def _snapshot_id(files):
    encoded = json.dumps(files, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def create_snapshot(root, names, output):
    inputs = _inputs(root, names)
    if not inputs:
        raise ValueError("No source inputs selected")
    manifest = {"schemaVersion": 1, "scope": "selected-working-tree-inputs", "files": {}}
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, path in inputs.items():
            content = path.read_bytes()
            manifest["files"][name] = hashlib.sha256(content).hexdigest()
            archive.writestr("source/" + name, content)
        manifest["snapshotId"] = _snapshot_id(manifest["files"])
        archive.writestr("source-manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2))
    verify_snapshot(root, list(inputs), output)
    return {"snapshotId": manifest["snapshotId"], "files": len(inputs), "archive": str(output)}


def verify_snapshot(root, names, archive_path):
    inputs = _inputs(root, names)
    with zipfile.ZipFile(archive_path) as archive:
        manifest = json.loads(archive.read("source-manifest.json"))
        files = manifest.get("files", {})
        if set(inputs) != set(files):
            raise ValueError("Source input set changed since snapshot")
        expected = {"source/" + name for name in files} | {"source-manifest.json"}
        if len(archive.namelist()) != len(expected) or set(archive.namelist()) != expected:
            raise ValueError("Unexpected source snapshot contents")
        if manifest.get("schemaVersion") != 1 or manifest.get("snapshotId") != _snapshot_id(files):
            raise ValueError("Invalid source snapshot manifest")
        for name, path in inputs.items():
            saved = hashlib.sha256(archive.read("source/" + name)).hexdigest()
            with path.open("rb") as source:
                current = hashlib.file_digest(source, "sha256").hexdigest()
            if saved != files[name] or current != files[name]:
                raise ValueError(f"Source input changed since snapshot: {name}")
    return {"snapshotId": manifest["snapshotId"], "files": len(inputs)}
