#!/usr/bin/env bash
# Keep npm package versions aligned with Cargo.toml.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n1)"
if [[ -z "$VERSION" ]]; then
  echo "could not read version from Cargo.toml" >&2
  exit 1
fi

update() {
  local file="$1"
  if command -v node >/dev/null 2>&1; then
    node -e "
      const fs = require('fs');
      const p = process.argv[1];
      const v = process.argv[2];
      const j = JSON.parse(fs.readFileSync(p, 'utf8'));
      j.version = v;
      if (j.optionalDependencies) {
        for (const k of Object.keys(j.optionalDependencies)) {
          if (k.startsWith('@fallow-luau/')) j.optionalDependencies[k] = v;
        }
      }
      fs.writeFileSync(p, JSON.stringify(j, null, 2) + '\n');
    " "$file" "$VERSION"
  else
    echo "node required to sync $file" >&2
    exit 1
  fi
}

update "$ROOT/npm/fallow-luau/package.json"
for pkg in "$ROOT"/npm/@fallow-luau/*/package.json; do
  update "$pkg"
done
echo "synced npm versions to $VERSION"
