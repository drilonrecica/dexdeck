# DexDeck Gradle bridge

The bridge is compiled with Java 17 and loaded only through the tracked Gradle
init script. `scripts/build-bridge.sh` creates a deterministic JAR; CI rebuilds
it and byte-compares its SHA-256 digest with `bridge/dexdeck-bridge.jar.sha256`.

Common output and adapter contracts live under `src/main`. The AGP 8 and AGP 9
adapters are separate source sets compiled against the pinned 8.0.2 and 9.0.1
public APIs. At runtime the init plugin detects AGP through the Android plugin's
classloader, then loads only the matching adapter in an isolated classloader.

The build reuses cached AGP API jars when available. On a clean machine it downloads
the two pinned API jars from Google's Android Maven repository and verifies their
SHA-256 digests before compilation. Minimal Gradle `Provider` interfaces are used
only at compile time and are deliberately excluded from the bridge jar.
