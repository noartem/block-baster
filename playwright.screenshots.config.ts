import { defineConfig, devices } from '@playwright/test';

/** Captures README media (see tests/screenshots). Output goes to screenshots/. */
export default defineConfig({
  testDir: 'tests/screenshots',
  workers: 1,
  reporter: 'list',
  use: { ...devices['Desktop Chrome'], baseURL: 'http://localhost:4173' },
  webServer: {
    command: process.env.SKIP_BUILD ? 'npm run serve' : 'npm run build:web && npm run serve',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 600_000,
  },
});
