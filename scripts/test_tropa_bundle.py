import hashlib
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch

from tropa_bundle import CONTENT_FILES, INVENTORY, INVENTORY_BYTES, verify_archive


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
            "resources/verge-mihomo.exe": hashlib.sha256(self.contents["resources/verge-mihomo.exe"]).hexdigest(),
            "resources/warpscout.exe": hashlib.sha256(self.contents["resources/warpscout.exe"]).hexdigest(),
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

    def test_packager_omits_existing_registration_and_survives_independent_verification(self):
        source = Path(__file__).resolve().parent.parent
        root = Path(self.directory.name) / "fixture"
        resources = root / "src-tauri/resources"
        (resources / "licenses").mkdir(parents=True)
        for name in ["verge-mihomo.exe", "warpscout.exe", "licenses/WarpScout.txt"]:
            shutil.copyfile(source / "src-tauri/resources" / name, resources / name)
        for name in ["LICENSE", "THIRD_PARTY_NOTICES.md"]:
            shutil.copyfile(source / name, root / name)
        shutil.copytree(source / "scripts/licenses", root / "scripts/licenses")
        secrets = root / "data/secrets"
        secrets.mkdir(parents=True)
        (secrets / "warp-account.json").write_text("synthetic private registration", encoding="utf-8")
        binary = root / "synthetic.exe"
        binary.write_bytes(b"not an executable; packaging fixture only")
        spec = importlib.util.spec_from_file_location("tropa_packager", source / "scripts/package-tropa.py")
        packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(packager)
        packager.ROOT = root
        with patch.object(sys, "argv", ["package-tropa.py", "--binary", str(binary), "--name", "fixture-test"]):
            with contextlib.redirect_stdout(io.StringIO()):
                packager.main()
        archive = root / ".artifacts/portable/fixture-test.zip"
        result = verify_archive(archive)
        self.assertFalse(result["publicReleaseReady"])
        with zipfile.ZipFile(archive) as package:
            self.assertNotIn("fixture-test/data/secrets/warp-account.json", package.namelist())
            info = json.loads(package.read("fixture-test/build-info.json"))
            self.assertEqual(set(info["files"]), CONTENT_FILES)


if __name__ == "__main__":
    unittest.main()
