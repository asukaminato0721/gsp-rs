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
  execFileSync(path.resolve(repoRoot, 'target/debug/gsp-rs'), ['--html', tempFixturePath], {
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
    ...(() => {
      const env = window.gspDebug.viewerEnv;
      const drag = window.GspViewerModules.drag;
      const scene = window.gspDebug.runtime.scene;
      const aIndex = scene.points.findIndex((point) => point.debug?.groupOrdinal === 1);
      const pointA = scene.points[aIndex];
      const oIndex = scene.points.findIndex((point) => point.debug?.groupOrdinal === 8);
      const eIndex = scene.points.findIndex((point) => point.debug?.groupOrdinal === 10);
      const pointO = scene.points[oIndex];
      const beforeA = { ...env.resolveScenePoint(aIndex) };
      const beforeO = { ...env.resolveScenePoint(oIndex) };
      const beforeE = { ...env.resolveScenePoint(eIndex) };
      const dragMode = drag.dragModeFor(env, aIndex, null, null, null, null);
      if (beforeA) {
        drag.beginDrag(env, 1, env.toScreen(beforeA), aIndex, null, null, null, null);
        drag.updateDraggedPoint(env, { x: beforeA.x - 40, y: beforeA.y + 35 });
      }
      const afterA = env.resolveScenePoint(aIndex);
      const afterO = env.resolveScenePoint(oIndex);
      const afterE = env.resolveScenePoint(eIndex);
      return {
        pointAVisible: pointA?.visible === true,
        pointODraggable: pointO?.draggable === true,
        pointOBinding: pointO?.binding?.kind ?? null,
        pointADragMode: dragMode,
        pointAMoved: beforeA && afterA ? Math.hypot(afterA.x - beforeA.x, afterA.y - beforeA.y) : 0,
        pointOMoved: beforeO && afterO ? Math.hypot(afterO.x - beforeO.x, afterO.y - beforeO.y) : 0,
        pointEMoved: beforeE && afterE ? Math.hypot(afterE.x - beforeE.x, afterE.y - beforeE.y) : 0,
        visibleArcOrdinals: scene.arcs
          .filter((arc) => arc.visible)
          .map((arc) => arc.debug?.groupOrdinal)
          .filter((ordinal) => typeof ordinal === 'number'),
      };
    })(),
  }));

  expect(result.graphMode).toBe(false);
  expect(result.gridChildren).toBe(0);
  expect(result.sceneLineCount).toBeGreaterThan(0);
  expect(result.pointAVisible).toBe(true);
  expect(result.pointODraggable).toBe(false);
  expect(result.pointOBinding).toBe('circumcenter');
  expect(result.pointADragMode).toBe('point');
  expect(result.pointAMoved).toBeGreaterThan(20);
  expect(result.pointOMoved).toBeGreaterThan(1);
  expect(result.pointEMoved).toBeGreaterThan(1);
  expect(result.visibleArcOrdinals).toEqual(expect.arrayContaining([6, 9]));
});

