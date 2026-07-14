import { expect, test } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('spectrum polygon colors are evaluated by the typed object graph', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/高峻清作品/勾股树开花（gjq）.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const source = window.gspDebug.sourceScene;
    const scene = () => window.gspDebug.runtime.scene;
    const driverIndex = scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === 22,
    );
    const before = scene().polygons.map((polygon: any) => [...polygon.color]);
    const operations = source.objectGraph.nodes
      .filter((node: any) => /^polygon-color:\d+$/.test(node.id))
      .map((node: any) => node.definition.op.kind);

    env.updateScene((draft) => {
      const constraint = draft.points[driverIndex].constraint;
      if (constraint && 't' in constraint) {
        constraint.t = 0.46067897795334134 + 1 / 6;
      }
    }, 'graph');

    return {
      complete: source.objectGraph.geometryComplete,
      pending: source.objectGraph.pendingOperations,
      driverIndex,
      operations,
      before,
      after: scene().polygons.map((polygon: any) => [...polygon.color]),
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.driverIndex).toBeGreaterThanOrEqual(0);
  expect(result.operations).toEqual(Array(5).fill('spectrum-color'));
  expect(result.before[0]).toEqual([0, 255, 255, 127]);
  expect(result.after[0]).toEqual([0, 0, 255, 127]);
  expect(result.after).not.toEqual(result.before);
});
