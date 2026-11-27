#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${1:-$ROOT_DIR/target/dist}"
[[ -d "$DIST_DIR" ]] || { echo "Artifact directory not found: $DIST_DIR" >&2; exit 1; }

mapfile -t FILES < <(cd "$DIST_DIR" && find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%f\n' | sort)
[[ ${#FILES[@]} -gt 0 ]] || { echo "No artifacts found in $DIST_DIR" >&2; exit 1; }

(cd "$DIST_DIR" && sha256sum -- "${FILES[@]}" > SHA256SUMS)
printf 'File\tBytes\tSHA256\n'
while IFS= read -r file; do
  [[ -f "$DIST_DIR/$file" ]] || continue
  size="$(stat -c '%s' "$DIST_DIR/$file")"
  hash="$(sha256sum "$DIST_DIR/$file" | cut -d ' ' -f1)"
  printf '%s\t%s\t%s\n' "$file" "$size" "$hash"
done < <(cd "$DIST_DIR" && find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%f\n' | sort)
