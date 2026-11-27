#!/usr/bin/env bash
set -euo pipefail

EXPECTED='https://github.com/MilMit/modemdeck-linux.git'
ACTUAL="$(git remote get-url origin 2>/dev/null || true)"
[[ "$ACTUAL" == "$EXPECTED" || "$ACTUAL" == "git@github.com:MilMit/modemdeck-linux.git" ]] || {
  echo "Expected the existing MilMit/modemdeck-linux origin; got '${ACTUAL:-no origin}'. No repository was created or changed." >&2
  exit 1
}
command -v gh >/dev/null 2>&1 || { echo "GitHub CLI is unavailable; remote settings were not inspected." >&2; exit 1; }

gh repo view MilMit/modemdeck-linux --json name,url,description,homepageUrl,defaultBranchRef,hasIssuesEnabled,hasDiscussionsEnabled,licenseInfo
echo "Read-only inspection complete. Repository settings require a separate approved action."
