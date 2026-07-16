#!/usr/bin/env sh
set -eu

version=${DEXDECK_VERSION:-0.2.0}
repository=${DEXDECK_REPOSITORY:-drilonrecica/dexdeck}
install_dir=${DEXDECK_INSTALL_DIR:-${CARGO_HOME:-$HOME/.cargo}/bin}
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  *) printf 'unsupported platform; install a reviewed archive manually\n' >&2; exit 1 ;;
esac

archive="dexdeck-${version}-${target}.tar.gz"
base="https://github.com/${repository}/releases/download/v${version}"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT HUP INT TERM
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error "$base/$archive" -o "$temporary/$archive"
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error "$base/$archive.sha256" -o "$temporary/$archive.sha256"
(cd "$temporary" && sha256sum --check "$archive.sha256")
tar -xzf "$temporary/$archive" -C "$temporary"
mkdir -p "$install_dir"
install -m 755 "$temporary/dexdeck-${version}-${target}/dexdeck" "$install_dir/dexdeck"
printf 'DexDeck %s installed at %s/dexdeck\n' "$version" "$install_dir"
