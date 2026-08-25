const { defineConfig, devices } = require('@playwright/test');

// Keep in sync with `port` in tests/config.test.yml.
const baseURL = 'http://localhost:8181';

module.exports = defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  timeout: 60 * 1000,
  reporter: process.env.CI ? [['html'], ['github']] : 'html',
  use: {
    baseURL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    permissions: ['clipboard-read', 'clipboard-write'],
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: {
    // release build: every test opens the database, and the KDF takes seconds
    // in a debug build
    command: 'cargo run --release -- --config ./tests/config.test.yml',
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    cwd: process.cwd(),
    timeout: 300 * 1000,
    stdout: 'pipe',
    stderr: 'pipe',
    env: {
      RUST_LOG: 'info',
    },
  },
});
