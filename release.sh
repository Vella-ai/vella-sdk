#!/bin/bash

set -e

# Read version from package.json
VERSION=$(jq -r '.version' package.json)
RELEASE_TITLE="$VERSION"
TMP_DIR=".release_tmp"

echo "📁 Creating temp dir: $TMP_DIR"
mkdir -p "$TMP_DIR"

# Define the source and output filenames
SOURCES=(
  "android-arm64-v8a.a:android/src/main/jniLibs/arm64-v8a/libvella_sdk.a"
  "android-armeabi-v7a.a:android/src/main/jniLibs/armeabi-v7a/libvella_sdk.a"
  "android-x86.a:android/src/main/jniLibs/x86/libvella_sdk.a"
  "android-x86_64.a:android/src/main/jniLibs/x86_64/libvella_sdk.a"
  "ios-arm64.a:ios/VellaSDK.xcframework/ios-arm64/libvella_sdk.a"
  "ios-arm64-simulator.a:ios/VellaSDK.xcframework/ios-arm64-simulator/libvella_sdk.a"
)

echo "🔄 Copying files to temp directory..."
for ENTRY in "${SOURCES[@]}"; do
  # Split the entry into output filename and source file path
  OUT_NAME=$(echo $ENTRY | cut -d ':' -f 1)
  SRC_PATH=$(echo $ENTRY | cut -d ':' -f 2)

  # Ensure the source file exists before copying
  if [[ -f "$SRC_PATH" ]]; then
    cp "$SRC_PATH" "$TMP_DIR/$OUT_NAME"
  else
    echo "❌ Source file $SRC_PATH does not exist!"
    exit 1
  fi
done

echo "🚀 Creating GitHub release..."
gh release create "$VERSION" \
  "$TMP_DIR/android-arm64-v8a.a" \
  "$TMP_DIR/android-armeabi-v7a.a" \
  "$TMP_DIR/android-x86.a" \
  "$TMP_DIR/android-x86_64.a" \
  "$TMP_DIR/ios-arm64.a" \
  "$TMP_DIR/ios-arm64-simulator.a" \
  --title "$RELEASE_TITLE" \
  --notes "Precompiled static libraries for vella-sdk $VERSION"

echo "🧹 Cleaning up..."
rm -rf "$TMP_DIR"

echo "✅ Release created!"
