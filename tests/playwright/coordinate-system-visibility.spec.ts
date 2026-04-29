import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-coordinate-system-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('calibration-only geometry fixture hides the coordinate system', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/20260421角平分线的作用.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => ({
    graphMode: window.gspDebug.viewerEnv.sourceScene.graphMode,
    gridChildren: document.querySelector('#grid-layer')?.childElementCount ?? -1,
    sceneLineCount: window.gspDebug.runtime.scene.lines.length,
  }));

  expect(result.graphMode).toBe(false);
  expect(result.gridChildren).toBe(0);
  expect(result.sceneLineCount).toBeGreaterThan(0);
});
