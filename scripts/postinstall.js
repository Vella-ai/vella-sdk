const fs = require('fs');
const https = require('https');
const path = require('path');
const os = require('os');
const cliProgress = require('cli-progress');
const tar = require('tar');

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

const baseURL = `https://github.com/Vella-ai/vella-sdk/releases/download/${version}/`;
const archiveName = `vella-sdk-libs-${version}.tar.gz`;
const archiveUrl = baseURL + archiveName;

const projectRoot = path.join(__dirname, '..');

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
    name: 'ios-arm64-simulator.a',
    dest: 'ios/VellaSDK.xcframework/ios-arm64-simulator/libvella_sdk.a',
  },
];

function formatBytes(bytes) {
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  if (bytes === 0) return '0 Bytes';
  const i = parseInt(Math.floor(Math.log(bytes) / Math.log(1024)), 10);
  if (i === 0) return bytes + ' ' + sizes[i];
  return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + sizes[i];
}

async function checkBinariesExist() {
  console.log(
    `\n🔍 Checking if native binaries for v${version} already exist...`
  );
  for (const bin of binaries) {
    const destPath = path.join(projectRoot, bin.dest);
    try {
      await fs.promises.access(destPath, fs.constants.F_OK);
    } catch (err) {
      if (err.code === 'ENOENT') {
        console.log(`  ❌ Missing: ${bin.dest}`);
        return false;
      } else {
        console.error(`  ⚠️ Error checking file ${destPath}: ${err.message}`);
        return false;
      }
    }
  }
  console.log(`👍 All required binaries for v${version} are already present.`);
  return true;
}

function downloadWithProgress(url, dest) {
  return new Promise((resolve, reject) => {
    console.log(`\n⬇️ Downloading ${path.basename(dest)}...`);

    const download = (currentUrl) => {
      https
        .get(currentUrl, { timeout: 30000 }, (res) => {
          if (
            res.statusCode >= 300 &&
            res.statusCode < 400 &&
            res.headers.location
          ) {
            console.log(`  Redirecting to ${res.headers.location}...`);
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
              // Ensure progress bar reaches 100% if total size was known
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
            fs.unlink(dest, () => {});
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
  const binariesAlreadyExist = await checkBinariesExist();
  if (binariesAlreadyExist) {
    console.log('\n🎉 Skipping download and extraction.');
    process.exitCode = 0;
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
    await fs.promises.mkdir(tempExtractDir); // Ensure extraction target exists

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
          console.error(
            `❌ Error moving ${bin.name} to ${destPath}: ${err.message}`
          );
          throw err;
        }
      }
    }

    if (filesMovedCount === binaries.length) {
      console.log('\n🎉 All expected binaries processed successfully.');
    } else if (filesMovedCount > 0) {
      console.log(
        `\n⚠️ Processed ${filesMovedCount} out of ${binaries.length} expected binaries.`
      );
    } else {
      console.error(
        `\n❌ No binaries were moved. Check archive content and paths.`
      );
      throw new Error('No binaries found or moved from the archive.');
    }
  } catch (err) {
    console.error(`\n❌ Operation failed: ${err.message}`);
    if (err.stack) {
      console.error(err.stack);
    }
    process.exitCode = 1;
  } finally {
    if (tempDir) {
      console.log(`\n🧹 Cleaning up temporary files in ${tempDir}...`);
      try {
        await fs.promises.rm(tempDir, { recursive: true, force: true });
        console.log('✅ Cleanup complete.');
      } catch (cleanupErr) {
        console.error(
          `⚠️ Failed to cleanup temporary directory ${tempDir}: ${cleanupErr.message}`
        );
        if (!process.exitCode) {
          process.exitCode = 1;
        }
      }
    }
  }
})();
