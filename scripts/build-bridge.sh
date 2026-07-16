#!/bin/sh
set -eu
export SOURCE_DATE_EPOCH=0
rm -rf target/bridge-classes
mkdir -p target/bridge-classes
javac --release 17 -d target/bridge-classes bridge/src/main/java/dev/dexdeck/bridge/ModelBridge.java
jar --create --file bridge/dexdeck-bridge.jar --date=1980-01-01T00:00:02Z -C target/bridge-classes .
sha256sum bridge/dexdeck-bridge.jar | sed 's#  bridge/#  #' > bridge/dexdeck-bridge.jar.sha256
