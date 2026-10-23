import { defineConfig, devices } from '@playwright/test';

/** Browser tests of the web (WebAssembly) build, served from dist/. */
export default defineConfig({
  testDir: 'tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['list']] : 'list',
  use: {
    baseURL: 'http://localhost:4173',
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'mobile', use: { ...devices['Pixel 7'] } },
    { name: 'desktop', use: { ...devices['Desktop Chrome'] } },
  ],
  webServer: {
    // CI builds dist/ in an earlier step and sets SKIP_BUILD.
    command: process.env.SKIP_BUILD ? 'npm run serve' : 'npm run build:web && npm run serve',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 600_000,
  },
});
