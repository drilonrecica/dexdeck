#!/usr/bin/env bash
set -euo pipefail

output=${1:-target/benchmark-record}
mkdir -p "$output"

{
  printf 'timestamp_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'revision=%s\n' "$(git rev-parse HEAD)"
  printf 'dirty=%s\n' "$(test -z "$(git status --porcelain)" && echo false || echo true)"
  printf 'rustc=%s\n' "$(rustc --version)"
  printf 'cargo=%s\n' "$(cargo --version)"
  printf 'host=%s\n' "$(rustc -vV | sed -n 's/^host: //p')"
  printf 'os=%s\n' "$(uname -a)"
  if [[ -r /proc/cpuinfo ]]; then
    printf 'cpu=%s\n' "$(sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -1)"
  fi
  if [[ -r /proc/meminfo ]]; then
    printf 'memory=%s\n' "$(sed -n 's/^MemTotal:[[:space:]]*//p' /proc/meminfo)"
  fi
} > "$output/environment.txt"

cargo bench -p dexdeck --bench operational -- --save-baseline dexdeck
cargo bench -p dexdeck-android --bench logcat_pipeline -- --save-baseline dexdeck
cp -R target/criterion "$output/criterion"
