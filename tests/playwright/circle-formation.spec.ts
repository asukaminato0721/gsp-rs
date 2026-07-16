import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('circle formation fixture keeps rebuilt polygon edges and non-draggable iterated points', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/圆的形成.gsp');
  await page.goto(`file://${file}`);

  const runtime = await page.evaluate(() => JSON.parse(window.gspDebug.json()));
  expect(runtime.scene.lines).toHaveLength(5);
  expect(runtime.scene.lines.map((line: any) => line.debug?.groupOrdinal).filter(Boolean)).not.toContain(26);

  const renderedBlueSegments = page.locator('#scene-layer path[stroke=\"rgba(0, 0, 128, 1.000)\"]');
  await expect(renderedBlueSegments).toHaveCount(4);
  expect(runtime.scene.lines.filter((line: any) => line.visible === false)).toHaveLength(1);

  const generatedPoints = runtime.scene.points.filter((point: { debug?: unknown }) => point.debug == null);
  expect(generatedPoints.length).toBeGreaterThan(0);
  expect(generatedPoints.every((point: { draggable?: boolean }) => point.draggable === false)).toBe(true);

  const table = runtime.scene.iterationTables[0];
  expect(table.rows.map((row: any) => row.values[0])).toEqual([6, 7, 8, 9, 10]);
  const angleLabel = runtime.scene.labels.find((label: any) => label.debug?.groupOrdinal === 8);
  expect(angleLabel?.text).toBe('2*180 / t₂ = 72.00°');
  expect(angleLabel?.binding?.exprLabel).toBe('2*180 / t₂');

  await page.locator('input[type=number]').first().fill('6');
  await expect.poll(async () =>
    page.evaluate(() => window.gspDebug.runtime.scene.iterationTables[0]?.rows.map((row: any) => row.values[0])),
  ).toEqual([7, 8, 9, 10, 11, 12]);
});
