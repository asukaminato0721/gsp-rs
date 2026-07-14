import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('circle-circle intersections stay distinct after dragging both circle centers', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/gsp/insection/circle_circle_insection.gsp');
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => ({
    objectGraphComplete: window.gspDebug.viewerEnv.sourceScene.objectGraph.geometryComplete,
    points: window.gspDebug.runtime.scene.points
      .slice(4, 6)
      .map((point) => ({ x: point.x, y: point.y })),
  }));
  expect(before.objectGraphComplete).toBe(true);
  expect(before.points).toHaveLength(2);
  expect(Math.hypot(
    before.points[0].x - before.points[1].x,
    before.points[0].y - before.points[1].y,
  )).toBeGreaterThan(1);

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    for (let step = 0; step < 16; step += 1) {
      for (const pointIndex of [0, 3]) {
        if (typeof rootId === 'function') {
          env.markDependencyRootsDirty(rootId(pointIndex));
        }
      }
      env.updateScene((draft) => {
        draft.points[0].x -= 10;
        draft.points[0].y -= 10;
        draft.points[3].x -= 10;
        draft.points[3].y -= 10;
      }, 'graph');
    }
  });

  const after = await page.evaluate(() =>
    window.gspDebug.runtime.scene.points.slice(4, 6).map((point) => ({
      x: point.x,
      y: point.y,
      visible: point.visible !== false,
    })),
  );
  expect(after).toHaveLength(2);
  expect(after.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y) && point.visible)).toBe(true);
  expect(Math.hypot(after[0].x - after[1].x, after[0].y - after[1].y)).toBeGreaterThan(1);
});

test('intersection branch selection follows the reference and rejects invalid variants', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/gsp/insection/circle_circle_insection.gsp');
  await page.goto(`file://${file}`);

  const selected = await page.evaluate(() => {
    const scene = window.GspViewerModules.scene!;
    const circleArgs = [
      { x: 0, y: 0 },
      { x: 2, y: 0 },
      { x: 2, y: 0 },
      { x: 4, y: 0 },
    ] as const;
    const lineArgs = [
      { x: -3, y: 0 },
      { x: 3, y: 0 },
      'line' as const,
      { x: 0, y: 0 },
      { x: 2, y: 0 },
    ] as const;
    return {
      circleByReference: scene.circleCircleIntersection(
        ...circleArgs,
        0,
        { x: 1, y: 2 },
      ),
      circleInvalidVariant: scene.circleCircleIntersection(...circleArgs, 99, null),
      lineByReference: scene.lineCircleIntersection(
        ...lineArgs,
        1,
        { x: 2.1, y: 0 },
      ),
      lineInvalidVariant: scene.lineCircleIntersection(...lineArgs, 99, null),
    };
  });

  expect(selected.circleByReference).not.toBeNull();
  expect(selected.circleByReference!.y).toBeGreaterThan(0);
  expect(selected.circleInvalidVariant).toBeNull();
  expect(selected.lineByReference).toEqual({ x: 2, y: 0 });
  expect(selected.lineInvalidVariant).toBeNull();
});
