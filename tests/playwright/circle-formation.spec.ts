import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-circle-formation-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('circle formation fixture keeps rebuilt polygon edges and non-draggable iterated points', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/圆的形成.gsp');
  await page.goto(`file://${file}`);

  const runtime = await page.evaluate(() => JSON.parse(window.gspDebug.json()));
  expect(runtime.scene.lines).toHaveLength(5);

  const renderedBlueSegments = page.locator('#scene-layer path[stroke=\"rgba(0, 0, 128, 1.000)\"]');
  await expect(renderedBlueSegments).toHaveCount(5);

  const rotatePoints = runtime.scene.points.filter((point: { binding?: { kind?: string } | null }) =>
    point.binding?.kind === 'rotate',
  );
  expect(rotatePoints.length).toBeGreaterThan(0);
  expect(rotatePoints.every((point: { draggable?: boolean }) => point.draggable === false)).toBe(true);
});
