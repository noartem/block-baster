import { defineConfig, devices } from '@playwright/test';

/** WebGL 2 on machines without a GPU (CI) runs on SwiftShader; CHROMIUM_PATH points at a preinstalled browser. */
const launchOptions = {
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  executablePath: process.env.CHROMIUM_PATH || undefined,
};

/** Captures README media (see tests/screenshots). Output goes to screenshots/. */
export default defineConfig({
  testDir: 'tests/screenshots',
  workers: 1,
  reporter: 'list',
  use: { ...devices['Desktop Chrome'], baseURL: 'http://localhost:4173', launchOptions },
  webServer: {
    command: process.env.SKIP_BUILD ? 'npm run serve' : 'npm run build:web && npm run serve',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 600_000,
  },
});
