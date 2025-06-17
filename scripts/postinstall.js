const fs = require('fs');
const https = require('https');
const path = require('path');
const os = require('os');
const cliProgress = require('cli-progress');
const tar = require('tar');

const SUPABASE_URL = 'https://qqzbfyxqzqdkcycxyoyj.supabase.co';
const SUPABASE_BUCKET = 'vella-sdk';

let version;
try {
  version = require('../package.json').version;
} catch (error) {
  console.error(
    '❌ Error: Could not load package.json or find version.',
    error
  );
  process.exit(1);
}

// Construct the Supabase Storage URL
const archiveName = `vella-sdk-libs-${version}.tar.gz`;
const archiveUrl = `${SUPABASE_URL}/storage/v1/object/public/${SUPABASE_BUCKET}/${archiveName}`;

const projectRoot = path.join(__dirname, '..');
const versionFilePath = path.join(projectRoot, '.binary-version');

const binaries = [
  {
    name: 'android-arm64-v8a.a',
    dest: 'android/src/main/jniLibs/arm64-v8a/libvella_sdk.a',
  },
  {
    name: 'android-armeabi-v7a.a',
    dest: 'android/src/main/jniLibs/armeabi-v7a/libvella_sdk.a',
  },
  {
    name: 'android-x86.a',
    dest: 'android/src/main/jniLibs/x86/libvella_sdk.a',
  },
  {
    name: 'android-x86_64.a',
    dest: 'android/src/main/jniLibs/x86_64/libvella_sdk.a',
  },
  {
    name: 'ios-arm64.a',
    dest: 'ios/VellaSDK.xcframework/ios-arm64/libvella_sdk.a',
  },
  {
    name: 'ios-arm64_x86_64-simulator.a', // Note: Corrected name from original script for consistency
    dest: 'ios/VellaSDK.xcframework/ios-arm64_x86_64-simulator/libvella_sdk.a',
  },
];

function formatBytes(bytes) {
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  if (bytes === 0) return '0 Bytes';
  const i = parseInt(Math.floor(Math.log(bytes) / Math.log(1024)), 10);
  if (i === 0) return `${bytes} ${sizes[i]}`;
  return `${(bytes / 1024 ** i).toFixed(1)} ${sizes[i]}`;
}

async function shouldSkipDownload() {
  console.log(`\n🔍 Checking state for v${version}...`);
  let storedVersion = null;

  try {
    storedVersion = await fs.promises.readFile(versionFilePath, 'utf8');
    storedVersion = storedVersion.trim();
  } catch (err) {
    if (err.code === 'ENOENT') {
      // No stored version, download is needed
      return false;
    }
    // Safer to download if we can't read the state
    return false;
  }

  if (storedVersion !== version) {
    return false;
  }

  for (const bin of binaries) {
    const destPath = path.join(projectRoot, bin.dest);
    try {
      await fs.promises.access(destPath, fs.constants.F_OK);
    } catch {
      // A file is missing or inaccessible, re-download
      return false;
    }
  }

  return true;
}

function downloadWithProgress(url, dest) {
  return new Promise((resolve, reject) => {
    console.log(`\n⬇️ Downloading ${path.basename(dest)} from Supabase...`);

    const download = (currentUrl) => {
      https
        .get(currentUrl, { timeout: 300000 }, (res) => {
          if (
            res.statusCode >= 300 &&
            res.statusCode < 400 &&
            res.headers.location
          ) {
            const redirectUrl = new URL(
              res.headers.location,
              currentUrl
            ).toString();
            return download(redirectUrl);
          }

          if (res.statusCode !== 200) {
            res.resume();
            return reject(
              new Error(
                `Download failed - status ${res.statusCode}\nURL: ${currentUrl}`
              )
            );
          }

          const totalHeader = res.headers['content-length'];
          const total = totalHeader ? parseInt(totalHeader, 10) : 0;

          const progressBar = new cliProgress.SingleBar(
            {
              format:
                '{filename} [{bar}] {percentage}% | {formattedValue} / {formattedTotal}',
              hideCursor: true,
            },
            cliProgress.Presets.shades_classic
          );

          const displayFilename = path.basename(dest);

          progressBar.start(total, 0, {
            filename: displayFilename.padEnd(archiveName.length, ' '),
            formattedTotal: total ? formatBytes(total) : 'Unknown size',
            formattedValue: formatBytes(0).padStart(10, ' '),
          });

          const file = fs.createWriteStream(dest);
          let downloaded = 0;

          res.on('data', (chunk) => {
            downloaded += chunk.length;
            const currentProgress = total
              ? Math.min(downloaded, total)
              : downloaded;
            progressBar.update(currentProgress, {
              formattedValue: formatBytes(currentProgress).padStart(10, ' '),
            });
          });

          res.pipe(file);

          file.on('finish', () => {
            file.close(() => {
              const finalProgress = total || downloaded;
              progressBar.update(finalProgress, {
                formattedValue: formatBytes(finalProgress).padStart(10, ' '),
              });
              progressBar.stop();
              resolve();
            });
          });

          file.on('error', (err) => {
            progressBar.stop();
            fs.unlink(dest, () => {}); // Attempt to delete incomplete file
            reject(
              new Error(`File system error writing to ${dest}: ${err.message}`)
            );
          });
        })
        .on('error', (err) => {
          reject(new Error(`Network error downloading ${url}: ${err.message}`));
        })
        .on('timeout', () => {
          reject(new Error(`Network timeout downloading ${url}`));
        });
    };

    download(url);
  });
}

