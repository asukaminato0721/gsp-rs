import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-legacy-runtime-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('line-intersection helper points stay pan-only in the browser runtime', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/热研系列/概率问题/蒲丰投针实验求π的近似值.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const scene = window.gspDebug.runtime.scene;
    const pointIndex = scene.points.findIndex((point: any) => point.constraint?.kind === 'line-intersection');
    if (pointIndex < 0) {
      return null;
    }
    const dragMode = drag.dragModeFor(env, pointIndex, null, null, null, null);
    return { dragMode };
  });

  expect(result).not.toBeNull();
  expect(result?.dragMode).toBe('pan');
});

test('fixed coordinate helper points stay pan-only and follow their graph source in runtime', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/热研系列/概率问题/蒲丰投针实验求π的近似值.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const dynamics = window.GspViewerModules.dynamics;
    const scene = window.gspDebug.runtime.scene;
    const pointIndex = scene.points.findIndex((point: any) => point.binding?.kind === 'coordinate-source-2d');
    if (pointIndex < 0) {
      return null;
    }
    const point = scene.points[pointIndex] as any;
    const sourceIndex = point.binding.sourceIndex as number;
    const before = { x: point.x, y: point.y };
    const dragMode = drag.dragModeFor(env, pointIndex, null, null, null, null);
    env.markDependencyRootsDirty?.([dynamics.sourcePointRootId(sourceIndex)]);
    env.updateScene((draft: any) => {
      draft.points[sourceIndex].x += 0.4;
      draft.points[sourceIndex].y -= 0.3;
    }, 'graph');
    const afterPoint = window.gspDebug.runtime.scene.points[pointIndex] as any;
    return {
      dragMode,
      dx: afterPoint.x - before.x,
      dy: afterPoint.y - before.y,
    };
  });

  expect(result).not.toBeNull();
  expect(result?.dragMode).toBe('pan');
  expect(result?.dx).toBeCloseTo(0.4, 3);
  expect(result?.dy).toBeCloseTo(-0.3, 3);
});

test('angle-referenced rotate points stay live in the browser runtime', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/热研系列/滚动系列/正Ｎ边形真滚1.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const pointIndex = scene.points.findIndex((point: any) =>
      point.binding?.kind === 'derived'
      && point.binding?.transform?.kind === 'rotate'
      && typeof point.binding.transform.angleStartIndex === 'number'
      && typeof point.binding.transform.angleVertexIndex === 'number'
      && typeof point.binding.transform.angleEndIndex === 'number',
    );
    if (pointIndex < 0) {
      return null;
    }
    const point = scene.points[pointIndex] as any;
    return {
      hasAngleRefs:
        typeof point.binding.transform.angleStartIndex === 'number'
        && typeof point.binding.transform.angleVertexIndex === 'number'
        && typeof point.binding.transform.angleEndIndex === 'number',
      draggable: point.draggable,
    };
  });

  expect(result).not.toBeNull();
  expect(result?.hasAngleRefs).toBe(true);
  expect(result?.draggable).toBe(false);
});
