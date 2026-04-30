import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-two-circle-fill-'));
  const tempFixturePath = path.join(tempDir, 'fixture.gsp');
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('two circle intersection fill renders as intersection, not union', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/未实现/(inRm)两圆之交.gsp');
  await page.goto(`file://${file}`);

  const initial = await page.evaluate(() => ({
    filledCircleCount: Array.from(document.querySelectorAll('circle[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none' && Number(element.getAttribute('r')) > 1)
      .length,
    filledPathCount: Array.from(document.querySelectorAll('path[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none')
      .length,
  }));
  expect(initial).toEqual({ filledCircleCount: 1, filledPathCount: 0 });

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    env.updateScene((draft) => {
      draft.circles[3].radiusPoint = { x: 320, y: 182 };
    });
  });

  const intersecting = await page.evaluate(() => ({
    filledCircleCount: Array.from(document.querySelectorAll('circle[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none' && Number(element.getAttribute('r')) > 1)
      .length,
    filledPathCount: Array.from(document.querySelectorAll('path[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none')
      .length,
    filledCircleRadii: Array.from(document.querySelectorAll('circle[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none')
      .map((element) => Number(element.getAttribute('r'))),
  }));
  expect(intersecting.filledCircleCount).toBe(0);
  expect(intersecting.filledPathCount).toBe(1);
  expect(intersecting.filledCircleRadii).toEqual([]);
});
