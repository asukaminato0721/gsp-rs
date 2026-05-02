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

test('triangle angle sum measured-angle rotation updates dependent geometry', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/未分类档/三角形内角和定理.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = (ordinal: number) =>
      scene().points.findIndex((point: any) => point.debug?.groupOrdinal === ordinal);
    const lineByOrdinal = (ordinal: number) =>
      scene().lines.find((line: any) => line.debug?.groupOrdinal === ordinal);

    const cIndex = pointIndex(18);
    const rotatedIndex = pointIndex(31);
    const intersectionIndex = pointIndex(46);
    if (cIndex < 0 || rotatedIndex < 0 || intersectionIndex < 0) {
      return null;
    }
    const before = {
      rotated: { x: scene().points[rotatedIndex].x, y: scene().points[rotatedIndex].y },
      intersection: { x: scene().points[intersectionIndex].x, y: scene().points[intersectionIndex].y },
      marker: lineByOrdinal(38)?.points?.map((point: any) => ({ x: point.x, y: point.y })) ?? [],
      rotatedBinding: scene().points[rotatedIndex].binding,
      intersectionConstraint: scene().points[intersectionIndex].constraint,
    };

    env.markDependencyRootsDirty?.([dynamics.sourcePointRootId(cIndex)]);
    env.updateScene((draft: any) => {
      draft.points[cIndex].x += 40;
      draft.points[cIndex].y -= 10;
    }, 'graph');

    const after = {
      rotated: { x: scene().points[rotatedIndex].x, y: scene().points[rotatedIndex].y },
      intersection: { x: scene().points[intersectionIndex].x, y: scene().points[intersectionIndex].y },
      marker: lineByOrdinal(38)?.points?.map((point: any) => ({ x: point.x, y: point.y })) ?? [],
    };
    const moved = (left: { x: number, y: number }, right: { x: number, y: number }) =>
      Math.hypot(left.x - right.x, left.y - right.y);
    return {
      hasMeasuredAngleRotate:
        before.rotatedBinding?.kind === 'derived'
        && before.rotatedBinding?.transform?.kind === 'rotate'
        && typeof before.rotatedBinding.transform.angleStartIndex === 'number'
        && typeof before.rotatedBinding.transform.angleVertexIndex === 'number'
        && typeof before.rotatedBinding.transform.angleEndIndex === 'number',
      hasMeasuredRadiusIntersection:
        before.intersectionConstraint?.kind === 'line-circular-intersection'
        && before.intersectionConstraint?.circle?.kind === 'segment-radius-circle',
      rotatedDelta: moved(before.rotated, after.rotated),
      intersectionDelta: moved(before.intersection, after.intersection),
      markerDelta: Math.max(
        0,
        ...before.marker.map((point: { x: number, y: number }, index: number) => {
          const afterPoint = after.marker[index];
          return afterPoint ? moved(point, afterPoint) : 0;
        }),
      ),
    };
  });

  expect(result).not.toBeNull();
  expect(result?.hasMeasuredAngleRotate).toBe(true);
  expect(result?.hasMeasuredRadiusIntersection).toBe(true);
  expect(result?.rotatedDelta).toBeGreaterThan(1);
  expect(result?.intersectionDelta).toBeGreaterThan(1);
  expect(result?.markerDelta).toBeGreaterThan(1);
});

