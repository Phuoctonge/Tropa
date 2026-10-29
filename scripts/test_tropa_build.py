import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

from tropa_build import capture_build, verify_build


class BuildBindingTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        subprocess.run(['git', 'init', '--quiet', str(self.root)], check=True)
        (self.root / 'src').mkdir()
        (self.root / 'src/main.ts').write_bytes(b'source')
        (self.root / 'dist').mkdir()
        (self.root / 'dist/index.html').write_bytes(b'frontend')
        self.binary = self.root / 'output.exe'
        self.record = self.root / 'record.json'
        self.snapshot = self.root / 'source.zip'
        self.inspector = patch('tropa_build.inspect_pe', return_value={'executed': False})
        self.inspector.start()
        self.addCleanup(self.inspector.stop)

    def compile(self):
        self.binary.write_bytes(b'compiled image')
        return {'features': ['custom-protocol', 'default', 'portable'],
                'profile': {'opt_level': '3', 'test': False}, 'executable': str(self.binary)}

    def capture(self, action=None):
        return capture_build(self.root, self.binary, self.snapshot, self.record, action or self.compile)

    def test_binding_rejects_replaced_binary_and_changed_frontend(self):
        self.capture()
        verify_build(self.root, self.binary, self.snapshot, self.record)
        self.binary.write_bytes(b'old image')
        with self.assertRaisesRegex(ValueError, 'Binary'):
            verify_build(self.root, self.binary, self.snapshot, self.record)
        self.binary.write_bytes(b'compiled image')
        (self.root / 'dist/index.html').write_bytes(b'stale frontend')
        with self.assertRaisesRegex(ValueError, 'Frontend'):
            verify_build(self.root, self.binary, self.snapshot, self.record)

    def test_changes_during_compilation_never_create_success_record(self):
        def changed():
            artifact = self.compile()
            (self.root / 'src/main.ts').write_bytes(b'edited during compilation')
            return artifact
        with self.assertRaisesRegex(ValueError, 'changed'):
            self.capture(changed)
        self.assertFalse(self.record.exists())

    def test_new_source_after_build_invalidates_record(self):
        self.capture()
        (self.root / 'src/new.ts').write_bytes(b'new input')
        with self.assertRaisesRegex(ValueError, 'set changed'):
            verify_build(self.root, self.binary, self.snapshot, self.record)

    def test_frontend_changes_during_compilation_block_success_record(self):
        def changed():
            artifact = self.compile()
            (self.root / 'dist/index.html').write_bytes(b'rebuilt during compilation')
            return artifact
        with self.assertRaisesRegex(ValueError, 'Frontend inputs changed'):
            self.capture(changed)
        self.assertFalse(self.record.exists())

    def test_failed_or_probe_build_cannot_be_recorded(self):
        def failed():
            raise subprocess.CalledProcessError(1, ['cargo'])
        with self.assertRaises(subprocess.CalledProcessError):
            self.capture(failed)
        self.assertFalse(self.record.exists())
        self.snapshot.unlink()
        def probe():
            artifact = self.compile()
            artifact['features'].append('tropa-probe')
            return artifact
        with self.assertRaisesRegex(ValueError, 'features'):
            self.capture(probe)
        self.assertFalse(self.record.exists())

    def test_packaged_summary_has_no_local_paths(self):
        self.capture()
        summary = verify_build(self.root, self.binary, self.snapshot, self.record)
        self.assertNotIn(str(self.root), json.dumps(summary))
        self.assertFalse(summary['publicReleaseReady'])
        self.assertFalse(summary['binary']['executed'])

    def test_build_requires_engine_build_argument(self):
        source = Path(__file__).resolve().parent.parent
        spec = importlib.util.spec_from_file_location("tropa_builder", source / "scripts/build-tropa.py")
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        with patch.object(sys, "argv", ["build-tropa.py", "--name", "test-no-engine"]), \
             patch("sys.stderr", new_callable=io.StringIO):
            with self.assertRaises(SystemExit) as error:
                builder.main()
            self.assertEqual(error.exception.code, 2)

    def test_build_rejects_worker_evidence_changed_during_compilation(self):
        source = Path(__file__).resolve().parent.parent
        spec = importlib.util.spec_from_file_location("tropa_builder", source / "scripts/build-tropa.py")
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        builder.ROOT = self.root
        engine_dir = self.root / "fake-engine"
        engine_dir.mkdir(parents=True)
        (self.root / "src-tauri").mkdir(parents=True)
        (self.root / "src-tauri/tauri.tropa.conf.json").write_text("{}", encoding="utf-8")
        engine1 = {"protocolVersion": 1, "buildId": "b1", "source": {"snapshotId": "s1"}, "binary": {"sha256": "h1"}}
        engine2 = {"protocolVersion": 1, "buildId": "b2", "source": {"snapshotId": "s2"}, "binary": {"sha256": "h2"}}
        calls = [engine1, engine2]
        def fake_verify(*args, **kwargs):
            return calls.pop(0)

        artifact_msg = json.dumps({
            "reason": "compiler-artifact",
            "target": {"name": "clash-verge", "kind": ["bin"]},
            "executable": str(self.binary),
        })
        mock_proc = MagicMock()
        mock_proc.stdout = [artifact_msg + "\n"]
        mock_proc.wait.return_value = 0
        mock_proc.poll.return_value = 0

        real_popen = subprocess.Popen
        def selective_popen(*args, **kwargs):
            if args and args[0] == builder.COMMAND:
                return mock_proc
            return real_popen(*args, **kwargs)

        with patch.object(builder, "verify_engine_build", side_effect=fake_verify), \
             patch("subprocess.Popen", side_effect=selective_popen), \
             patch.object(sys, "argv", ["build-tropa.py", "--name", "test-changed-engine", "--engine-build", str(engine_dir)]):
            with self.assertRaisesRegex(ValueError, "Worker build evidence changed"):
                builder.main()


if __name__ == '__main__':
    unittest.main()
