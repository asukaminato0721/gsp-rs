import { test, expect } from '@playwright/test';
import path from 'node:path';

test('ant fixture rebuilds iterated lines without duplicating exported payload lines', async ({ page }) => {
  const file = path.resolve('tests/fixtures/bug/迭代方法2(蚂蚁).html');
  await page.goto(`file://${file}`);

  const countsAtLoad = await page.evaluate(() => {
    const source = JSON.parse(document.getElementById('scene-data')!.textContent!);
    const runtime = JSON.parse(window.gspDebug.json());
    return {
      sourceLines: source.lines.length,
      currentLines: runtime.scene.lines.length,
    };
  });
  expect(countsAtLoad.currentLines).toBe(countsAtLoad.sourceLines);

  await page.locator('#parameter-controls input').evaluate((element) => {
    const input = element as HTMLInputElement;
    input.value = '4';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });

  const countsAtFour = await page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    return {
      currentLines: runtime.scene.lines.length,
      n: runtime.dynamics.parameters[0]?.value,
    };
  });
  expect(countsAtFour.n).toBe(4);
  expect(countsAtFour.currentLines).toBe(160);
});