test('triangle angle sum translated H handle drags source H and drives animation', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/未分类档/三角形内角和定理.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = (ordinal: number) =>
      scene().points.findIndex((point: any) => point.debug?.groupOrdinal === ordinal);
    const labelByOrdinal = (ordinal: number) =>
      scene().labels.find((label: any) => label.debug?.groupOrdinal === ordinal);
    const segmentProjection = (binding: any) => {
      const point = scene().points[binding.pointIndex];
      const start = scene().points[binding.startIndex];
      const end = scene().points[binding.endIndex];
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const lenSq = dx * dx + dy * dy;
      const value = ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSq;
      return Math.max(0, Math.min(1, value));
    };

    const hIndex = pointIndex(11);
    const translatedHIndex = pointIndex(40);
    const lIndex = pointIndex(21);
    const pIndex = pointIndex(28);
    const rotatedIndex = pointIndex(31);
    if ([hIndex, translatedHIndex, lIndex, pIndex, rotatedIndex].some((index) => index < 0)) {
      return null;
    }

    const before = {
      h: { x: scene().points[hIndex].x, y: scene().points[hIndex].y },
      translatedH: { x: scene().points[translatedHIndex].x, y: scene().points[translatedHIndex].y },
      l: { x: scene().points[lIndex].x, y: scene().points[lIndex].y },
      p: { x: scene().points[pIndex].x, y: scene().points[pIndex].y },
      rotated: { x: scene().points[rotatedIndex].x, y: scene().points[rotatedIndex].y },
      offset: { ...scene().points[translatedHIndex].constraint },
      dragMode: drag.dragModeFor(env, translatedHIndex, null, null, null, null),
      values: {
        s12: segmentProjection(labelByOrdinal(12).binding),
        s23: segmentProjection(labelByOrdinal(13).binding),
        s01: segmentProjection(labelByOrdinal(14).binding),
      },
    };

    drag.beginDrag(env, 13, env.toScreen(scene().points[translatedHIndex]), translatedHIndex, null, null, null, null);
    drag.updateDraggedPoint(env, {
      x: before.translatedH.x + 400,
      y: before.translatedH.y,
    });
    env.dragState.val = null;

    const after = {
      h: { x: scene().points[hIndex].x, y: scene().points[hIndex].y },
      translatedH: { x: scene().points[translatedHIndex].x, y: scene().points[translatedHIndex].y },
      l: { x: scene().points[lIndex].x, y: scene().points[lIndex].y },
      p: { x: scene().points[pIndex].x, y: scene().points[pIndex].y },
      rotated: { x: scene().points[rotatedIndex].x, y: scene().points[rotatedIndex].y },
      offset: { ...scene().points[translatedHIndex].constraint },
      values: {
        s12: segmentProjection(labelByOrdinal(12).binding),
        s23: segmentProjection(labelByOrdinal(13).binding),
        s01: segmentProjection(labelByOrdinal(14).binding),
      },
    };
    const moved = (left: { x: number, y: number }, right: { x: number, y: number }) =>
      Math.hypot(left.x - right.x, left.y - right.y);

    return {
      dragMode: before.dragMode,
      hDelta: moved(before.h, after.h),
      translatedHDelta: moved(before.translatedH, after.translatedH),
      lDelta: moved(before.l, after.l),
      pDelta: moved(before.p, after.p),
      rotatedDelta: moved(before.rotated, after.rotated),
      offsetDxDelta: Math.abs((after.offset as any).dx - (before.offset as any).dx),
      offsetDyDelta: Math.abs((after.offset as any).dy - (before.offset as any).dy),
      beforeValues: before.values,
      afterValues: after.values,
    };
  });

  expect(result).not.toBeNull();
  expect(result?.dragMode).toBe('point');
  expect(result?.hDelta).toBeGreaterThan(90);
  expect(result?.translatedHDelta).toBeGreaterThan(90);
  expect(result?.lDelta).toBeGreaterThan(10);
  expect(result?.pDelta).toBeGreaterThan(10);
  expect(result?.rotatedDelta).toBeGreaterThan(10);
  expect(result?.beforeValues).toEqual({ s12: 0, s23: 0, s01: 0 });
  expect(result?.afterValues.s01).toBeCloseTo(1, 6);
  expect(result?.afterValues.s12).toBeCloseTo(1, 6);
  expect(result?.afterValues.s23).toBeGreaterThan(0);
  expect(result?.afterValues.s23).toBeLessThan(1);
  expect(result?.offsetDxDelta).toBeLessThan(1e-6);
  expect(result?.offsetDyDelta).toBeLessThan(1e-6);
});

