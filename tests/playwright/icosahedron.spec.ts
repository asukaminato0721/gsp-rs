import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-icosahedron-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('icosahedron projection rotates when dragging point A', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/向忠作品/正二十面体.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const pointAIndex = 6;
    const projectedPointIndex = 14;
    const parametersBefore = dynamics.parameterMapForScene(env, scene);
    const before = {
      theta: parametersBefore.get('θ[Ov]'),
      phi: parametersBefore.get('φ[Ov]'),
      projected: {
        x: scene.points[projectedPointIndex].x,
        y: scene.points[projectedPointIndex].y,
      },
    };

    return {
      before,
      graphMode: window.gspDebug.sourceScene.graphMode,
      savedViewport: window.gspDebug.sourceScene.savedViewport,
      yUp: window.gspDebug.sourceScene.yUp,
      gridChildren: document.getElementById('grid-layer')?.childElementCount ?? -1,
      blackPanelCount: (window.gspDebug.sourceScene.polygons || [])
        .filter((polygon) => polygon.visible !== false
          && polygon.debug?.groupOrdinal === 41
          && polygon.binding === null
          && polygon.color?.[0] === 0
          && polygon.color?.[1] === 0
          && polygon.color?.[2] === 0
          && polygon.color?.[3] === 255)
        .length,
      controls: (window.gspDebug.sourceScene.parameters || [])
        .filter((parameter) => parameter.visible !== false)
        .map((parameter) => [parameter.name, parameter.value]),
    };
  });

  expect(result.graphMode).toBe(false);
  expect(result.savedViewport).toBe(true);
  expect(result.yUp).toBe(true);
  expect(result.gridChildren).toBe(0);
  expect(result.blackPanelCount).toBeGreaterThanOrEqual(1);
  const controls = new Map(result.controls);
  for (const [name, value] of [
    ['r', 8],
    ['H₁', 1],
    ['H₂', 0.85],
    ['H₃', 0.65],
    ['H₄', 0.45],
    ['H[5]', 0.25],
  ] as const) {
    expect(controls.get(name)).toBeCloseTo(value, 8);
  }

  const pointA = page.locator('circle[fill="rgba(255, 255, 255, 1.000)"]').first();
  const box = await pointA.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  await page.mouse.down();
  await page.mouse.move((box?.x ?? 0) + 58, (box?.y ?? 0) - 36, { steps: 12 });
  await page.mouse.up();

  const after = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const pointAIndex = 6;
    const projectedPointIndex = 14;
    const parametersAfter = dynamics.parameterMapForScene(env, scene);
    return {
      theta: parametersAfter.get('θ[Ov]'),
      phi: parametersAfter.get('φ[Ov]'),
      pointA: {
        x: scene.points[pointAIndex].x,
        y: scene.points[pointAIndex].y,
      },
      projected: {
        x: scene.points[projectedPointIndex].x,
        y: scene.points[projectedPointIndex].y,
      },
    };
  });

  expect(result.before.theta).toBeCloseTo(1.1178336373, 6);
  expect(result.before.phi).toBeCloseTo(0.2400153305, 6);
  expect(after.theta).toBeCloseTo(after.pointA.x, 6);
  expect(after.phi).toBeCloseTo(after.pointA.y, 6);
  expect(after.theta).not.toBeCloseTo(result.before.theta, 6);
  expect(after.phi).not.toBeCloseTo(result.before.phi, 6);
  expect(Math.hypot(
    after.projected.x - result.before.projected.x,
    after.projected.y - result.before.projected.y,
  )).toBeGreaterThan(0.05);
});
