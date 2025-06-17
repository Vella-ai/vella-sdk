#!/bin/bash

set -e

# Check if required environment variables are set for Supabase S3
if [ -z "$SUPABASE_S3_ENDPOINT" ] || [ -z "$AWS_ACCESS_KEY_ID" ] || [ -z "$AWS_SECRET_ACCESS_KEY" ] || [ -z "$S3_BUCKET" ]; then
  echo "❌ Error: Please set SUPABASE_S3_ENDPOINT, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, and S3_BUCKET environment variables."
  exit 1
fi

# Read version from package.json
VERSION=$(jq -r '.version' package.json)
if [ -z "$VERSION" ]; then
    echo "❌ Error: Could not read version from package.json. Make sure jq is installed and package.json is valid."
    exit 1
fi

TMP_DIR=".release_tmp"
# Define the name for the final archive
ARCHIVE_NAME="vella-sdk-libs-$VERSION.tar.gz"

echo "➡️ Version: $VERSION"
echo "➡️ Archive: $ARCHIVE_NAME"

echo "📁 Creating temp dir: $TMP_DIR"
mkdir -p "$TMP_DIR"

# Define the source files and their desired names within the temp directory
# format: "output-name:source-path"
SOURCES=(
  "android-arm64-v8a.a:android/src/main/jniLibs/arm64-v8a/libvella_sdk.a"
  "android-armeabi-v7a.a:android/src/main/jniLibs/armeabi-v7a/libvella_sdk.a"
  "android-x86.a:android/src/main/jniLibs/x86/libvella_sdk.a"
  "android-x86_64.a:android/src/main/jniLibs/x86_64/libvella_sdk.a"
  "ios-arm64.a:ios/VellaSDK.xcframework/ios-arm64/libvella_sdk.a"
  "ios-arm64_x86_64-simulator.a:ios/VellaSDK.xcframework/ios-arm64_x86_64-simulator/libvella_sdk.a"
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
    echo "❌ Source file $SRC_PATH does not exist! Aborting."
    rm -rf "$TMP_DIR" # Clean up temp dir on error
    exit 1
  fi
done

echo "📦 Creating archive $ARCHIVE_NAME..."
# Create the tar.gz archive.
# -c: Create archive
# -z: Compress with gzip
# -f: Specify archive filename
# -C: Change to the specified directory before adding files
# .: Add all files from the current directory (which is TMP_DIR due to -C)
tar -czf "$ARCHIVE_NAME" -C "$TMP_DIR" .

echo "🚀 Uploading $ARCHIVE_NAME to Supabase S3 Storage..."

# Use the AWS S3 CLI to upload the file to your Supabase S3-compatible storage
# The --endpoint-url flag is crucial for directing the upload to Supabase
aws s3 cp "$ARCHIVE_NAME" "s3://$S3_BUCKET/$ARCHIVE_NAME" --endpoint-url "$SUPABASE_S3_ENDPOINT"

echo "🧹 Cleaning up..."
rm -rf "$TMP_DIR"
rm "$ARCHIVE_NAME"

echo "✅ Archive uploaded successfully to Supabase S3 bucket: $S3_BUCKET"