test('hejixu fold2 marked-ratio dilation stays live when E reaches C', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/贺基旭作品/翻折2(hjx4882).gsp');
  await page.goto(`file://${file}`);

  const dragPoints = await page.evaluate(() => {
    const scene = () => window.gspDebug.runtime.scene;
    const pointByGroup = (ordinal: number) =>
      scene().points[scene().points.findIndex((point: any) => point.debug?.groupOrdinal === ordinal)];
    const env = window.gspDebug.viewerEnv;
    const beforeE = pointByGroup(7);
    const pointC = pointByGroup(3);
    const rect = env.canvas.getBoundingClientRect();
    const clientPoint = (point: any) => {
      const screen = env.toScreen(point);
      return {
        x: rect.left + screen.x * rect.width / env.sourceScene.width,
        y: rect.top + screen.y * rect.height / env.sourceScene.height,
      };
    };

    return {
      start: clientPoint(beforeE),
      end: clientPoint(pointC),
    };
  });

  await page.mouse.move(dragPoints.start.x, dragPoints.start.y);
  await page.mouse.down();
  await page.mouse.move(dragPoints.end.x, dragPoints.end.y, { steps: 20 });
  await page.mouse.up();

  const result = await page.evaluate(() => {
    const scene = () => window.gspDebug.runtime.scene;
    const pointByGroup = (ordinal: number) =>
      scene().points[scene().points.findIndex((point: any) => point.debug?.groupOrdinal === ordinal)];
    const reflectAcrossLine = (source: any, lineStart: any, lineEnd: any) => {
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const lenSq = dx * dx + dy * dy;
      const t = ((source.x - lineStart.x) * dx + (source.y - lineStart.y) * dy) / lenSq;
      const projection = {
        x: lineStart.x + t * dx,
        y: lineStart.y + t * dy,
      };
      return {
        x: 2 * projection.x - source.x,
        y: 2 * projection.y - source.y,
      };
    };

    const pointA = pointByGroup(1);
    const pointB = pointByGroup(2);
    const pointD = pointByGroup(4);
    const pointE = pointByGroup(7);
    const pointF = pointByGroup(9);
    const pointG = pointByGroup(11);
    const pointH = pointByGroup(12);
    const pointC = pointByGroup(3);
    const denominator = Math.hypot(pointD.x - pointB.x, pointD.y - pointB.y);
    const rawRatio = Math.hypot(pointE.x - pointB.x, pointE.y - pointB.y) / denominator;
    const ratio = Math.min(rawRatio, 1);
    const expectedF = {
      x: pointB.x + (pointD.x - pointB.x) * ratio,
      y: pointB.y + (pointD.y - pointB.y) * ratio,
    };
    const expectedG = reflectAcrossLine(pointB, pointA, pointE);
    const expectedH = reflectAcrossLine(pointF, pointA, pointE);
    const folded = scene().polygons.find((polygon: any) => polygon.debug?.groupOrdinal === 13);
    const ratioLabel = scene().labels.find((label: any) => label.debug?.groupOrdinal === 8);
    const dragState = window.gspDebug.viewerEnv.dragState.val;

    return {
      eAtC: Math.hypot(pointE.x - pointC.x, pointE.y - pointC.y),
      rawRatio,
      ratio,
      fErr: Math.hypot(pointF.x - expectedF.x, pointF.y - expectedF.y),
      fAtD: Math.hypot(pointF.x - pointD.x, pointF.y - pointD.y),
      gErr: Math.hypot(pointG.x - expectedG.x, pointG.y - expectedG.y),
      hErr: Math.hypot(pointH.x - expectedH.x, pointH.y - expectedH.y),
      pointF,
      pointG,
      pointH,
      foldedBinding: folded?.binding,
      foldedPoints: folded?.points,
      ratioLabel,
      dragState,
    };
  });

  expect(result.eAtC).toBeLessThan(1e-3);
  expect(result.rawRatio).toBeGreaterThan(1);
  expect(result.ratio).toBe(1);
  expect(result.fErr).toBeLessThan(1);
  expect(result.fAtD).toBeLessThan(1);
  expect(result.gErr).toBeLessThan(1);
  expect(result.hErr).toBeLessThan(1);
  expect(result.foldedBinding?.kind).toBe('point-polygon');
  expect(result.foldedBinding?.vertexIndices).toEqual([0, 6, 7, 4]);
  expect(result.foldedPoints).toHaveLength(4);
  expect(result.ratioLabel?.text).toBe('(BE/BD) = 1');
  expect(result.ratioLabel?.richMarkup).toContain('</<H');
  expect(result.ratioLabel?.richMarkup).toContain('<TxBE>');
  expect(result.ratioLabel?.richMarkup).toContain('<TxBD>');
  expect(result.ratioLabel?.richMarkup).toContain('<Tx = 1>');
  expect(result.ratioLabel?.visible).toBe(true);
  expect(result.ratioLabel?.anchor.x).toBeLessThanOrEqual(20);
  expect(result.ratioLabel?.anchor.y).toBeLessThanOrEqual(60);
  expect(result.dragState).toBeNull();
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
      visibleArcOrdinals: scene.arcs
        .filter((arc: any) => arc.visible !== false)
        .map((arc: any) => arc.debug?.groupOrdinal),
      labelTexts: scene.labels.map((label: any) => label.text),
      angleMarkerClasses: scene.lines
        .filter((line: any) => line.binding?.kind === 'angle-marker')
        .map((line: any) => line.binding.markerClass),
      pointsByOrdinal: Object.fromEntries(
        scene.points
          .filter((point: any) => [6, 18, 20, 24].includes(point.debug?.groupOrdinal))
          .map((point: any) => [
            point.debug.groupOrdinal,
            {
              x: point.x,
              y: point.y,
              bindingKind: point.binding?.transform?.kind ?? null,
              factor: point.binding?.transform?.factor ?? null,
            },
          ]),
      ),
      crossing: (() => {
        const env = window.gspDebug.viewerEnv;
        const dynamics = window.GspViewerModules.dynamics;
        const startScene = window.gspDebug.runtime.scene;
        const a = startScene.points[0];
        const c = startScene.points[1];
        const cAngle = Math.atan2(-(c.y - a.y), c.x - a.x);
        const cParameter = ((cAngle % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2) / (Math.PI * 2);
        const moveD = (value: number) => {
          env.markDependencyRootsDirty?.([dynamics.sourcePointRootId(4)]);
          env.updateScene((draft: any) => {
            dynamics.applyNormalizedParameterToPoint(draft.points[4], draft, value);
          }, 'graph');
        };
        const lineCircleVariant = () => {
          const current = window.gspDebug.runtime.scene;
          const lineStart = current.points[1];
          const lineEnd = current.points[4];
          const center = current.points[6];
          const radiusPoint = current.points[4];
          const dx = lineEnd.x - lineStart.x;
          const dy = lineEnd.y - lineStart.y;
          const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
          const fx = lineStart.x - center.x;
          const fy = lineStart.y - center.y;
          const aCoef = dx * dx + dy * dy;
          const bCoef = 2 * (fx * dx + fy * dy);
          const cCoef = fx * fx + fy * fy - radius * radius;
          const root = Math.sqrt(Math.max(0, bCoef * bCoef - 4 * aCoef * cCoef));
          const candidates = [(-bCoef - root) / (2 * aCoef), (-bCoef + root) / (2 * aCoef)]
            .map((t) => ({ x: lineStart.x + dx * t, y: lineStart.y + dy * t }));
          return candidates[1];
        };
        moveD(cParameter - 0.0005);
        moveD(cParameter);
        moveD(cParameter + 0.0005);
        return {
          actual: window.gspDebug.runtime.scene.points[8],
          expected: lineCircleVariant(),
        };
      })(),
      bLabelAnchorKind:
        bLabel?.binding?.kind === 'point-anchor' || (bLabel?.anchor && 'pointIndex' in bLabel.anchor)
          ? 'point'
          : 'other',
    };
  });

  expect(Math.max(...result.angleMarkerClasses)).toBeLessThanOrEqual(2);
  expect(result.pathCount).toBe(19);
  expect(result.redPointCount).toBe(6);
  expect(result.visibleArcOrdinals).toEqual([]);
  expect(result.labelTexts).toContain('⇒△CBD∼△CEB');
  expect(result.labelTexts).toContain('⇒(BC^2)=CD*CE');
  expect(result.pointsByOrdinal[6]?.bindingKind).toBe('scale');
  expect(result.pointsByOrdinal[6]?.factor).toBeCloseTo(Math.sqrt(6), 12);
  expect(result.pointsByOrdinal[6]?.x).toBeCloseTo(943.2872383623309, 6);
  expect(result.pointsByOrdinal[6]?.y).toBeCloseTo(309.37346662472527, 6);
  expect(result.pointsByOrdinal[18]?.x).toBeCloseTo(538, 6);
  expect(result.pointsByOrdinal[18]?.y).toBeCloseTo(428, 6);
  expect(result.pointsByOrdinal[20]?.x).toBeCloseTo(714.459085867259, 6);
  expect(result.pointsByOrdinal[20]?.y).toBeCloseTo(647.5843289845315, 6);
  expect(result.pointsByOrdinal[24]?.x).toBeCloseTo(486, 6);
  expect(result.pointsByOrdinal[24]?.y).toBeCloseTo(551, 6);
  expect(result.crossing.actual.x).toBeCloseTo(result.crossing.expected.x, 6);
  expect(result.crossing.actual.y).toBeCloseTo(result.crossing.expected.y, 6);
  expect(result.bLabelAnchorKind).toBe('point');
});

