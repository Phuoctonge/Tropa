# Windows portable builds on GitHub Actions

The **Tropa Windows Portable** workflow builds a Windows x64 development package
on a fresh Windows runner. Start it from Actions after pushing the desired source
revision. Manual runs upload development artifacts without publishing a release.
Pushing a tag matching `v*-preview.*` runs the same checks and publishes a
GitHub prerelease only after successful packaging. Neither path enables a tunnel.

The workflow installs Rust 1.99.0, Go 1.26.3, Node 24.21.0 and pnpm 12.10.0. It
downloads locked dependencies before running the existing offline, source-bound
worker and application build scripts. Frontend checks, worker tests and package
verification must pass before the portable artifact is uploaded.

The repository must contain a prerelease named `tropa-build-inputs-v1` with these
assets:

- `verge-mihomo-x86_64-pc-windows-msvc.exe`: the pinned mihomo binary.
- `mihomo-source.zip`: its corresponding upstream source snapshot.

Both SHA-256 values must match `scripts/tropa-components.json`; the workflow
fails on a mismatch. These are dependency inputs, not an application release.
Replacing the core requires reviewing the source, notices and compatibility,
then updating its pin and dependency assets deliberately.

Download the successful run's `Tropa-windows-x64-<run number>` artifact. It contains
the portable ZIP, ZIP checksum, worker/application source snapshots and build
records, and the mihomo source snapshot. Extract `Tropa-windows-x64.zip` to run
the app. Keep the adjacent resources and licenses in place.

CI compilation does not establish real TUN routing, anti-cheat compatibility,
clean-machine acceptance or public release readiness. The package remains an
unsigned development build, with pending obligations recorded in its inventory.

## Preview publication

Push a unique preview tag, for example `v0.1.0-preview.2`, at the source commit
intended for distribution. The publication job needs a successful Windows build
and receives `contents: write`; the build job retains read-only permissions.
The job downloads that run's artifact, checks the ZIP checksum and attaches the
portable archive, a single `Tropa-sources.zip` containing the named app/worker
source snapshots, build records and mihomo source, and `SHA256SUMS.txt` to a prerelease. Existing release tags are not overwritten.

The first preview promotes successful run 40850455594 at commit
`9d013a048c6f6caf75fc33031791e21eeea5d533`. Subsequent README and publication
workflow changes do not change the provenance of that already-built executable.
