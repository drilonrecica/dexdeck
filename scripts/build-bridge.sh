#!/bin/sh
set -eu
export SOURCE_DATE_EPOCH=0

if ! java -version 2>&1 | grep -Fq 'Temurin-17.0.19+10'; then
    echo 'bridge build requires Temurin 17.0.19+10' >&2
    exit 1
fi
command -v javac >/dev/null 2>&1 || { echo 'javac is required' >&2; exit 1; }
command -v jar >/dev/null 2>&1 || { echo 'jar is required' >&2; exit 1; }

rm -rf target/bridge-classes
mkdir -p target/bridge-classes
javac --release 17 -d target/bridge-classes bridge/src/main/java/dev/dexdeck/bridge/ModelBridge.java
jar --create --file bridge/dexdeck-bridge.jar --date=1980-01-01T00:00:02Z -C target/bridge-classes .
sha256sum bridge/dexdeck-bridge.jar | sed 's#  bridge/#  #' > bridge/dexdeck-bridge.jar.sha256
