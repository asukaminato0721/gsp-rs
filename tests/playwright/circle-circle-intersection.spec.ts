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
    const runtime = window.GspRuntimeCore;
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
      circleByReference: runtime.choosePointCandidate(
        runtime.circleCircleIntersections(
          circleArgs[0],
          2,
          circleArgs[2],
          2,
        ),
        { x: 1, y: 2 },
        0,
      ),
      circleInvalidVariant: runtime.choosePointCandidate(
        runtime.circleCircleIntersections(circleArgs[0], 2, circleArgs[2], 2),
        null,
        99,
      ),
      lineByReference: runtime.lineCircleIntersectionCandidate(
        lineArgs[0], lineArgs[1], lineArgs[2], lineArgs[3], 2, 1,
      ),
      lineInvalidVariant: runtime.lineCircleIntersectionCandidate(
        lineArgs[0], lineArgs[1], lineArgs[2], lineArgs[3], 2, 99,
      ),
    };
  });

  expect(selected.circleByReference).not.toBeNull();
  expect(selected.circleByReference!.y).toBeGreaterThan(0);
  expect(selected.circleInvalidVariant).toBeNull();
  expect(selected.lineByReference).toEqual({ x: 2, y: 0 });
  expect(selected.lineInvalidVariant).toBeNull();
});
