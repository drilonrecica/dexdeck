#!/bin/sh
set -eu
export SOURCE_DATE_EPOCH=0

if ! java -version 2>&1 | grep -Fq 'Temurin-17.0.19+10'; then
    echo 'bridge build requires Temurin 17.0.19+10' >&2
    exit 1
fi
command -v javac >/dev/null 2>&1 || { echo 'javac is required' >&2; exit 1; }
command -v jar >/dev/null 2>&1 || { echo 'jar is required' >&2; exit 1; }

DEPENDENCY_DIR=target/bridge-dependencies
mkdir -p "$DEPENDENCY_DIR"

resolve_agp_api() {
    version=$1
    expected=$2
    cached=$(find "$HOME/.gradle/caches/modules-2/files-2.1/com.android.tools.build/gradle-api/$version" \
        -name "gradle-api-$version.jar" -print -quit 2>/dev/null || true)
    destination="$DEPENDENCY_DIR/gradle-api-$version.jar"
    if test -n "$cached"; then
        cp "$cached" "$destination"
    elif ! test -f "$destination"; then
        command -v curl >/dev/null 2>&1 || { echo 'curl is required to fetch bridge APIs' >&2; exit 1; }
        curl --fail --location --silent --show-error \
            "https://dl.google.com/dl/android/maven2/com/android/tools/build/gradle-api/$version/gradle-api-$version.jar" \
            --output "$destination"
    fi
    actual=$(sha256sum "$destination" | cut -d ' ' -f 1)
    test "$actual" = "$expected" || {
        echo "bridge API checksum mismatch for AGP $version" >&2
        rm -f "$destination"
        exit 1
    }
    printf '%s\n' "$destination"
}

AGP8_API=$(resolve_agp_api 8.0.2 e3a14c889d8e6b3e1c026015950db54a4d1bd09ade82e05307adba603f46f036)
AGP9_API=$(resolve_agp_api 9.0.1 4fb70676b7afa6922a02a4cd14a472b3122c369bd5595006fdaf0357b0e1bd47)

rm -rf target/bridge-common target/bridge-compile-only target/bridge-agp8 target/bridge-agp9
mkdir -p target/bridge-common target/bridge-compile-only target/bridge-agp8 target/bridge-agp9
javac --release 17 -d target/bridge-common \
    bridge/src/main/java/dev/dexdeck/bridge/ModelAdapter.java \
    bridge/src/main/java/dev/dexdeck/bridge/ModelBridge.java
javac --release 17 -d target/bridge-compile-only \
    bridge/src/compileOnly/java/org/gradle/api/provider/Provider.java \
    bridge/src/compileOnly/java/org/gradle/api/provider/Property.java
javac --release 17 -cp "target/bridge-common:target/bridge-compile-only:$AGP8_API" \
    -d target/bridge-agp8 bridge/src/agp8/java/dev/dexdeck/bridge/agp8/Agp8ModelAdapter.java
javac --release 17 -cp "target/bridge-common:target/bridge-compile-only:$AGP9_API" \
    -d target/bridge-agp9 bridge/src/agp9/java/dev/dexdeck/bridge/agp9/Agp9ModelAdapter.java
jar --create --file bridge/dexdeck-bridge.jar --date=1980-01-01T00:00:02Z \
    -C target/bridge-common . -C target/bridge-agp8 . -C target/bridge-agp9 .
sha256sum bridge/dexdeck-bridge.jar | sed 's#  bridge/#  #' > bridge/dexdeck-bridge.jar.sha256
