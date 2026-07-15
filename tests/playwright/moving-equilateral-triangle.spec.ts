import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('moving equilateral triangle follows E through its payload parameter chain', async ({ page }) => {
  const file = compileFixtureToTempHtml(
    'tests/Samples/个人专栏/侯仰顺作品/参数的应用-正三角形在正方形内滑动【蚂蚁制作】.gsp',
  );
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const read = () => JSON.parse(window.gspDebug.json());
    const before = read();
    const pointIndex = (scene: any, groupOrdinal: number) => scene.scene.points.findIndex(
      (point: any) => point.debug?.groupOrdinal === groupOrdinal,
    );
    const line = (scene: any, groupOrdinal: number) => scene.scene.lines.find(
      (candidate: any) => candidate.debug?.groupOrdinal === groupOrdinal,
    );
    const eIndex = pointIndex(before, 10);
    const fIndex = pointIndex(before, 13);
    const gIndex = pointIndex(before, 15);
    const e = before.scene.points[eIndex];

    drag.beginDrag(env, 1, { x: e.x, y: e.y }, eIndex, null, null, null, null);
    drag.updateDraggedPoint(env, { x: e.x + 35, y: e.y });

    const after = read();
    const distance = (left: any, right: any) => Math.hypot(left.x - right.x, left.y - right.y);
    const triangleLengths = (scene: any) => {
      const currentE = scene.scene.points[eIndex];
      const currentF = scene.scene.points[fIndex];
      const currentG = scene.scene.points[gIndex];
      return [
        distance(currentE, currentF),
        distance(currentF, currentG),
        distance(currentG, currentE),
      ];
    };
    const trace = line(after, 18);
    return {
      background: after.scene.backgroundColor,
      canvasBackground: getComputedStyle(document.querySelector('#view') as Element).backgroundColor,
      indices: [eIndex, fIndex, gIndex],
      visible: [eIndex, fIndex, gIndex].map((index) => after.scene.points[index].visible),
      bindings: [
        after.scene.points[fIndex].binding?.kind,
        after.scene.points[gIndex].binding?.kind,
      ],
      beforePositions: [eIndex, fIndex, gIndex].map((index) => before.scene.points[index]),
      afterPositions: [eIndex, fIndex, gIndex].map((index) => after.scene.points[index]),
      beforeLengths: triangleLengths(before),
      afterLengths: triangleLengths(after),
      edgeBindings: [14, 16, 17].map((ordinal) => line(after, ordinal)?.binding?.kind),
      traceBinding: trace?.binding?.kind,
      tracePointCount: trace?.points?.length,
    };
  });

  expect(result.background).toEqual([255, 255, 255, 255]);
  expect(result.canvasBackground).toBe('rgb(255, 255, 255)');
  expect(result.indices.every((index) => index >= 0)).toBe(true);
  expect(result.visible).toEqual([true, true, true]);
  expect(result.bindings).toEqual(['constraint-parameter-from-point-expr', 'derived']);
  expect(result.edgeBindings).toEqual(['segment', 'segment', 'segment']);
  expect(result.traceBinding).toBe('point-trace');
  expect(result.tracePointCount).toBeGreaterThan(20);

  for (let index = 0; index < 3; index += 1) {
    const before = result.beforePositions[index];
    const after = result.afterPositions[index];
    expect(Math.hypot(after.x - before.x, after.y - before.y)).toBeGreaterThan(1);
  }
  for (const lengths of [result.beforeLengths, result.afterLengths]) {
    expect(Math.max(...lengths) - Math.min(...lengths)).toBeLessThan(0.01);
  }
  expect(
    Math.abs(result.afterLengths[0] - result.beforeLengths[0]),
    JSON.stringify({
      beforePositions: result.beforePositions,
      afterPositions: result.afterPositions,
      beforeLengths: result.beforeLengths,
      afterLengths: result.afterLengths,
    }),
  ).toBeLessThan(0.01);
});
