import hashlib
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch

from tropa_bundle import CONTENT_FILES, INVENTORY, INVENTORY_BYTES, verify_archive
from tropa_build import capture_build
from tropa_engine import runtime_pin
from test_tropa_pe import fixture as pe_fixture


class BundleVerificationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "package.zip"
        self.contents = {name: b"synthetic artifact" for name in CONTENT_FILES}
        self.contents["component-inventory.json"] = INVENTORY_BYTES
        for notice in INVENTORY["notices"]:
            self.contents[notice["file"]] = (Path(__file__).parent.parent / notice["sourceFile"]).read_bytes()
        self.pins = {
            "Tropa.exe": hashlib.sha256(self.contents["Tropa.exe"]).hexdigest(),
            "resources/verge-mihomo.exe": hashlib.sha256(self.contents["resources/verge-mihomo.exe"]).hexdigest(),
            "resources/tropa-probe.exe": hashlib.sha256(self.contents["resources/tropa-probe.exe"]).hexdigest(),
        }

    def write_archive(self, extra=None):
        manifest = {name: hashlib.sha256(content).hexdigest() for name, content in self.contents.items()}
        with zipfile.ZipFile(self.path, "w") as archive:
            for name, content in self.contents.items():
                archive.writestr("Tropa-test/" + name, content)
            archive.writestr("Tropa-test/build-info.json", json.dumps({"build": "development-test", "files": manifest}))
            if extra:
                archive.writestr(extra, b"private synthetic registration")

    def test_rust_notices_are_required_and_pinned(self):
        name = "licenses/Rust-release-candidates.txt"
        self.assertIn(name, CONTENT_FILES)
        original = self.contents.pop(name)
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "contents"):
            verify_archive(self.path, pins=self.pins)
        self.contents[name] = original + b"modified notice"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_frontend_notices_are_required_and_pinned(self):
        name = "licenses/Frontend-candidates.txt"
        self.assertIn(name, CONTENT_FILES)
        original = self.contents.pop(name)
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "contents"):
            verify_archive(self.path, pins=self.pins)
        self.contents[name] = original + b"modified notice"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_font_license_texts_are_required_even_with_a_consistent_manifest(self):
        required = {"licenses/Twemoji-Mozilla.txt", "licenses/Apache-2.0.txt", "licenses/CC-BY-4.0.txt"}
        self.assertTrue(required.issubset(CONTENT_FILES), "The bundled emoji font needs its own notices")
        self.contents.pop("licenses/Twemoji-Mozilla.txt")
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "contents"):
            verify_archive(self.path, pins=self.pins)

    def test_recomputed_manifest_cannot_replace_a_pinned_font_notice(self):
        self.contents["licenses/Twemoji-Mozilla.txt"] = b"attribution removed"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_embedded_driver_and_go_notices_cannot_be_omitted(self):
        for name in ["licenses/Wintun-0.14.1.txt", "licenses/Go-modules.txt"]:
            with self.subTest(name=name):
                saved = self.contents.pop(name, None)
                self.write_archive()
                with self.assertRaisesRegex(ValueError, "contents"):
                    verify_archive(self.path, pins=self.pins)
                if saved is not None:
                    self.contents[name] = saved

    def test_rejects_user_data_even_when_manifest_looks_valid(self):
        self.write_archive("Tropa-test/data/secrets/warp-account.json")
        with self.assertRaisesRegex(ValueError, "contents"):
            verify_archive(self.path, pins=self.pins)

    def test_rejects_replaced_core_even_when_manifest_was_recomputed(self):
        self.contents["resources/verge-mihomo.exe"] = b"replaced core"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_rejects_replaced_worker_even_when_manifest_was_recomputed(self):
        self.contents["resources/tropa-probe.exe"] = b"replaced worker"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_requires_trusted_app_evidence(self):
        self.pins.pop("Tropa.exe")
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Application executable must be pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_rejects_replaced_app_even_when_manifest_was_recomputed(self):
        self.contents["Tropa.exe"] = b"replaced application"
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "Pinned"):
            verify_archive(self.path, pins=self.pins)

    def test_rejects_legacy_scanner_even_with_consistent_manifest(self):
        self.contents["resources/warpscout.exe"] = b"legacy scanner"
        self.pins["resources/warpscout.exe"] = hashlib.sha256(b"legacy scanner").hexdigest()
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "contents"):
            verify_archive(self.path, pins=self.pins)

    def test_requires_notices_and_rejects_traversal(self):
        del self.contents["THIRD_PARTY_NOTICES.md"]
        self.write_archive()
        with self.assertRaisesRegex(ValueError, "contents"):
            verify_archive(self.path, pins=self.pins)
        self.contents["THIRD_PARTY_NOTICES.md"] = b"notices"
        self.write_archive("Tropa-test/../escape.txt")
        with self.assertRaises(ValueError):
            verify_archive(self.path, pins=self.pins)

    def test_rejects_duplicate_entries(self):
        self.write_archive()
        with zipfile.ZipFile(self.path, "a") as archive:
            archive.writestr("Tropa-test/README.TXT", b"shadow readme")
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            verify_archive(self.path, pins=self.pins)

    def test_accepts_clean_archive_without_running_executables(self):
        self.write_archive()
        result = verify_archive(self.path, pins=self.pins)
        self.assertEqual(result["files"], len(CONTENT_FILES) + 1)

    def test_packager_requires_app_build_evidence(self):
        source = Path(__file__).resolve().parent.parent
        spec = importlib.util.spec_from_file_location("tropa_packager", source / "scripts/package-tropa.py")
        packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(packager)
        with patch.object(sys, "argv", ["package-tropa.py", "--engine-build", "unused"]), \
             contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                packager.main()
        self.assertEqual(error.exception.code, 2)

    def test_packager_omits_existing_registration_and_survives_independent_verification(self):
        source = Path(__file__).resolve().parent.parent
        root = Path(self.directory.name) / "fixture"
        resources = root / "src-tauri/resources"
        (resources / "licenses").mkdir(parents=True)
        for name in ["verge-mihomo.exe", "licenses/WarpScout.txt"]:
            shutil.copyfile(source / "src-tauri/resources" / name, resources / name)
        for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"]:
            shutil.copyfile(source / name, root / name)
        shutil.copytree(source / "scripts/licenses", root / "scripts/licenses")
        secrets = root / "data/secrets"
        secrets.mkdir(parents=True)
        (secrets / "warp-account.json").write_text("synthetic private registration", encoding="utf-8")
        binary = root / "synthetic.exe"
        binary.write_bytes(pe_fixture())
        engine_dir = root / "engine-build"
        engine_dir.mkdir(parents=True)
        (engine_dir / "tropa-probe.exe").write_bytes(b"worker binary fixture")
        worker_hash = hashlib.sha256(b"worker binary fixture").hexdigest()
        fake_engine = {
            "protocolVersion": 1,
            "buildId": "source:test",
            "source": {"snapshotId": "test"},
            "binary": {"sha256": worker_hash},
        }
        subprocess.run(["git", "init", "--quiet", str(root)], check=True)
        (root / "dist").mkdir()
        (root / "dist/index.html").write_bytes(b"frontend fixture")
        record_file = root / "build.json"
        snapshot_file = root / "source.zip"
        capture_build(root, binary, snapshot_file, record_file, lambda: {
            "features": ["custom-protocol", "default", "portable"],
            "profile": {"opt_level": "3", "test": False}, "executable": str(binary),
        }, engine_pin=runtime_pin(fake_engine))
        spec = importlib.util.spec_from_file_location("tropa_packager", source / "scripts/package-tropa.py")
        packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(packager)
        packager.ROOT = root
        with patch.object(packager, "verify_engine_build", return_value=fake_engine), \
             patch("tropa_engine.verify_engine_build", return_value=fake_engine):
            with patch.object(sys, "argv", [
                "package-tropa.py", "--binary", str(binary), "--name", "fixture-test",
                "--engine-build", str(engine_dir),
                "--build-record", str(record_file), "--source-snapshot", str(snapshot_file),
            ]):
                with contextlib.redirect_stdout(io.StringIO()):
                    packager.main()
            archive = root / ".artifacts/portable/fixture-test.zip"
            result = verify_archive(archive, engine_build=engine_dir, root=root,
                                    build_record=record_file, source_snapshot=snapshot_file)
            with zipfile.ZipFile(archive) as package:
                replaced = {name: package.read(name) for name in package.namelist()}
                inventory = json.loads(package.read("fixture-test/component-inventory.json"))
            workers = [component for component in inventory["components"]
                       if component["file"] == "resources/tropa-probe.exe"]
            self.assertEqual(len(workers), 1)
            self.assertEqual(workers[0]["sha256"], worker_hash)
            self.assertEqual(workers[0]["buildId"], "source:test")
            self.assertEqual(workers[0]["sourceSnapshotId"], "test")
            self.assertEqual(workers[0]["protocolVersion"], 1)
            inventory_changed = dict(replaced)
            workers[0]["buildId"] = "source:substituted"
            workers[0]["sourceSnapshotId"] = "substituted"
            inventory_changed["fixture-test/component-inventory.json"] = json.dumps(inventory).encode()
            inventory_info = json.loads(inventory_changed["fixture-test/build-info.json"])
            inventory_info["files"]["component-inventory.json"] = hashlib.sha256(
                inventory_changed["fixture-test/component-inventory.json"]).hexdigest()
            inventory_changed["fixture-test/build-info.json"] = json.dumps(inventory_info).encode()
            inventory_tampered = root / "inventory-tampered.zip"
            with zipfile.ZipFile(inventory_tampered, "w") as package:
                for name, content in inventory_changed.items():
                    package.writestr(name, content)
            with self.assertRaisesRegex(ValueError, "inventory differs"):
                verify_archive(inventory_tampered, engine_build=engine_dir, root=root,
                               build_record=record_file, source_snapshot=snapshot_file)
            replaced["fixture-test/Tropa.exe"] = pe_fixture() + b"changed image"
            info = json.loads(replaced["fixture-test/build-info.json"])
            changed_hash = hashlib.sha256(replaced["fixture-test/Tropa.exe"]).hexdigest()
            info["files"]["Tropa.exe"] = changed_hash
            info["localBuild"]["binary"]["sha256"] = changed_hash
            replaced["fixture-test/build-info.json"] = json.dumps(info).encode()
            tampered = root / "tampered.zip"
            with zipfile.ZipFile(tampered, "w") as package:
                for name, content in replaced.items():
                    package.writestr(name, content)
            with self.assertRaisesRegex(ValueError, "Binary differs from recorded compilation"):
                verify_archive(tampered, engine_build=engine_dir, root=root,
                               build_record=record_file, source_snapshot=snapshot_file)
        self.assertFalse(result["publicReleaseReady"])
        with zipfile.ZipFile(archive) as package:
            self.assertNotIn("fixture-test/data/secrets/warp-account.json", package.namelist())
            info = json.loads(package.read("fixture-test/build-info.json"))
            self.assertEqual(set(info["files"]), CONTENT_FILES)

    def test_packager_rejects_mismatched_worker_pin(self):
        source = Path(__file__).resolve().parent.parent
        root = Path(self.directory.name) / "fixture-mismatch"
        resources = root / "src-tauri/resources"
        (resources / "licenses").mkdir(parents=True)
        for name in ["verge-mihomo.exe", "licenses/WarpScout.txt"]:
            shutil.copyfile(source / "src-tauri/resources" / name, resources / name)
        for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"]:
            shutil.copyfile(source / name, root / name)
        shutil.copytree(source / "scripts/licenses", root / "scripts/licenses")
        binary = root / "synthetic.exe"
        binary.write_bytes(b"not an executable; packaging fixture only")
        engine_dir = root / "engine-build"
        engine_dir.mkdir(parents=True)
        (engine_dir / "tropa-probe.exe").write_bytes(b"worker binary fixture")
        worker_hash = hashlib.sha256(b"worker binary fixture").hexdigest()
        fake_engine = {
            "protocolVersion": 1,
            "buildId": "source:test",
            "source": {"snapshotId": "test"},
            "binary": {"sha256": worker_hash},
        }
        record_file = root / "build.json"
        snapshot_file = root / "source.zip"
        snapshot_file.write_bytes(b"fake zip")
        record_file.write_text(json.dumps({
            "schemaVersion": 1, "scope": "selected-source-and-frontend-local-build",
            "publicReleaseReady": False,
        }), encoding="utf-8")
        fake_provenance = {
            "engine": {"sha256": "different_hash"},
            "binary": {"sha256": hashlib.sha256(b"not an executable; packaging fixture only").hexdigest()},
        }
        spec = importlib.util.spec_from_file_location("tropa_packager", source / "scripts/package-tropa.py")
        packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(packager)
        packager.ROOT = root
        with patch.object(packager, "verify_engine_build", return_value=fake_engine), \
             patch("tropa_engine.verify_engine_build", return_value=fake_engine), \
             patch.object(packager, "verify_build", return_value=fake_provenance):
            with patch.object(sys, "argv", [
                "package-tropa.py", "--binary", str(binary), "--name", "mismatch-test",
                "--engine-build", str(engine_dir),
                "--build-record", str(record_file), "--source-snapshot", str(snapshot_file),
            ]):
                with self.assertRaisesRegex(ValueError, "does not match recorded compilation"):
                    packager.main()


if __name__ == "__main__":
    unittest.main()
