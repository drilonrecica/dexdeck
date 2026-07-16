#!/usr/bin/env bash
set -euo pipefail

for pattern in \
  'TcpStream|TcpListener|UdpSocket|tokio::net|std::net' \
  'reqwest::|hyper::|sentry::|opentelemetry::' \
  'Command::new\("(sh|bash)"\)|cmd\.exe /C'
do
  if grep -ERn --include='*.rs' --exclude='privacy_contract.rs' "$pattern" crates; then
    printf 'forbidden local-only runtime pattern: %s\n' "$pattern" >&2
    exit 1
  fi
done

cargo test -p dexdeck --test privacy_contract
cargo tree --workspace --edges normal > /dev/null

if command -v strace >/dev/null 2>&1 && [[ "$(uname -s)" == Linux ]]; then
  cargo build -p dexdeck
  trace=$(mktemp)
  trap 'rm -f "$trace"' EXIT
  strace -f -e trace=network -o "$trace" target/debug/dexdeck version >/dev/null
  if grep -E 'socket\(|connect\(|sendto\(|recvfrom\(' "$trace"; then
    printf 'DexDeck opened a runtime socket during the local-only smoke test\n' >&2
    exit 1
  fi
fi
