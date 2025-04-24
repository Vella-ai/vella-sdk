const fs = require('fs');
const https = require('https');
const path = require('path');
const os = require('os');
const cliProgress = require('cli-progress');
const tar = require('tar');

const version = require('../package.json').version;
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

function downloadWithProgress(url, dest) {
  return new Promise((resolve, reject) => {
    console.log(`\n⬇️ Downloading ${path.basename(dest)}...`);

    const download = (currentUrl) => {
      https
        .get(currentUrl, (res) => {
          // Follow redirects
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
              format: `{filename} [{bar}] {percentage}% | {formattedValue} / {formattedTotal}`,
              hideCursor: true,
            },
            cliProgress.Presets.shades_classic
          );

          const displayFilename = path.basename(dest);

          progressBar.start(total, 0, {
            filename: displayFilename,
            formattedTotal: total ? formatBytes(total) : 'Unknown size',
            formattedValue: formatBytes(0),
          });

          const file = fs.createWriteStream(dest);
          let downloaded = 0;

          res.on('data', (chunk) => {
            downloaded += chunk.length;
            const currentProgress = Math.min(downloaded, total || downloaded);
            progressBar.update(currentProgress, {
              formattedValue: formatBytes(currentProgress),
            });
          });

          res.pipe(file);

          file.on('finish', () => {
            file.close(() => {
              const finalProgress = total || downloaded;
              progressBar.update(finalProgress, {
                formattedValue: formatBytes(finalProgress),
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
        });
    };
    download(url);
  });
}

(async () => {
  console.log(`📦 Fetching native binaries archive for v${version}...`);
  let tempDir = null;

  try {
    tempDir = await fs.promises.mkdtemp(
      path.join(os.tmpdir(), `vella-sdk-${version}-`)
    );
    const tempArchivePath = path.join(tempDir, archiveName);
    const tempExtractDir = path.join(tempDir, 'extracted');
    await fs.promises.mkdir(tempExtractDir);

    await downloadWithProgress(archiveUrl, tempArchivePath);
    console.log(`✅ Archive downloaded to temporary location.`);

    console.log(`\n📦 Extracting ${archiveName}...`);
    await tar.x({
      file: tempArchivePath,
      C: tempExtractDir,
    });
    console.log('✅ Extraction complete.');

    console.log('\n🚚 Moving binaries to final locations...');
    for (const bin of binaries) {
      const sourcePath = path.join(tempExtractDir, bin.name);
      const destPath = path.join(projectRoot, bin.dest);

      await fs.promises.mkdir(path.dirname(destPath), { recursive: true });

      try {
        await fs.promises.access(sourcePath, fs.constants.F_OK);
        await fs.promises.rename(sourcePath, destPath);
        console.log(`-> ${bin.dest}`);
      } catch (err) {
        if (err.code === 'ENOENT') {
          console.warn(
            `⚠️ WARNING: Binary ${bin.name} not found in archive, skipping.`
          );
        } else {
          throw err;
        }
      }
    }

    console.log('\n🎉 All binaries processed successfully.');
  } catch (err) {
    console.error(`\n❌ Operation failed: ${err.message}`);
    process.exitCode = 1;
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
