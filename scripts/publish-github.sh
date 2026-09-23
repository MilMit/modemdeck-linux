#!/usr/bin/env bash
set -euo pipefail

REPO_NAME="${REPO_NAME:-modemdeck-linux}"
DESCRIPTION="ModemDeck Linux by MilMit — native GTK4 cellular modem manager for Ubuntu/Linux"
HOMEPAGE="https://milmit.net"

command -v git >/dev/null || { echo "git is required" >&2; exit 1; }
command -v gh >/dev/null || { echo "GitHub CLI (gh) is required. Install: sudo apt install gh" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run 'gh auth login' first." >&2; exit 1; }

if [[ ! -d .git ]]; then
  git init -b main
fi

git add .
if ! git diff --cached --quiet; then
  git commit -m "Initial ModemDeck Linux release candidate"
fi

git branch -M main

if ! git remote get-url origin >/dev/null 2>&1; then
  gh repo create "$REPO_NAME" \
    --public \
    --source=. \
    --remote=origin \
    --description "$DESCRIPTION" \
    --homepage "$HOMEPAGE" \
    --push
else
  git push -u origin main
fi

OWNER="$(gh api user --jq .login)"
gh api --method PUT "repos/${OWNER}/${REPO_NAME}/topics" \
  -H "Accept: application/vnd.github+json" \
  -f 'names[]=linux' \
  -f 'names[]=ubuntu' \
  -f 'names[]=modem' \
  -f 'names[]=lte' \
  -f 'names[]=5g' \
  -f 'names[]=modemmanager' \
  -f 'names[]=gtk4' \
  -f 'names[]=rust' \
  -f 'names[]=milmit' >/dev/null || true

echo
echo "Published: https://github.com/${OWNER}/${REPO_NAME}"
echo "Next: ./scripts/release-github.sh"