(async () => {
  const skipDownload = await shouldSkipDownload();
  if (skipDownload) {
    console.log(
      '\n🎉 Binaries are up-to-date. Skipping download and extraction.'
    );
    return;
  }

  console.log(`\n📦 Fetching native binaries archive for v${version}...`);
  let tempDir = null;

  try {
    tempDir = await fs.promises.mkdtemp(
      path.join(os.tmpdir(), `vella-sdk-${version}-`)
    );
    const tempArchivePath = path.join(tempDir, archiveName);
    const tempExtractDir = path.join(tempDir, 'extracted');
    await fs.promises.mkdir(tempExtractDir);

    await downloadWithProgress(archiveUrl, tempArchivePath);
    console.log(
      `✅ Archive downloaded to temporary location: ${tempArchivePath}`
    );

    console.log(`\n📦 Extracting ${archiveName}...`);
    await tar.x({
      file: tempArchivePath,
      C: tempExtractDir,
    });
    console.log('✅ Extraction complete.');

    console.log('\n🚚 Moving binaries to final locations...');
    let filesMovedCount = 0;
    const expectedFiles = binaries.length;

    for (const bin of binaries) {
      const sourcePath = path.join(tempExtractDir, bin.name);
      const destPath = path.join(projectRoot, bin.dest);

      await fs.promises.mkdir(path.dirname(destPath), { recursive: true });

      try {
        await fs.promises.access(sourcePath, fs.constants.F_OK);
        await fs.promises.rename(sourcePath, destPath);
        console.log(`  -> ${bin.dest}`);
        filesMovedCount++;
      } catch (err) {
        if (err.code === 'ENOENT') {
          console.warn(
            `⚠️ WARNING: Binary ${bin.name} not found in extracted archive at ${sourcePath}, skipping move.`
          );
        } else {
          throw new Error(
            `Error moving ${bin.name} to ${destPath}: ${err.message}`
          );
        }
      }
    }

    if (filesMovedCount > 0) {
      console.log(
        `\n💾 Storing current version (${version}) to ${path.basename(versionFilePath)}...`
      );
      await fs.promises.writeFile(versionFilePath, version, 'utf8');
      console.log('✅ Version stored.');
    } else {
      throw new Error('No binaries were found or moved from the archive.');
    }

    if (filesMovedCount !== expectedFiles) {
      console.warn(
        `\n⚠️ Processed ${filesMovedCount} out of ${expectedFiles} expected binaries.`
      );
    } else {
      console.log('\n🎉 All expected binaries processed successfully.');
    }
  } catch (err) {
    console.error(`\n❌ Operation failed: ${err.message}`);
    process.exitCode = 1;

    // Attempt to remove version file on failure to force re-download next time
    try {
      await fs.promises.unlink(versionFilePath);
      console.log(`  -> Removed potentially outdated version file.`);
    } catch (unlinkErr) {
      if (unlinkErr.code !== 'ENOENT') {
        console.warn(
          `  ⚠️ Could not remove version file: ${unlinkErr.message}`
        );
      }
    }
  } finally {
    if (tempDir) {
      console.log(`\n🧹 Cleaning up temporary files...`);
      try {
        await fs.promises.rm(tempDir, { recursive: true, force: true });
        console.log('✅ Cleanup complete.');
      } catch (cleanupErr) {
        console.error(
          `⚠️ Failed to cleanup temporary directory ${tempDir}: ${cleanupErr.message}`
        );
      }
    }
  }
})();
