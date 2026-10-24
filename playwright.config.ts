import { defineConfig, devices } from '@playwright/test';

/** WebGL 2 on machines without a GPU (CI) runs on SwiftShader; CHROMIUM_PATH points at a preinstalled browser. */
const launchOptions = {
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  executablePath: process.env.CHROMIUM_PATH || undefined,
};

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
    launchOptions,
  },
  projects: [
    { name: 'mobile', use: { ...devices['Pixel 7'], launchOptions } },
    { name: 'desktop', use: { ...devices['Desktop Chrome'], launchOptions } },
  ],
  webServer: {
    // CI builds dist/ in an earlier step and sets SKIP_BUILD.
    command: process.env.SKIP_BUILD ? 'npm run serve' : 'npm run build:web && npm run serve',
    url: 'http://localhost:4173',
    reuseExistingServer: !process.env.CI,
    timeout: 600_000,
  },
});
