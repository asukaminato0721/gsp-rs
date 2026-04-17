import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-segment-circle-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('segment-circle intersection stays interactive and preserves segment semantics', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/gsp/insection/circle_insection.gsp');
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => {
    const runtime = window.gspDebug.runtime;
    const intersection = runtime.scene.points[4];
    return {
      intersectionX: intersection.x,
      intersectionY: intersection.y,
      lineKind: intersection.constraint?.line?.kind ?? null,
    };
  });
  expect(before.lineKind).toBe('segment');

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    const pointIndex = 1;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(pointIndex));
    }
    env.updateScene((draft) => {
      draft.points[pointIndex].x -= 140;
      draft.points[pointIndex].y -= 70;
    }, 'graph');
  });

  const after = await page.evaluate(() => {
    const runtime = window.gspDebug.runtime;
    const start = runtime.scene.points[0];
    const end = runtime.scene.points[1];
    const intersection = runtime.scene.points[4];
    return {
      startX: start.x,
      startY: start.y,
      endX: end.x,
      endY: end.y,
      intersectionX: intersection.x,
      intersectionY: intersection.y,
      lineKind: intersection.constraint?.line?.kind ?? null,
    };
  });

  expect(after.lineKind).toBe('segment');
  expect(after.intersectionX).not.toBeCloseTo(before.intersectionX, 6);
  expect(after.intersectionY).not.toBeCloseTo(before.intersectionY, 6);
  expect(after.intersectionX).toBeGreaterThanOrEqual(Math.min(after.startX, after.endX) - 1e-6);
  expect(after.intersectionX).toBeLessThanOrEqual(Math.max(after.startX, after.endX) + 1e-6);
  expect(after.intersectionY).toBeGreaterThanOrEqual(Math.min(after.startY, after.endY) - 1e-6);
  expect(after.intersectionY).toBeLessThanOrEqual(Math.max(after.startY, after.endY) + 1e-6);
});
