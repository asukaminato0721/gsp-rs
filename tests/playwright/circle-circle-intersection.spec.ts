import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-circle-circle-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('circle-circle intersections stay distinct after dragging both circle centers', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/gsp/insection/circle_circle_insection.gsp');
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() =>
    window.gspDebug.runtime.scene.points.slice(4, 6).map((point) => ({ x: point.x, y: point.y })),
  );
  expect(before).toHaveLength(2);
  expect(Math.hypot(before[0].x - before[1].x, before[0].y - before[1].y)).toBeGreaterThan(1);

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    for (const pointIndex of [0, 3]) {
      if (typeof rootId === 'function') {
        env.markDependencyRootsDirty(rootId(pointIndex));
      }
    }
    env.updateScene((draft) => {
      draft.points[0].x -= 160;
      draft.points[0].y -= 160;
      draft.points[3].x -= 160;
      draft.points[3].y -= 160;
    }, 'graph');
  });

  const after = await page.evaluate(() =>
    window.gspDebug.runtime.scene.points.slice(4, 6).map((point) => ({
      x: point.x,
      y: point.y,
      visible: point.visible !== false,
    })),
  );
  expect(after).toHaveLength(2);
  expect(after.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y) && point.visible)).toBe(true);
  expect(Math.hypot(after[0].x - after[1].x, after[0].y - after[1].y)).toBeGreaterThan(1);
});
