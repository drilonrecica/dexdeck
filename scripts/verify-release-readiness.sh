#!/usr/bin/env bash
set -euo pipefail

test "$(git status --porcelain)" = ""
git diff --check
version=$(sed -n 's/^version = "\([0-9][0-9.]*\)"$/\1/p' Cargo.toml | head -n1)
[[ -n "$version" ]]
grep -q '^rust-version = "1\.97"$' Cargo.toml
grep -q '^cargo-dist-version = "0\.31\.0"$' dist-workspace.toml
grep -q "^## ${version//./\\.} — " RELEASES.md
grep -q 'Windows x86-64 is experimental' RELEASES.md
bash scripts/verify-user-docs.sh
python3 scripts/generate-homebrew-formula.py --self-test
(cd bridge && sha256sum --check dexdeck-bridge.jar.sha256)

if git tag --points-at HEAD | grep -q "^v${version}$"; then
  printf 'v%s already points at this commit; validate every external gate before tagging\n' "$version" >&2
  exit 1
fi
