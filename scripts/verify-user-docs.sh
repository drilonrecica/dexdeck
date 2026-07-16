#!/usr/bin/env bash
set -euo pipefail

commands=(init doctor project modules variants devices emulators build install launch run rerun reinstall clean-reinstall stop uninstall clear-data test logs gradle emulator command version)
files=(man/dexdeck.1 completions/dexdeck.bash completions/_dexdeck completions/dexdeck.fish)

for command in "${commands[@]}"; do
  for file in "${files[@]}"; do
    grep -Eq "(^|[^a-z-])${command}([^a-z-]|$)" "$file" || {
      printf '%s is missing command %s\n' "$file" "$command" >&2
      exit 1
    }
  done
done

grep -Eq 'DexDeck 0\.2\.0' man/dexdeck.1 docs/demo.cast
grep -q 'docs/installation.md' README.md
grep -q 'docs/privacy.md' README.md
if grep -ERni 'droiddeck|DROIDDECK_' README.md docs completions man; then
  printf 'retired product identifier found in user documentation\n' >&2
  exit 1
fi
