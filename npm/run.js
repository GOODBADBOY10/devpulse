#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const os = require('os');

const isWindows = os.platform() === 'win32';
const binaryName = isWindows ? 'devpulse.exe' : 'devpulse';
const binaryPath = path.join(__dirname, 'bin', binaryName);

// Pass all arguments from the user straight to the binary
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',  // connect stdin/stdout/stderr directly
});

child.on('exit', (code) => {
  process.exit(code ?? 0);
});

child.on('error', (err) => {
  if (err.code === 'ENOENT') {
    console.error('devpulse binary not found.');
    console.error('Try reinstalling: npm install -g devpulse-cli');
  } else {
    console.error('Failed to run devpulse:', err.message);
  }
  process.exit(1);
});