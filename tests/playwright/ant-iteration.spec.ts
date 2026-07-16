import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('ant fixture rebuilds iterated lines without duplicating exported payload lines', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/迭代方法2(蚂蚁).gsp');
  await page.goto(`file://${file}`);

  const countsAtLoad = await page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    return {
      sourceLines: window.gspDebug.viewerEnv.sourceScene.lines.length,
      currentLines: runtime.scene.lines.length,
      visibleLines: runtime.scene.lines.filter((line: any) => line.visible !== false).length,
    };
  });
  expect(countsAtLoad.sourceLines).toBe(4);
  expect(countsAtLoad.currentLines).toBe(100);
  expect(countsAtLoad.visibleLines).toBe(96);

  await page.locator('#parameter-controls input').evaluate((element) => {
    const input = element as HTMLInputElement;
    input.value = '4';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });

  const countsAtFour = await page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    return {
      currentLines: runtime.scene.lines.length,
      visibleLines: runtime.scene.lines.filter((line: any) => line.visible !== false).length,
      n: runtime.dynamics.parameters[0]?.value,
    };
  });
  expect(countsAtFour.n).toBe(4);
  expect(countsAtFour.currentLines).toBe(164);
  expect(countsAtFour.visibleLines).toBe(160);
});
