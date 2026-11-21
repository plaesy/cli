import { defineConfig } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const electronE2EDir = path.join(__dirname, 'packages/electron/e2e');

if (!process.env.TS_NODE_PROJECT) {
  process.env.TS_NODE_PROJECT = path.join(__dirname, 'tsconfig.playwright.json');
}

// Never pop the HTML report open in a browser. This is set as an env var rather
// than a reporter option so it still holds when someone passes `--reporter=html`
// on the command line, which bypasses the reporter config below.
process.env.PLAYWRIGHT_HTML_OPEN = 'never';

export default defineConfig({
  testDir: electronE2EDir,
  fullyParallel: false,
  timeout: 120_000,
  expect: {
    timeout: 10_000,
  },
  retries: process.env.CI ? 1 : 0,
  // Locally, stop the entire run at the first failure. These specs launch real
  // Electron windows on the developer's desktop; grinding through the remaining
  // cases after a failure steals the machine and produces no extra information.
  // Fix the first failure, then re-run. CI still runs the full suite.
  maxFailures: process.env.CI ? 0 : 1,
  reporter: process.env.CI
    ? [['list'], ['junit', { outputFile: 'playwright-report/electron-e2e.xml' }]]
    : [['list']],
  use: {
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'electron',
      testDir: electronE2EDir,
    },
  ],
});
