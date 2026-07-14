import { expect, test } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('coordinate trace and its intersection follow the source point through the object graph', async ({ page }) => {
  const file = compileFixtureToTempHtml(
    'tests/fixtures/gsp/insection/cood_intersection.gsp',
  );
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    return {
      complete: window.gspDebug.sourceScene.objectGraph.geometryComplete,
      pending: window.gspDebug.sourceScene.objectGraph.pendingOperations,
      sourceX: scene.points[3].x,
      coordinateX: scene.points[4].x,
      intersectionX: scene.points[5].x,
      traceFirstX: scene.lines[2].points[0].x,
      traceLastX: scene.lines[2].points.at(-1)!.x,
    };
  });
  expect(before.complete).toBe(true);
  expect(before.pending).toEqual([]);

  const dx = 2.25;
  await page.evaluate((deltaX) => {
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(3));
    }
    env.updateScene((draft) => {
      draft.points[3].x += deltaX;
    }, 'graph');
  }, dx);

  const after = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    return {
      sourceX: scene.points[3].x,
      coordinateX: scene.points[4].x,
      intersectionX: scene.points[5].x,
      traceFirstX: scene.lines[2].points[0].x,
      traceLastX: scene.lines[2].points.at(-1)!.x,
    };
  });

  for (const key of [
    'sourceX',
    'coordinateX',
    'intersectionX',
    'traceFirstX',
    'traceLastX',
  ] as const) {
    expect(after[key] - before[key]).toBeCloseTo(dx, 9);
  }
});
