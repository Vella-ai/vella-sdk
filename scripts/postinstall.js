const fs = require('fs');
const https = require('https');
const path = require('path');
const cliProgress = require('cli-progress');

const version = require('../package.json').version;
const baseURL = `https://github.com/Vella-ai/vella-sdk/releases/download/${version}/`;

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
  return Math.round(bytes / Math.pow(1024, i)) + ' ' + sizes[i];
}

function downloadWithProgress(url, dest) {
  return new Promise((resolve, reject) => {
    const dir = path.dirname(dest);
    fs.mkdirSync(dir, { recursive: true });

    const download = (currentUrl) => {
      https
        .get(currentUrl, (res) => {
          // Check for redirect (status 3xx)
          if (
            res.statusCode >= 300 &&
            res.statusCode < 400 &&
            res.headers.location
          ) {
            return download(res.headers.location);
          }

          if (res.statusCode !== 200) {
            return reject(
              new Error(
                `Failed to download ${currentUrl} - status ${res.statusCode}`
              )
            );
          }

          const total = parseInt(res.headers['content-length'], 10);
          const progressBar = new cliProgress.SingleBar(
            {
              format: `⬇️ {filename} [{bar}] {percentage}% | {formattedValue} / {formattedTotal}`,
              hideCursor: true,
            },
            cliProgress.Presets.shades_classic
          );

          progressBar.start(total, 0, {
            filename: path.basename(dest),
            formattedTotal: formatBytes(total),
          });

          const file = fs.createWriteStream(dest);
          let downloaded = 0;

          res.on('data', (chunk) => {
            downloaded += chunk.length;
            progressBar.update(downloaded, {
              formattedValue: formatBytes(downloaded),
            });
          });

          res.pipe(file);
          file.on('finish', () => {
            file.close(() => {
              progressBar.stop();
              console.log(`✅ Downloaded: ${dest}`);
              resolve();
            });
          });
        })
        .on('error', reject);
    };

    download(url);
  });
}

(async () => {
  console.log('📦 Fetching native binaries for v' + version + '...');
  for (const bin of binaries) {
    const url = baseURL + bin.name;
    const output = path.join(__dirname, '..', bin.dest);
    try {
      await downloadWithProgress(url, output);
    } catch (err) {
      console.error(`❌ Failed to download ${bin.name}: ${err.message}`);
      process.exit(1);
    }
  }
  console.log('🎉 All binaries downloaded successfully.');
})();
