#!/usr/bin/env node
import React from 'react';
import { render } from 'ink';
import App from './App';

const { waitUntilExit, unmount } = render(React.createElement(App), {
  exitOnCtrlC: false,
  patchConsole: false,
});

// Ctrl+C double-press handler at process level
let sigintCount = 0;
let sigintTimer: ReturnType<typeof setTimeout> | null = null;

process.on('SIGINT', () => {
  sigintCount++;

  if (sigintCount >= 2) {
    // Second press → quit
    if (sigintTimer) clearTimeout(sigintTimer);
    unmount();
    process.exit(0);
  }

  // First press → show hint, reset after 3s
  console.log('\n  再次按 Ctrl+C 退出');
  if (sigintTimer) clearTimeout(sigintTimer);
  sigintTimer = setTimeout(() => {
    sigintCount = 0;
    sigintTimer = null;
  }, 3000);
});

waitUntilExit();
