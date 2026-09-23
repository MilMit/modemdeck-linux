#!/usr/bin/env bash
set -euo pipefail

command -v git >/dev/null || { echo "git is required" >&2; exit 1; }
VERSION="$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)"
[[ -n "$VERSION" ]] || { echo "Could not read version from Cargo.toml" >&2; exit 1; }
TAG="v${VERSION}"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Working tree is not clean. Commit changes before releasing." >&2
  exit 1
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Tag $TAG already exists locally." >&2
  exit 1
fi

git tag -a "$TAG" -m "ModemDeck Linux ${TAG}"
git push origin main
git push origin "$TAG"

echo "Release tag pushed: $TAG"
echo "GitHub Actions will build and publish the release assets automatically."
