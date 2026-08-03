#!/usr/bin/env node
'use strict';

const { spawnSync } = require('child_process');
const path = require('path');

const isWin = process.platform === 'win32';
const binName = isWin ? 'aura-bin.exe' : 'aura-bin';
const binPath = path.join(__dirname, binName);

const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

if (result.error) {
  if (result.error.code === 'ENOENT') {
    process.stderr.write(
      'aura binary not found. Try reinstalling: npm install -g aura\n'
    );
  } else {
    process.stderr.write(`aura: ${result.error.message}\n`);
  }
  process.exit(1);
}

process.exit(result.status ?? 0);
