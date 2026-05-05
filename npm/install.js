const { execSync } = require('child_process');
const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');

const VERSION = '0.1.0';
const REPO = 'GOODBADBOY10/devpulse';
const BIN_DIR = path.join(__dirname, 'bin');

// Figure out which binary to download based on OS and CPU
function getBinaryName() {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === 'linux') return 'devpulse-linux-x64';
  if (platform === 'darwin') {
    return arch === 'arm64'
      ? 'devpulse-macos-arm64'
      : 'devpulse-macos-x64';
  }
  if (platform === 'win32') return 'devpulse-windows-x64.exe';

  throw new Error(`Unsupported platform: ${platform} ${arch}`);
}

// Download a file from a URL to a destination path
function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);

    const request = (targetUrl) => {
      https.get(targetUrl, (res) => {
        // Follow redirects (GitHub redirects release downloads)
        if (res.statusCode === 302 || res.statusCode === 301) {
          file.close();
          request(res.headers.location);
          return;
        }

        if (res.statusCode !== 200) {
          reject(new Error(`Download failed: HTTP ${res.statusCode}`));
          return;
        }

        res.pipe(file);
        file.on('finish', () => {
          file.close(resolve);
        });
      }).on('error', reject);
    };

    request(url);
  });
}

async function install() {
  const binaryName = getBinaryName();
  const downloadUrl =
    `https://github.com/${REPO}/releases/download/v${VERSION}/${binaryName}`;

  // Create bin directory if it doesn't exist
  if (!fs.existsSync(BIN_DIR)) {
    fs.mkdirSync(BIN_DIR, { recursive: true });
  }

  const isWindows = os.platform() === 'win32';
  const destName = isWindows ? 'devpulse.exe' : 'devpulse';
  const destPath = path.join(BIN_DIR, destName);

  console.log(`Downloading devpulse v${VERSION} for ${os.platform()}...`);
  console.log(`From: ${downloadUrl}`);

  await download(downloadUrl, destPath);

  // Make binary executable on Unix
  if (!isWindows) {
    fs.chmodSync(destPath, '755');
  }

  console.log('devpulse installed successfully!');
  console.log('Run: devpulse --help');
}

install().catch((err) => {
  console.error('Installation failed:', err.message);
  console.error('Please report this at https://github.com/GOODBADBOY10/devpulse/issues');
  process.exit(1);
});