test('rolling triangle fixture keeps reference geometry coordinates without graph grid', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/方小庆作品/三角形滚动(inRm).gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    const beforeRatioText = scene.labels.find((label) => label.binding?.kind === 'point-distance-ratio-value')?.text;
    const beforeExpressionText = scene.labels.find((label) => label.binding?.kind === 'expression-value')?.text;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(2));
    }
    env.updateScene((draft) => {
      draft.points[2].constraint.t = 2.4;
      draft.points[2].x = 55 + (180.46194225721783 - 55) * 2.4;
      draft.points[2].y = 364;
    }, 'graph');
    const afterScene = window.gspDebug.runtime.scene;

    return {
      graphMode: window.gspDebug.viewerEnv.sourceScene.graphMode,
      gridChildren: document.querySelector('#grid-layer')?.childElementCount ?? -1,
      unitPoint: { x: scene.points[1].x, y: scene.points[1].y },
      rayPoint: { x: afterScene.points[2].x, y: afterScene.points[2].y },
      lineCount: scene.lines.length,
      hasRatioMeasurement: afterScene.labels.some((label) => label.visible
        && label.binding?.kind === 'point-distance-ratio-value'),
      parameterMeasurementCount: afterScene.labels.filter((label) => label.binding?.kind === 'segment-projection-parameter').length,
      hasTriangle: afterScene.polygons.some((polygon) => polygon.binding?.kind === 'point-polygon'),
      hasTranslatedPolygonPoint: afterScene.points.some((point) => point.constraint?.kind === 'translated-polygon-boundary'),
      hasPointTrace: afterScene.lines.some((line) => line.binding?.kind === 'point-trace' && line.points.length > 20),
      pointTrace: (() => {
        const line = afterScene.lines.find((line) => line.binding?.kind === 'point-trace');
        const ys = line?.points.map((point) => point.y) ?? [];
        return line ? {
          pointIndex: line.binding.pointIndex,
          driverIndex: line.binding.driverIndex,
          targetConstraint: afterScene.points[line.binding.pointIndex]?.constraint?.kind,
          driverConstraint: afterScene.points[line.binding.driverIndex]?.constraint?.kind,
          minY: Math.min(...ys),
          maxY: Math.max(...ys),
        } : null;
      })(),
      beforeRatioText,
      afterRatioText: afterScene.labels.find((label) => label.binding?.kind === 'point-distance-ratio-value')?.text,
      beforeExpressionText,
      afterExpressionText: afterScene.labels.find((label) => label.binding?.kind === 'expression-value')?.text,
    };
  });

  expect(result.graphMode).toBe(false);
  expect(result.gridChildren).toBe(0);
  expect(result.unitPoint.x).toBeCloseTo(180.461942, 3);
  expect(result.unitPoint.y).toBeCloseTo(364, 3);
  expect(result.rayPoint.x).toBeCloseTo(356.108661, 3);
  expect(result.lineCount).toBeGreaterThanOrEqual(6);
  expect(result.hasRatioMeasurement).toBe(true);
  expect(result.parameterMeasurementCount).toBeGreaterThanOrEqual(3);
  expect(result.hasTriangle).toBe(true);
  expect(result.hasTranslatedPolygonPoint).toBe(true);
  expect(result.hasPointTrace).toBe(true);
  expect(result.pointTrace?.pointIndex).not.toBe(result.pointTrace?.driverIndex);
  expect(result.pointTrace?.targetConstraint).toBe('translated-polygon-boundary');
  expect(result.pointTrace?.driverConstraint).toBe('ray');
  expect((result.pointTrace?.maxY ?? 0) - (result.pointTrace?.minY ?? 0)).toBeGreaterThan(50);
  expect(result.afterRatioText).not.toBe(result.beforeRatioText);
  expect(result.afterExpressionText).not.toBe(result.beforeExpressionText);
});

test('rolling triangle fixture scales marked-angle helpers and starts at the left triangle', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/方小庆作品/三角形滚动(inRm).gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(2));
    }
    const origin = scene.points[0];
    const denominator = scene.points[7];
    const unitPoint = scene.points[1];
    const targetX = origin.x;
    env.updateScene((draft) => {
      draft.points[2].constraint.t = (targetX - origin.x) / (unitPoint.x - origin.x);
      draft.points[2].x = targetX;
      draft.points[2].y = origin.y;
    }, 'graph');
    const afterScene = window.gspDebug.runtime.scene;
    const movingTriangle = afterScene.polygons.find((polygon) => polygon.binding?.kind === 'derived');
    const scaleControlledHelpers = afterScene.points.filter((point) => {
      const transform = point.binding?.transform;
      return transform?.kind === 'scale'
        && typeof transform.factorParameterPointIndex === 'number'
        && typeof transform.factorParameterStartIndex === 'number'
        && typeof transform.factorParameterEndIndex === 'number';
    });

    return {
      scaleControlledHelperCount: scaleControlledHelpers.length,
      movingTrianglePoints: movingTriangle?.points ?? [],
    };
  });

  expect(result.scaleControlledHelperCount).toBeGreaterThanOrEqual(3);
  expect(result.movingTrianglePoints[0].x).toBeCloseTo(39, 1);
  expect(result.movingTrianglePoints[0].y).toBeCloseTo(291, 1);
  expect(result.movingTrianglePoints[1].x).toBeCloseTo(180.461942, 1);
  expect(result.movingTrianglePoints[1].y).toBeCloseTo(364, 1);
  expect(result.movingTrianglePoints[2].x).toBeCloseTo(55, 1);
  expect(result.movingTrianglePoints[2].y).toBeCloseTo(364, 1);
});

test('rolling triangle fixture is already synced before dragging M', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/方小庆作品/三角形滚动(inRm).gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const movingTrianglePoints = () => window.gspDebug.runtime.scene.polygons
      .find((polygon) => polygon.binding?.kind === 'derived')
      ?.points.map((point) => ({ x: point.x, y: point.y })) ?? [];
    const before = movingTrianglePoints();
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(2));
    }
    env.updateScene((draft) => {
      draft.points[2].x = scene.points[2].x;
      draft.points[2].y = scene.points[2].y;
    }, 'graph');
    return { before, after: movingTrianglePoints() };
  });

  expect(result.before.length).toBe(3);
  expect(result.after.length).toBe(3);
  result.before.forEach((point, index) => {
    expect(point.x).toBeCloseTo(result.after[index].x, 6);
    expect(point.y).toBeCloseTo(result.after[index].y, 6);
  });
});
