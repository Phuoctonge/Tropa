# Legacy upstream publishing workflows

These Clash Verge Rev release, scheduled autobuild and Telegram notification
workflows, development releases, updater publication, release asset cleanup
and autobuild decision helpers are retained outside `.github/workflows` so GitHub Actions does not
run them for Tropa. They refer to upstream publication channels and archived
release helpers; they are not a Tropa release pipeline.

Local Tropa build and package commands are documented in
[portable release](../../docs/validation/portable-release.md). A future publishing
workflow must use those source-bound app and worker checks and Tropa's own
destinations. Do not restore these files to the active workflow directory as a
way to publish a portable build.

The Tropa Windows Portable workflow is the active portable build. It runs on
manual dispatch and uploads development artifacts without creating an app release.
