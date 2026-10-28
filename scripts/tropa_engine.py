"""Build and verify the owned Windows worker against exact local source inputs."""

import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess

from tropa_source import create_snapshot, include_path, verify_snapshot


ROOT = Path(__file__).resolve().parent.parent
GO_VERSION = 'go1.26.3'
PACKAGE = 'tropa.local/probe/cmd/tropa-probe'
BUILD_SCRIPTS = ('tropa_engine.py', 'build-tropa-engine.py', 'tropa_source.py')


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def engine_paths(root):
    names = set()
    for directory in ('engine', 'protocol'):
        for path in (root / directory).rglob('*'):
            if path.is_file() and include_path(path.relative_to(root).as_posix()):
                names.add(path.relative_to(root).as_posix())
    for script in BUILD_SCRIPTS:
        name = 'scripts/' + script
        if (root / name).is_file():
            names.add(name)
    return sorted(names)


def environment(go):
    result = os.environ.copy()
    result.update({
        'GOTOOLCHAIN': 'local', 'GOPROXY': 'off', 'GOSUMDB': 'off', 'GOWORK': 'off', 'GO111MODULE': 'on',
        'GOENV': 'off', 'GOFLAGS': '', 'GOEXPERIMENT': '', 'GOFIPS140': 'off',
        'GOOS': 'windows', 'GOARCH': 'amd64', 'GOAMD64': 'v1', 'CGO_ENABLED': '0',
        'GOROOT': str(go.parent.parent), 'GOCACHE': str(ROOT / '.tools/go-cache'),
        'GOPATH': str(ROOT / '.tools/go-path'),
        'GOMODCACHE': str(ROOT / '.tools/go-path/pkg/mod'),
    })
    return result


def run_go(go, arguments, cwd):
    return subprocess.run([str(go), *arguments], cwd=cwd, env=environment(go),
                          check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          text=True, encoding='utf-8').stdout


def toolchain(go, root):
    version = run_go(go, ['env', 'GOVERSION'], root).strip()
    if version != GO_VERSION:
        raise ValueError('Unexpected Go toolchain version')
    files = [go, go.parent.parent / 'VERSION']
    files.extend(go.parent.parent / 'pkg/tool/windows_amd64' / name
                 for name in ('compile.exe', 'link.exe', 'asm.exe'))
    return {'version': version, 'files': {path.relative_to(go.parent.parent).as_posix(): digest(path)
                                        for path in files}}


def inspect_binary(binary):
    data = binary.read_bytes()
    try:
        offset = struct.unpack_from('<I', data, 0x3c)[0]
        machine = struct.unpack_from('<H', data, offset + 4)[0]
        characteristics = struct.unpack_from('<H', data, offset + 22)[0]
        magic = struct.unpack_from('<H', data, offset + 24)[0]
        subsystem = struct.unpack_from('<H', data, offset + 24 + 68)[0]
    except struct.error as error:
        raise ValueError('Truncated worker PE image') from error
    if (data[:2] != b'MZ' or data[offset:offset + 4] != b'PE\0\0'
            or machine != 0x8664 or magic != 0x20b or subsystem != 3
            or not characteristics & 2 or characteristics & 0x2000):
        raise ValueError('Expected Windows x64 console worker executable')
    return {'sha256': digest(binary), 'size': len(data), 'architecture': 'x64',
            'subsystem': 'windowsConsole', 'executed': False}


def build_info(go, root, binary):
    info = json.loads(run_go(go, ['version', '-m', '-json', str(binary)], root))
    settings = {entry['Key']: entry['Value'] for entry in info.get('Settings', [])}
    expected = {'-buildmode': 'exe', '-compiler': 'gc', '-trimpath': 'true',
                'CGO_ENABLED': '0', 'GOOS': 'windows', 'GOARCH': 'amd64', 'GOAMD64': 'v1'}
    if (info.get('GoVersion') != GO_VERSION or info.get('Path') != PACKAGE
            or info.get('Main') != {'Path': 'tropa.local/probe', 'Version': '(devel)'}
            or any(settings.get(key) != value for key, value in expected.items())
            or any(key.startswith('vcs.') for key in settings)):
        raise ValueError('Worker Go build metadata does not match controlled compilation')
    return info


def audit_dependencies(root, info):
    inventory = json.loads((root / 'engine/dependencies.json').read_text(encoding='utf-8'))
    expected = []
    sums = set((root / 'engine/go.sum').read_text(encoding='utf-8').splitlines())
    for module in inventory['modules']:
        expected.append({key: module[key] for key in ('Path', 'Version', 'Sum')})
        if (f"{module['Path']} {module['Version']} {module['Sum']}" not in sums
                or f"{module['Path']} {module['Version']}/go.mod {module['GoModSum']}" not in sums):
            raise ValueError('Dependency inventory differs from locked module sums')
        for license in module['licenses']:
            path = root / 'engine' / license['path']
            if (not path.resolve(strict=True).is_relative_to((root / 'engine/licenses').resolve())
                    or path.is_symlink() or digest(path) != license['sha256']):
                raise ValueError('Dependency license differs from recorded source')
    actual = info.get('Deps', [])
    if sorted(actual, key=lambda module: module['Path']) != sorted(expected, key=lambda module: module['Path']):
        raise ValueError('Imported dependency graph differs from audited inventory')


