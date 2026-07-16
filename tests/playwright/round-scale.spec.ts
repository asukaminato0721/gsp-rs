import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('scaled circle intersections render', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/圆的伸缩变换.gsp');
  await page.goto(`file://${file}`);

  const circleStrokes = page.locator('circle[stroke^="rgba(0, 128, 0"]');
  await expect(circleStrokes).toHaveCount(3);

  const pointMarkers = page.locator('circle[fill="rgba(255, 0, 0, 1.000)"]');
  await expect(pointMarkers).toHaveCount(5);

  const intersections = await page.evaluate(() => window.gspDebug.runtime.scene.points
    .filter((point: any) => point.constraint?.kind === 'circular-intersection'));
  expect(intersections).toHaveLength(2);
  expect(intersections.every((point: any) => Number.isFinite(point.x) && Number.isFinite(point.y))).toBe(true);
});

test('nested reflected scale circle renders and keeps intersections', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/圆的伸缩变换1.gsp');
  await page.goto(`file://${file}`);
  const sceneData = page.locator('#scene-data');
  await expect(sceneData).toHaveCount(1);
  const scene = JSON.parse(await sceneData.textContent() ?? '{}');
  expect(scene.circles).toHaveLength(3);
  expect(scene.circles.some((circle: any) => circle.binding?.kind === 'matrix-apply'
    && circle.binding.matrixApply.some((matrix: any) => matrix.kind === 'reflect'))).toBe(true);
  expect(scene.circles.some((circle: any) => circle.binding?.kind === 'matrix-apply'
    && circle.binding.matrixApply.some((matrix: any) => matrix.kind === 'scale'))).toBe(true);
  expect(scene.points.some((point: { constraint?: { kind?: string } | null }) =>
    point.constraint?.kind === 'circular-constraint')).toBe(true);
  expect(scene.points.filter((point: { constraint?: { kind?: string } | null }) =>
    point.constraint?.kind === 'circular-intersection')).toHaveLength(2);
});
