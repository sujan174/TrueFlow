/* eslint-disable @typescript-eslint/no-require-imports */
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

async function run() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 }
  });

  const outDir = path.join(__dirname, 'screenshots');

  const payload = Buffer.from('{"email":"admin@trueflow.io","name":"Admin","picture":"","exp":9999999999}').toString('base64url');

  await context.addCookies([
    {
      name: 'trueflow_session',
      value: payload,
      domain: 'localhost',
      path: '/',
      httpOnly: true,
      sameSite: 'Lax'
    },
    {
      name: 'trueflow_user',
      value: btoa(JSON.stringify({email: 'admin@trueflow.io', name: 'Admin', picture: ''})),
      domain: 'localhost',
      path: '/',
      httpOnly: false,
      sameSite: 'Lax'
    },
    {
      name: 'dashboard_token',
      value: 'dev_secret',
      domain: 'localhost',
      path: '/',
      httpOnly: true,
      sameSite: 'Strict'
    }
  ]);

  const page = await context.newPage();

  const routes = [
    { url: '/', name: '01_dashboard_home.png' },
    { url: '/virtual-keys', name: '03_virtual_keys.png' },
    { url: '/policies', name: '04_policies.png' },
    { url: '/prompts', name: '05_prompts.png' },
    { url: '/audit', name: '06_audit.png' },
    { url: '/api-keys', name: '07_api_keys.png' },
    { url: '/analytics', name: '08_analytics.png' }
  ];

  let firstPage = true;

  for (const route of routes) {
    try {
      console.log(`Navigating to http://localhost:3000${route.url}`);
      await page.goto(`http://localhost:3000${route.url}`, { waitUntil: 'domcontentloaded', timeout: 30000 });
      await page.waitForTimeout(2000);

      if (firstPage) {
        // Try to close the tour modal if it exists
        try {
          const skipButton = page.locator('button:has-text("Skip tour")');
          if (await skipButton.isVisible()) {
            await skipButton.click();
            await page.waitForTimeout(500); // Wait for modal to disappear
          }
        } catch (e) {
          console.log('No skip button found or failed to click');
        }
        firstPage = false;
      }

      await page.screenshot({ path: path.join(outDir, route.name) });
      console.log(`Saved screenshot: ${route.name}`);
    } catch (e) {
      console.error(`Failed to screenshot ${route.url}:`, e.message);
    }
  }

  await browser.close();
}

run();
