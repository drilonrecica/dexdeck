#!/usr/bin/env bash
set -euo pipefail

root=${1:-target/distrib}
mapfile -t archives < <(find "$root" -type f \( -name 'dexdeck-*.tar.gz' -o -name 'dexdeck-*.zip' \) | LC_ALL=C sort)

if [[ ${#archives[@]} -ne 5 ]]; then
  printf 'expected five release archives, found %s\n' "${#archives[@]}" >&2
  exit 1
fi

for archive in "${archives[@]}"; do
  checksum="$archive.sha256"
  [[ -f "$checksum" ]] || { printf 'missing checksum for %s\n' "$archive" >&2; exit 1; }
  expected=$(awk '{print $1}' "$checksum")
  actual=$(python3 -c 'import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' "$archive")
  [[ "$actual" == "$expected" ]] || { printf 'checksum mismatch for %s\n' "$archive" >&2; exit 1; }
  if [[ "$archive" == *.tar.gz ]]; then
    listing=$(tar -tzf "$archive")
    details=$(tar -tvzf "$archive")
    grep -qE '/dexdeck$' <<<"$listing"
    grep -qE '/README.md$' <<<"$listing"
    grep -qE '/LICENSE$' <<<"$listing"
    grep -qE '/man/man1/dexdeck.1$' <<<"$listing"
    grep -qE '^-rwxr-xr-x .*/dexdeck$' <<<"$details"
  else
    listing=$(unzip -Z1 "$archive")
    grep -qE '/dexdeck.exe$' <<<"$listing"
    grep -qE '/README.md$' <<<"$listing"
    grep -qE '/LICENSE$' <<<"$listing"
    grep -qE '/man/man1/dexdeck.1$' <<<"$listing"
  fi
done
