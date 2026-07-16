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

cargo test -p dexdeck-android application::tests
cargo test -p dexdeck-android emulator::tests
cargo test -p dexdeck-gradle test_runner::tests
cargo run -p dexdeck -- devices list
