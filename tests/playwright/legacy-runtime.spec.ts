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
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
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

test('angle-marker class payload renders bug fixture without path explosion', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/测试10.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const svg = document.querySelector('svg#view');
    const scene = window.gspDebug.runtime.scene;
    const bLabel = scene.labels.find((label: any) => label.text === 'B');
    return {
      pathCount: svg?.querySelectorAll('path').length ?? 0,
      redPointCount: svg?.querySelectorAll('circle[fill="rgba(255, 0, 0, 1.000)"]').length ?? 0,
      angleMarkerClasses: scene.lines
        .filter((line: any) => line.binding?.kind === 'angle-marker')
        .map((line: any) => line.binding.markerClass),
      bLabelAnchorKind:
        bLabel?.binding?.kind === 'point-anchor' || (bLabel?.anchor && 'pointIndex' in bLabel.anchor)
          ? 'point'
          : 'other',
    };
  });

  expect(Math.max(...result.angleMarkerClasses)).toBeLessThanOrEqual(2);
  expect(result.pathCount).toBe(20);
  expect(result.redPointCount).toBe(6);
  expect(result.bLabelAnchorKind).toBe('point');
});

test('one dragon fixture preserves JavaSketchpad visibility and clickable sequence action', async ({ page }) => {
  const fixturePath = 'tests/Samples/个人专栏/李章博作品/一条龙.gsp';
  test.skip(!fs.existsSync(path.resolve(process.cwd(), fixturePath)), 'sample fixture missing');
  const file = compileFixtureToTempHtml(fixturePath);
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const visibleButtons = Array.from(document.querySelectorAll('.scene-link-button'))
      .map((button) => button.textContent || '');
    const hiddenChildOrdinals = new Set([10, 11, 12, 14, 19, 21, 26, 28]);
    return {
      visiblePointCount: scene.points.filter((point: any) => point.visible).length,
      redDomPointCount: document.querySelectorAll('circle[fill="rgba(255, 0, 0, 1.000)"]').length,
      visibleButtons,
      hiddenChildrenStayHidden: scene.buttons
        .filter((button: any) => hiddenChildOrdinals.has(button.debug?.groupOrdinal))
        .every((button: any) => button.visible === false),
      animatedPointX: scene.points[9].x,
      animatedPointY: scene.points[9].y,
      dragonLineX: scene.lines.find((line: any) => line.debug?.groupOrdinal === 32)?.points[1]?.x,
      dragonLineY: scene.lines.find((line: any) => line.debug?.groupOrdinal === 32)?.points[1]?.y,
    };
  });

  expect(before.visiblePointCount).toBe(2);
  expect(before.redDomPointCount).toBe(2);
  expect(before.visibleButtons).toEqual(['系列2 个动作', 'http://exjh.com']);
  expect(before.hiddenChildrenStayHidden).toBe(true);

  await page.getByRole('button', { name: '系列2 个动作' }).click();
  await page.waitForTimeout(1200);

  const after = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    return {
      animatedPointX: scene.points[9].x,
      animatedPointY: scene.points[9].y,
      dragonLineX: scene.lines.find((line: any) => line.debug?.groupOrdinal === 32)?.points[1]?.x,
      dragonLineY: scene.lines.find((line: any) => line.debug?.groupOrdinal === 32)?.points[1]?.y,
    };
  });

  expect(Math.hypot(
    after.animatedPointX - before.animatedPointX,
    after.animatedPointY - before.animatedPointY,
  )).toBeGreaterThan(1);
  expect(Math.hypot(
    (after.dragonLineX ?? 0) - (before.dragonLineX ?? 0),
    (after.dragonLineY ?? 0) - (before.dragonLineY ?? 0),
  )).toBeGreaterThan(1);
});

test('Lizhangbo solid-geometry trace label buttons drive hidden parameters', async ({ page }) => {
  const fixturePath = 'tests/Samples/个人专栏/李章博作品/动画演示立体几何轨迹形成（李章博）.gsp';
  test.skip(!fs.existsSync(path.resolve(process.cwd(), fixturePath)), 'sample fixture missing');
  const file = compileFixtureToTempHtml(fixturePath);
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => {
    const dynamics = window.gspDebug.runtime.dynamics;
    const scene = window.gspDebug.runtime.scene;
    const generatedDepth = scene.pointIterations
      .filter((family: any) => family.kind === 'parameterized')
      .reduce((sum: number, family: any) => sum + (family.depth || 0), 0);
    const standaloneParameters = scene.points
      .filter((point: any) => point?.binding?.kind === 'parameter' && !point.constraint)
      .length;
    const generated = scene.points.slice(
      Math.max(0, scene.points.length - standaloneParameters - generatedDepth),
      Math.max(0, scene.points.length - standaloneParameters),
    );
    const xs = generated.map((point: any) => point.x);
    const ys = generated.map((point: any) => point.y);
    return {
      t7: dynamics.parameters.find((parameter: any) => parameter.name === 't[7]')?.value,
      buttons: scene.buttons.map((button: any) => button.action.kind),
      pointCount: scene.points.length,
      pointIterations: scene.pointIterations.map((family: any) => family.kind),
      lineIterations: scene.lineIterations.length,
      generatedTraceCount: generated.length,
      generatedTraceWidth: Math.max(...xs) - Math.min(...xs),
      generatedTraceHeight: Math.max(...ys) - Math.min(...ys),
    };
  });

  expect(before.t7).toBe(399);
  expect(before.buttons).toContain('set-parameter');
  expect(before.buttons).toContain('animate-parameter');
  expect(before.pointIterations).toContain('parameterized');
  expect(before.lineIterations).toBe(0);
  expect(before.generatedTraceCount).toBe(798);
  expect(before.generatedTraceWidth).toBeGreaterThan(80);
  expect(before.generatedTraceHeight).toBeGreaterThan(80);

  await page.getByRole('button', { name: '初 始 化' }).click();
  const reset = await page.evaluate(() => {
    const dynamics = window.gspDebug.runtime.dynamics;
    const scene = window.gspDebug.runtime.scene;
    return {
      t7: dynamics.parameters.find((parameter: any) => parameter.name === 't[7]')?.value,
      pointCount: scene.points.length,
    };
  });

  expect(reset.t7).toBe(0);
  expect(reset.pointCount).toBeLessThan(before.pointCount);

  await page.getByRole('button', { name: '轨迹生成' }).click();
  await page.waitForTimeout(500);
  const animated = await page.evaluate(() => {
    const dynamics = window.gspDebug.runtime.dynamics;
    const scene = window.gspDebug.runtime.scene;
    return {
      t7: dynamics.parameters.find((parameter: any) => parameter.name === 't[7]')?.value,
      pointCount: scene.points.length,
    };
  });

  expect(animated.t7).toBeGreaterThan(0);
  expect(animated.pointCount).toBeGreaterThan(reset.pointCount);
});
