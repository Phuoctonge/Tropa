import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from tropa_source import create_snapshot, include_path, verify_snapshot


class SourceSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "src").mkdir()
        (self.root / "src/new.ts").write_bytes(b"export const value = 2\r\n")
        self.archive = self.root / "snapshot.zip"

    def test_preserves_actual_working_files_and_detects_later_changes(self):
        result = create_snapshot(self.root, ["src/new.ts"], self.archive)
        with zipfile.ZipFile(self.archive) as archive:
            self.assertEqual(archive.read("source/src/new.ts"), b"export const value = 2\r\n")
            manifest = json.loads(archive.read("source-manifest.json"))
            self.assertEqual(manifest["snapshotId"], result["snapshotId"])
        verify_snapshot(self.root, ["src/new.ts"], self.archive)
        (self.root / "src/new.ts").write_text("changed after snapshot", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "changed"):
            verify_snapshot(self.root, ["src/new.ts"], self.archive)

    def test_does_not_admit_user_data_credentials_or_generated_binaries(self):
        for name in [".artifacts/account.json", "data/secrets/key.json", "src/.env",
                     "src-tauri/resources/warpscout.exe", "target/debug/clash-verge.exe"]:
            self.assertFalse(include_path(name), name)
        self.assertTrue(include_path("src-tauri/src/core/warp_connection.rs"))
        self.assertTrue(include_path("Cargo.lock"))
        with self.assertRaises(ValueError):
            create_snapshot(self.root, ["src/../data/secrets/account.json"], self.archive)
        self.assertFalse(self.archive.exists())

    def test_new_source_file_prevents_reusing_old_snapshot(self):
        create_snapshot(self.root, ["src/new.ts"], self.archive)
        (self.root / "src/another.ts").write_text("new input", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "set"):
            verify_snapshot(self.root, ["src/new.ts", "src/another.ts"], self.archive)

    def test_owned_worker_sources_and_contract_are_preserved_without_runtime_data(self):
        names = ["engine/go.mod", "engine/go.sum", "engine/internal/probe/session.go",
                 "engine/licenses/example/LICENSE", "protocol/search-v1.json"]
        for name in names:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'source input')
        create_snapshot(self.root, names, self.archive)
        with zipfile.ZipFile(self.archive) as archive:
            self.assertEqual(archive.read('source/engine/internal/probe/session.go'), b'source input')
        for name in ['engine/account.json', 'engine/secrets/key.json', 'engine/tropa-probe.exe']:
            self.assertFalse(include_path(name), name)


if __name__ == "__main__":
    unittest.main()
