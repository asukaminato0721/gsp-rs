import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-round-scale-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('scaled circle intersections render', async ({ page }) => {
  const file = path.resolve('tests/fixtures/bug/圆的伸缩变换.html');
  await page.goto(`file://${file}`);

  const circleStrokes = page.locator('circle[stroke^="rgba(0, 128, 0"]');
  await expect(circleStrokes).toHaveCount(3);

  const pointMarkers = page.locator('circle[fill="rgba(255, 0, 0, 1.000)"]');
  await expect(pointMarkers).toHaveCount(5);

  const leftIntersections = await pointMarkers.evaluateAll((nodes) =>
    nodes
      .map((node) => ({
        cx: Number(node.getAttribute('cx')),
        cy: Number(node.getAttribute('cy')),
      }))
      .filter((point) => point.cx < 300),
  );
  expect(leftIntersections).toHaveLength(2);
});

test('nested reflected scale circle renders and keeps intersections', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/圆的伸缩变换1.gsp');
  await page.goto(`file://${file}`);
  const sceneData = page.locator('#scene-data');
  await expect(sceneData).toHaveCount(1);
  const scene = JSON.parse(await sceneData.textContent() ?? '{}');
  expect(scene.circles).toHaveLength(3);
  expect(scene.circles.some((circle: { binding?: { kind?: string; transform?: { kind?: string } | null } | null }) =>
    circle.binding?.kind === 'derived' && circle.binding?.transform?.kind === 'reflect')).toBe(true);
  expect(scene.circles.some((circle: { binding?: { kind?: string; transform?: { kind?: string } | null } | null }) =>
    circle.binding?.kind === 'derived' && circle.binding?.transform?.kind === 'scale')).toBe(true);
  expect(scene.points.some((point: { constraint?: { kind?: string } | null }) =>
    point.constraint?.kind === 'circular-constraint')).toBe(true);
  expect(scene.points.filter((point: { constraint?: { kind?: string } | null }) =>
    point.constraint?.kind === 'circular-intersection')).toHaveLength(2);
});
