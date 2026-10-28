"""Bind a controlled local compilation to selected source and frontend bytes."""

import hashlib
import json

from tropa_pe import inspect_pe
from tropa_source import create_snapshot, selected_paths, verify_snapshot


FEATURES = ['custom-protocol', 'default', 'portable']


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def frontend_inputs(root):
    directory = root / 'dist'
    if directory.is_symlink() or not (directory / 'index.html').is_file():
        raise ValueError('Frontend dist/index.html is missing or unsafe')
    files = {}
    for path in sorted(directory.rglob('*')):
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError('Frontend input resolves outside workspace or is a symlink')
        if path.is_file():
            files[path.relative_to(directory).as_posix()] = digest(path)
    return files


def validate_artifact(artifact, binary):
    if sorted(artifact.get('features', [])) != FEATURES:
        raise ValueError('Unexpected Cargo features for portable release')
    profile = artifact.get('profile', {})
    if profile.get('opt_level') != '3' or profile.get('test') is not False:
        raise ValueError('Unexpected Cargo release profile')
    if artifact.get('executable') != str(binary):
        raise ValueError('Cargo artifact executable does not match output')


def capture_build(root, binary, snapshot, record, compile_action, *, engine_pin=None):
    if record.exists() or snapshot.exists():
        raise FileExistsError('Build evidence already exists; choose new output paths')
    source = create_snapshot(root, selected_paths(root), snapshot)
    frontend = frontend_inputs(root)
    artifact = compile_action()
    validate_artifact(artifact, binary)
    verify_snapshot(root, selected_paths(root), snapshot)
    if frontend_inputs(root) != frontend:
        raise ValueError('Frontend inputs changed during compilation')
    pe = inspect_pe(binary.read_bytes())
    summary = {
        'schemaVersion': 1, 'scope': 'selected-source-and-frontend-local-build',
        'publicReleaseReady': False,
        'source': {'snapshotId': source['snapshotId'], 'archiveSha256': digest(snapshot)},
        'frontend': frontend,
        'binary': {**pe, 'sha256': digest(binary), 'executed': False},
        'cargo': {'features': FEATURES, 'profile': artifact['profile']},
    }
    if engine_pin is not None:
        summary['engine'] = engine_pin
    with record.open('x', encoding='utf-8') as output:
        json.dump(summary, output, indent=2)
    return summary


def verify_build(root, binary, snapshot, record):
    summary = json.loads(record.read_text(encoding='utf-8'))
    if (summary.get('schemaVersion') != 1
            or summary.get('scope') != 'selected-source-and-frontend-local-build'
            or summary.get('publicReleaseReady') is not False):
        raise ValueError('Invalid local build record')
    source = verify_snapshot(root, selected_paths(root), snapshot)
    if summary['source'] != {'snapshotId': source['snapshotId'], 'archiveSha256': digest(snapshot)}:
        raise ValueError('Source snapshot does not match build record')
    if summary['binary'].get('sha256') != digest(binary):
        raise ValueError('Binary differs from recorded compilation')
    if summary.get('frontend') != frontend_inputs(root):
        raise ValueError('Frontend inputs differ from recorded compilation')
    validate_artifact({**summary['cargo'], 'executable': str(binary)}, binary)
    pe = inspect_pe(binary.read_bytes())
    if summary['binary'] != {**pe, 'sha256': digest(binary), 'executed': False}:
        raise ValueError('Binary PE evidence does not match build record')
    return summary
