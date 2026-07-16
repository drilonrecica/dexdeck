# DexDeck Gradle bridge

The bridge is compiled with Java 17 and loaded only through the tracked Gradle
init script. `scripts/build-bridge.sh` creates a deterministic JAR; CI rebuilds
it and byte-compares its SHA-256 digest with `bridge/dexdeck-bridge.jar.sha256`.
