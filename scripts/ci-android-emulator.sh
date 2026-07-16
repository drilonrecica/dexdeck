#!/usr/bin/env bash
set -euo pipefail

: "${ANDROID_HOME:=${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
export ANDROID_HOME ANDROID_SDK_ROOT="$ANDROID_HOME"

image="system-images;android-35;google_apis;x86_64"
sdkmanager --install "platform-tools" "emulator" "$image"
printf 'no\n' | avdmanager create avd --force --name dexdeck-ci --package "$image"
emulator -avd dexdeck-ci -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect &
emulator_pid=$!
trap 'kill "$emulator_pid" 2>/dev/null || true; adb kill-server 2>/dev/null || true' EXIT

adb wait-for-device
timeout 600 bash -c 'until [[ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d "\r")" == 1 ]]; do sleep 2; done'
adb shell input keyevent 82

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
