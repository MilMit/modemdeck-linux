# Release process

The existing upstream repository is `MilMit/modemdeck-linux`. Do not create a replacement repository or publish a release from an unreviewed working tree.

## Local quality gates

Run `./scripts/validate-release.sh` from the repository root. It checks formatting, compilation, Clippy, tests, desktop metadata, AppStream metadata, and the release version/tag relationship. Package builders are separate and only count as successful when their output passes the relevant package validator.

## Review and release

1. Review `git status`, `git diff --check`, and every staged file. Keep local configuration, credentials, and unvalidated artifacts out of the commit.
2. Confirm the intended release version and check both local and remote tags/releases. The current release target is `v2.0.0`.
3. Build and validate packages for the actual target architectures. Do not rename artifacts to claim another architecture.
4. Generate SHA256 checksums after all artifacts are final.
5. Commit and push to the existing `origin` only after maintainer approval.
6. Create a GitHub **draft prerelease** for beta/RC versions, attach only validated artifacts, and review the rendered notes before publishing.
7. Update the hosted update manifest only after the release URL and checksums exist. The current website manifest URL returns 404, so the application falls back to GitHub Releases. Package-manager installs must be updated through their package manager.

The checked-in GitHub Actions workflow runs checks and uploads short-lived review artifacts. It does not push tags, create releases, or alter repository settings.

The helper scripts `scripts/publish-github.sh` and `scripts/release-github.sh` are legacy and must not be used for initial repository creation or unattended publication.
