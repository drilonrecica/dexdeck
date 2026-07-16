#!/usr/bin/env bash
set -euo pipefail

: "${ANDROID_HOME:=${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
export ANDROID_HOME ANDROID_SDK_ROOT="$ANDROID_HOME"

find_sdk_tool() {
  local name=$1
  shift
  local candidate
  for candidate in "$@"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  command -v "$name" 2>/dev/null || {
    printf 'Android SDK tool not found: %s\n' "$name" >&2
    return 1
  }
}

sdkmanager=$(find_sdk_tool sdkmanager \
  "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" \
  "$ANDROID_HOME/cmdline-tools/bin/sdkmanager")
avdmanager=$(find_sdk_tool avdmanager \
  "$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" \
  "$ANDROID_HOME/cmdline-tools/bin/avdmanager")

image="system-images;android-35;google_apis;x86_64"
"$sdkmanager" --install "platform-tools" "emulator" "$image"
adb=$(find_sdk_tool adb "$ANDROID_HOME/platform-tools/adb")
emulator=$(find_sdk_tool emulator "$ANDROID_HOME/emulator/emulator")
printf 'no\n' | "$avdmanager" create avd --force --name dexdeck-ci --package "$image"
"$emulator" -avd dexdeck-ci -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect &
emulator_pid=$!
trap 'kill "$emulator_pid" 2>/dev/null || true; "$adb" kill-server 2>/dev/null || true' EXIT

"$adb" wait-for-device
deadline=$((SECONDS + 600))
until [[ "$("$adb" shell getprop sys.boot_completed 2>/dev/null | tr -d "\r")" == 1 ]]; do
  if ((SECONDS >= deadline)); then
    printf 'Android emulator did not finish booting within 600 seconds\n' >&2
    exit 1
  fi
  sleep 2
done
"$adb" shell input keyevent 82

fixture_root="$RUNNER_TEMP/dexdeck-android-fixture"
project=$(cargo run -q -p dexdeck-test-support --bin write_android_fixture -- "$fixture_root")
mkdir -p "$project/app/src/main/java/dev/dexdeck/fixture" "$project/app/src/androidTest/java/dev/dexdeck/fixture"
cat > "$project/app/src/main/AndroidManifest.xml" <<'MANIFEST'
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
  <application android:theme="@android:style/Theme.Material.Light.NoActionBar">
    <activity android:name=".MainActivity" android:exported="true">
      <intent-filter>
        <action android:name="android.intent.action.MAIN" />
        <category android:name="android.intent.category.LAUNCHER" />
      </intent-filter>
    </activity>
  </application>
</manifest>
MANIFEST
cat > "$project/app/src/main/java/dev/dexdeck/fixture/MainActivity.java" <<'JAVA'
package dev.dexdeck.fixture;
public final class MainActivity extends android.app.Activity {}
JAVA
cat > "$project/app/src/androidTest/java/dev/dexdeck/fixture/SmokeTest.java" <<'JAVA'
package dev.dexdeck.fixture;
public final class SmokeTest extends android.test.InstrumentationTestCase {
  public void testDexDeckInstrumentation() { assertTrue(true); }
}
JAVA
cat >> "$project/app/build.gradle.kts" <<'GRADLE'
android { defaultConfig { applicationId = "dev.dexdeck.fixture"; minSdk = 23; testInstrumentationRunner = "android.test.InstrumentationTestRunner" } }
GRADLE

cargo build -p dexdeck
dexdeck_bin="$PWD/target/debug/dexdeck"
"$dexdeck_bin" --project "$project" doctor
"$dexdeck_bin" --project "$project" --module :app --variant debug build
"$dexdeck_bin" --project "$project" --module :app --variant debug --device emulator-5554 install --yes
"$dexdeck_bin" --project "$project" --module :app --variant debug --device emulator-5554 launch
"$dexdeck_bin" --project "$project" --module :app --variant debug --device emulator-5554 test --kind instrumentation
"$dexdeck_bin" --project "$project" --module :app --variant debug --device emulator-5554 stop
