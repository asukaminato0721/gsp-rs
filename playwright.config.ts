import { defineConfig } from '@playwright/test';

export default defineConfig({
testDir: 'tests/playwright',
globalSetup: './tests/playwright/global-setup.ts',
use: {
  browserName: 'chromium',
  headless: true,
},
});
