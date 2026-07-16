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
  (cd "$(dirname "$archive")" && sha256sum --check "$(basename "$checksum")")
  if [[ "$archive" == *.tar.gz ]]; then
    listing=$(tar -tzf "$archive")
    details=$(tar -tvzf "$archive")
    rg -q '/dexdeck$' <<<"$listing"
    rg -q '/README.md$' <<<"$listing"
    rg -q '/LICENSE$' <<<"$listing"
    rg -q '/man/man1/dexdeck.1$' <<<"$listing"
    rg -q '^-rwxr-xr-x .* /dexdeck$' <<<"$details"
  else
    listing=$(unzip -Z1 "$archive")
    rg -q '/dexdeck.exe$' <<<"$listing"
    rg -q '/README.md$' <<<"$listing"
    rg -q '/LICENSE$' <<<"$listing"
    rg -q '/man/man1/dexdeck.1$' <<<"$listing"
  fi
done
