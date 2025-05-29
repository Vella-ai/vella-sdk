#!/bin/bash

set -e

# Read version from package.json
VERSION=$(jq -r '.version' package.json)
RELEASE_TITLE="$VERSION"
TMP_DIR=".release_tmp"
# Define the name for the final archive
ARCHIVE_NAME="vella-sdk-libs-$VERSION.tar.gz"

echo "📁 Creating temp dir: $TMP_DIR"
mkdir -p "$TMP_DIR"

# Define the source and output filenames within the archive
SOURCES=(
  "android-arm64-v8a.a:android/src/main/jniLibs/arm64-v8a/libvella_sdk.a"
  "android-armeabi-v7a.a:android/src/main/jniLibs/armeabi-v7a/libvella_sdk.a"
  "android-x86.a:android/src/main/jniLibs/x86/libvella_sdk.a"
  "android-x86_64.a:android/src/main/jniLibs/x86_64/libvella_sdk.a"
  "ios-arm64.a:ios/VellaSDK.xcframework/ios-arm64/libvella_sdk.a"
  "ios-arm64_x86_64-simulator-simulator.a:ios/VellaSDK.xcframework/ios-arm64_x86_64-simulator/libvella_sdk.a"
)

echo "🔄 Copying files to temp directory..."
for ENTRY in "${SOURCES[@]}"; do
  # Split the entry into output filename and source file path
  OUT_NAME=$(echo "$ENTRY" | cut -d ':' -f 1)
  SRC_PATH=$(echo "$ENTRY" | cut -d ':' -f 2-)

  # Ensure the source file exists before copying
  if [[ -f "$SRC_PATH" ]]; then
    echo "  -> Copying $SRC_PATH to $TMP_DIR/$OUT_NAME"
    cp "$SRC_PATH" "$TMP_DIR/$OUT_NAME"
  else
    echo "❌ Source file $SRC_PATH does not exist!"
    rm -rf "$TMP_DIR" # Clean up temp dir on error
    exit 1
  fi
done

echo "📦 Creating archive $ARCHIVE_NAME..."
# Create the tar.gz archive.
# -c: Create archive
# -z: Compress with gzip
# -f: Specify archive filename
# -C "$TMP_DIR": Change to the TMP_DIR before adding files (avoids including .release_tmp/ path in the archive)
# .: Add all files from the current directory (which is TMP_DIR due to -C)
tar -czf "$ARCHIVE_NAME" -C "$TMP_DIR" .

echo "🚀 Creating GitHub release and uploading archive..."
# Upload only the single archive file
gh release create "$VERSION" \
  "$ARCHIVE_NAME" \
  --title "$RELEASE_TITLE" \
  --notes "Precompiled static libraries for vella-sdk $VERSION"

echo "🧹 Cleaning up..."
rm -rf "$TMP_DIR"
rm "$ARCHIVE_NAME"

echo "✅ Release created with archive $ARCHIVE_NAME!"