def audit_owned_inputs(root, go, names):
    stream = run_go(go, ['list', '-mod=readonly', '-deps', '-json', './cmd/tropa-probe'], root / 'engine')
    decoder = json.JSONDecoder()
    names = set(names)
    while stream.strip():
        package, consumed = decoder.raw_decode(stream.lstrip())
        stream = stream.lstrip()[consumed:]
        if package.get('Module', {}).get('Path') != 'tropa.local/probe':
            continue
        for field in ('GoFiles', 'CgoFiles', 'CFiles', 'CXXFiles', 'MFiles', 'HFiles', 'FFiles',
                      'SFiles', 'SwigFiles', 'SwigCXXFiles', 'SysoFiles', 'EmbedFiles'):
            for name in package.get(field, []):
                path = (Path(package['Dir']) / name).resolve(strict=True)
                if not path.is_relative_to(root) or path.relative_to(root).as_posix() not in names:
                    raise ValueError('Unbound owned package input')


def build_arguments(build_id, binary):
    return ['build', '-trimpath', '-buildvcs=false', '-mod=readonly',
            '-ldflags', '-X main.buildID=' + build_id, '-o', str(binary), './cmd/tropa-probe']


def runtime_pin(summary):
    return {'protocolVersion': summary['protocolVersion'], 'buildId': summary['buildId'],
            'snapshotId': summary['source']['snapshotId'], 'sha256': summary['binary']['sha256']}


def build_engine(root, output, go):
    root, output, go = root.resolve(), output.resolve(), go.resolve(strict=True)
    if not output.is_relative_to(root):
        raise ValueError('Worker output must stay within the workspace')
    output.mkdir(parents=True, exist_ok=False)
    snapshot = output / 'source.zip'
    names = engine_paths(root)
    source = create_snapshot(root, names, snapshot)
    build_id = 'source:' + source['snapshotId']
    before_toolchain = toolchain(go, root)
    run_go(go, ['mod', 'verify'], root / 'engine')
    audit_owned_inputs(root, go, names)
    binary = output / 'tropa-probe.exe'
    flags = build_arguments(build_id, binary)
    run_go(go, flags, root / 'engine')
    verify_snapshot(root, engine_paths(root), snapshot)
    if toolchain(go, root) != before_toolchain:
        raise ValueError('Go toolchain changed during compilation')
    info = build_info(go, root, binary)
    audit_dependencies(root, info)
    summary = {
        'schemaVersion': 1, 'scope': 'owned-worker-source-bound-local-build',
        'publicReleaseReady': False, 'protocolVersion': 1, 'buildId': build_id,
        'source': {'snapshotId': source['snapshotId'], 'archiveSha256': digest(snapshot)},
        'binary': inspect_binary(binary),
        'go': {'toolchain': before_toolchain, 'buildInfo': info,
               'arguments': build_arguments(build_id, '<output>/tropa-probe.exe')},
    }
    with (output / 'build.json').open('x', encoding='utf-8') as record:
        json.dump(summary, record, indent=2)
        record.write('\n')
    return summary


def verify_engine_build(root, output, go):
    root, output, go = root.resolve(), output.resolve(), go.resolve(strict=True)
    summary = json.loads((output / 'build.json').read_text(encoding='utf-8'))
    if (summary.get('schemaVersion') != 1 or summary.get('scope') != 'owned-worker-source-bound-local-build'
            or summary.get('publicReleaseReady') is not False or summary.get('protocolVersion') != 1):
        raise ValueError('Invalid worker build record')
    source = verify_snapshot(root, engine_paths(root), output / 'source.zip')
    if (summary['source'] != {'snapshotId': source['snapshotId'], 'archiveSha256': digest(output / 'source.zip')}
            or summary.get('buildId') != 'source:' + source['snapshotId']):
        raise ValueError('Source snapshot differs from worker build record')
    binary = output / 'tropa-probe.exe'
    if summary['binary'].get('sha256') != digest(binary):
        raise ValueError('Binary differs from recorded worker compilation')
    if summary['binary'] != inspect_binary(binary):
        raise ValueError('Binary PE evidence differs from worker build record')
    if summary['go'].get('arguments') != build_arguments(summary['buildId'], '<output>/tropa-probe.exe'):
        raise ValueError('Worker build arguments differ from controlled compilation')
    info = build_info(go, root, binary)
    if summary['go']['toolchain'] != toolchain(go, root) or summary['go']['buildInfo'] != info:
        raise ValueError('Go build evidence differs from recorded worker compilation')
    audit_dependencies(root, info)
    return summary
