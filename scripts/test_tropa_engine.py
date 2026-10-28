import json
from pathlib import Path
import tempfile
import unittest

from tropa_engine import build_engine, verify_engine_build


GO = Path(__file__).resolve().parent.parent / '.tools/go1.26.3/go/bin/go.exe'


@unittest.skipUnless(GO.is_file(), 'requires the pinned local Go toolchain')
class EngineBuildTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / 'engine/cmd/tropa-probe').mkdir(parents=True)
        (self.root / 'engine/go.mod').write_text('module tropa.local/probe\n\ngo 1.26.3\n')
        (self.root / 'engine/go.sum').write_text('')
        (self.root / 'engine/dependencies.json').write_text('{"modules": []}')
        self.source = self.root / 'engine/cmd/tropa-probe/main.go'
        self.source.write_text('package main\nimport "fmt"\nvar buildID="development"\nfunc main(){fmt.Println(buildID)}\n')
        self.output = self.root / 'build'

    def test_real_go_build_binds_sources_and_rejects_replaced_binary(self):
        result = build_engine(self.root, self.output, GO)
        verified = verify_engine_build(self.root, self.output, GO)
        self.assertEqual(result, verified)
        self.assertEqual(result['buildId'], 'source:' + result['source']['snapshotId'])
        self.assertFalse(result['publicReleaseReady'])
        self.assertNotIn(str(self.root), json.dumps(result))
        (self.output / 'tropa-probe.exe').write_bytes(b'replaced image')
        with self.assertRaisesRegex(ValueError, 'Binary'):
            verify_engine_build(self.root, self.output, GO)

    def test_new_go_source_is_detected_even_when_git_would_ignore_it(self):
        build_engine(self.root, self.output, GO)
        (self.root / 'engine/cmd/tropa-probe/ignored.go').write_text('package main\nvar injected=1\n')
        with self.assertRaisesRegex(ValueError, 'set changed'):
            verify_engine_build(self.root, self.output, GO)

    def test_identical_source_builds_have_identical_bytes(self):
        first = build_engine(self.root, self.output, GO)
        second = build_engine(self.root, self.root / 'repeat', GO)
        self.assertEqual(first['binary']['sha256'], second['binary']['sha256'])

    def test_embedded_input_cannot_escape_the_source_binding(self):
        self.source.write_text('package main\nimport ("fmt";_ "embed")\nvar buildID="development"\n//go:embed payload.txt\nvar payload string\nfunc main(){fmt.Println(buildID,payload)}\n')
        self.source.with_name('payload.txt').write_text('unrecorded embedded content')
        with self.assertRaisesRegex(ValueError, 'Unbound'):
            build_engine(self.root, self.output, GO)
        self.assertFalse((self.output / 'build.json').exists())

    def test_changed_build_arguments_invalidate_the_record(self):
        build_engine(self.root, self.output, GO)
        record = self.output / 'build.json'
        result = json.loads(record.read_text())
        result['go']['arguments'] = ['build']
        record.write_text(json.dumps(result))
        with self.assertRaisesRegex(ValueError, 'arguments'):
            verify_engine_build(self.root, self.output, GO)

    def test_failed_compilation_has_no_success_record(self):
        self.source.write_text('package main\ninvalid Go syntax\n')
        with self.assertRaises(Exception):
            build_engine(self.root, self.output, GO)
        self.assertFalse((self.output / 'build.json').exists())


if __name__ == '__main__':
    unittest.main()