test('three-moving-point fixture keeps measured rotation and move buttons live', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/bug/三动点最小值_20260419_123930.gsp');
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const moveButtons = scene.buttons.filter((button: any) => button.action?.kind === 'move-point');
    return {
      cPoint: scene.points.find((point: any) => point.debug?.groupOrdinal === 4),
      parameter: scene.parameters.find((parameter: any) => parameter.name === 't₁'),
      moveButtonOrdinals: moveButtons.map((button: any) => button.debug?.groupOrdinal),
      measurementTexts: scene.labels
        .filter((label: any) => [15, 31, 34, 35, 36, 44, 45, 46].includes(label.debug?.groupOrdinal))
        .map((label: any) => label.text),
      movingPoints: moveButtons.map((button: any) => {
        const point = scene.points[button.action.pointIndex];
        return { x: point.x, y: point.y };
      }),
    };
  });

  expect(before.parameter?.value).toBeCloseTo(60, 6);
  expect(before.parameter?.unit).toBe('degree');
  expect(before.cPoint?.binding?.transform?.kind).toBe('rotate');
  expect(before.cPoint?.binding?.transform?.parameterName).toBe('t₁');
  expect(before.cPoint?.x).toBeCloseTo(673.4418668596801, 6);
  expect(before.cPoint?.y).toBeCloseTo(608.7880361833353, 6);
  expect(before.moveButtonOrdinals).toEqual([39, 42, 43]);
  expect(before.measurementTexts).toHaveLength(8);
  expect(before.measurementTexts).toEqual(expect.arrayContaining([
    expect.stringMatching(/^∠BAC = /),
    expect.stringMatching(/^△ABC的面积 = /),
    expect.stringMatching(/^MN = /),
    expect.stringMatching(/^AM = /),
    expect.stringMatching(/^AN = /),
    expect.stringMatching(/^AE = /),
    expect.stringMatching(/^BC = /),
    expect.stringMatching(/^AE \/ BC\*3 = /),
  ]));

  const buttons = page.getByRole('button', { name: '移动点' });
  await expect(buttons).toHaveCount(3);
  for (let index = 0; index < 3; index += 1) {
    await buttons.nth(index).click();
    await page.waitForTimeout(220);
  }

  const after = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    return {
      movingPoints: scene.buttons
        .filter((button: any) => button.action?.kind === 'move-point')
        .map((button: any) => {
          const point = scene.points[button.action.pointIndex];
          return { x: point.x, y: point.y };
        }),
      measurementTexts: scene.labels
        .filter((label: any) => [15, 31, 34, 35, 36, 44, 45, 46].includes(label.debug?.groupOrdinal))
        .map((label: any) => label.text),
    };
  });

  for (let index = 0; index < 3; index += 1) {
    const dx = after.movingPoints[index].x - before.movingPoints[index].x;
    const dy = after.movingPoints[index].y - before.movingPoints[index].y;
    expect(Math.hypot(dx, dy)).toBeGreaterThan(1);
  }
  expect(after.measurementTexts).not.toEqual(before.measurementTexts);
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
