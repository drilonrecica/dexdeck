#!/usr/bin/env bash
set -euo pipefail

test "$(git status --porcelain)" = ""
git diff --check
rg -q '^version = "0\.2\.0"$' Cargo.toml
rg -q '^rust-version = "1\.97"$' Cargo.toml
rg -q '^cargo-dist-version = "0\.31\.0"$' dist-workspace.toml
rg -q '^## 0\.2\.0 — release candidate$' RELEASES.md
rg -q 'Windows x86-64 is experimental' RELEASES.md
bash scripts/verify-user-docs.sh
python3 scripts/generate-homebrew-formula.py --self-test
(cd bridge && sha256sum --check dexdeck-bridge.jar.sha256)

if git tag --points-at HEAD | rg -q '^v0\.2\.0$'; then
  printf 'v0.2.0 already points at this commit; validate every external gate before tagging\n' >&2
  exit 1
fi
