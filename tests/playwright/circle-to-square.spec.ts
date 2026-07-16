import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('circle-to-square sectors follow the E1 payload control', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/李章博作品/割圆为方（李章博）.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = (ordinal: number) => scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === ordinal,
    );
    const snapshot = () => {
      const current = scene();
      const centerIndex = pointIndex(21);
      const labelText = (ordinal: number) => current.labels.find(
        (label: any) => label.debug?.groupOrdinal === ordinal,
      )?.text;
      return {
        objectGraphComplete: window.gspDebug.sourceScene.objectGraph.geometryComplete,
        objectGraphPending: window.gspDebug.sourceScene.objectGraph.pendingOperations,
        polygonCount: current.polygons.length,
        firstIteratedPoint: { ...current.polygons[4].points[0] },
        center: { ...current.points[centerIndex] },
        axisX: current.points[pointIndex(1)].x,
        parameterTexts: {
          a: labelText(16),
          b: labelText(15),
          c: labelText(14),
        },
        baseColors: current.polygons.slice(0, 4).map((polygon: any) => polygon.color),
        iterationColors: [4, 13, 22, 31].map(
          (start) => current.polygons[start].color,
        ),
        allFinite: current.polygons.every((polygon: any) => polygon.points.every(
          (point: any) => Number.isFinite(point.x) && Number.isFinite(point.y),
        )),
        iterationBounds: [4, 13, 22, 31].map((start) => {
          const polygons = current.polygons.slice(start, start + 9);
          const points = polygons.flatMap((polygon: any) => polygon.points);
          return {
            minX: Math.min(...points.map((point: any) => point.x)),
            maxX: Math.max(...points.map((point: any) => point.x)),
            minY: Math.min(...points.map((point: any) => point.y)),
            maxY: Math.max(...points.map((point: any) => point.y)),
          };
        }),
      };
    };

    const driverIndex = pointIndex(13);
    const driver = scene().points[driverIndex];
    const segmentStart = scene().points[pointIndex(5)];
    const segmentEnd = scene().points[pointIndex(6)];
    const before = snapshot();
    const dragMode = drag.dragModeFor(env, driverIndex, null, null, null, null);
    drag.beginDrag(env, 1, env.toScreen(driver), driverIndex, null, null, null, null);
    drag.updateDraggedPoint(env, {
      x: segmentStart.x + 0.4 * 0.58 * (segmentEnd.x - segmentStart.x),
      y: segmentStart.y,
    });
    env.dragState.val = null;
    const after = snapshot();
    return { driverIndex, dragMode, before, after };
  });

  expect(result.driverIndex).toBeGreaterThanOrEqual(0);
  expect(result.dragMode).toBe('point');
  expect(result.before.objectGraphComplete).toBe(true);
  expect(result.before.objectGraphPending).toEqual([]);
  expect(result.before.polygonCount).toBe(40);
  expect(result.after.polygonCount).toBe(40);
  expect(result.before.allFinite).toBe(true);
  expect(result.after.allFinite).toBe(true);
  expect(result.after.parameterTexts.a).toContain('0.58');
  expect(result.after.parameterTexts.b).toMatch(/b\s*=\s*0(?:\.0+)?$/);
  expect(result.after.parameterTexts.c).toMatch(/c\s*=\s*0(?:\.0+)?$/);
  expect(result.after.baseColors).toEqual([
    [255, 0, 0, 127],
    [0, 128, 0, 127],
    [0, 128, 0, 127],
    [255, 0, 0, 127],
  ]);
  expect(result.after.iterationColors).toEqual([
    [0, 128, 0, 127],
    [0, 128, 0, 127],
    [255, 0, 0, 127],
    [255, 0, 0, 127],
  ]);
  const [initialLowerA, initialLowerB, initialUpperA, initialUpperB] =
    result.before.iterationBounds;
  expect([initialLowerA, initialLowerB].some(
    (bounds) => (bounds.minX + bounds.maxX) / 2 < result.before.axisX,
  )).toBe(true);
  expect([initialLowerA, initialLowerB].some(
    (bounds) => (bounds.minX + bounds.maxX) / 2 > result.before.axisX,
  )).toBe(true);
  expect([initialUpperA, initialUpperB].some(
    (bounds) => bounds.maxX <= result.before.axisX + 1e-6,
  )).toBe(true);
  expect([initialUpperA, initialUpperB].some(
    (bounds) => bounds.minX >= result.before.axisX - 1e-6,
  )).toBe(true);
  expect(result.after.iterationBounds.filter(
    (bounds) => (bounds.minX + bounds.maxX) / 2 < result.after.axisX,
  )).toHaveLength(2);
  expect(result.after.iterationBounds.filter(
    (bounds) => (bounds.minX + bounds.maxX) / 2 > result.after.axisX,
  )).toHaveLength(2);
  expect(Math.hypot(
    result.after.center.x - result.before.center.x,
    result.after.center.y - result.before.center.y,
  )).toBeGreaterThan(1);
  expect(Math.hypot(
    result.after.firstIteratedPoint.x - result.before.firstIteratedPoint.x,
    result.after.firstIteratedPoint.y - result.before.firstIteratedPoint.y,
  )).toBeGreaterThan(1);
});
