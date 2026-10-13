import { defineConfig, devices } from '@playwright/test';

/** Captures README media (see tests/screenshots). Output goes to screenshots/. */
export default defineConfig({
  testDir: 'tests/screenshots',
  workers: 1,
  reporter: 'list',
  use: { ...devices['Desktop Chrome'], baseURL: 'http://localhost:4173' },
  webServer: {
    command: 'npm run build && npm run preview',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
