import { expect, test } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('point trace samples its typed dependency subgraph', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml('tests/fixtures/gsp/trace.gsp')}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const trace = scene.lines[5];
    return {
      complete: window.gspDebug.sourceScene.objectGraph.geometryComplete,
      pending: window.gspDebug.sourceScene.objectGraph.pendingOperations,
      pointCount: trace.points.length,
      first: trace.points[0],
      last: trace.points.at(-1),
      expectedFirst: {
        x: (scene.points[1].x + scene.points[0].x) / 2,
        y: (scene.points[1].y + scene.points[0].y) / 2,
      },
      expectedLast: {
        x: (scene.points[3].x + scene.points[0].x) / 2,
        y: (scene.points[3].y + scene.points[0].y) / 2,
      },
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.pointCount).toBe(500);
  expect(result.first.x).toBeCloseTo(result.expectedFirst.x, 9);
  expect(result.first.y).toBeCloseTo(result.expectedFirst.y, 9);
  expect(result.last.x).toBeCloseTo(result.expectedLast.x, 9);
  expect(result.last.y).toBeCloseTo(result.expectedLast.y, 9);
});

test('circle family iteration is rebuilt from object graph results', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml('tests/fixtures/未实现/圆系(inRm).gsp')}`);

  const before = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const generated = scene.circles[1];
    return {
      complete: window.gspDebug.sourceScene.objectGraph.geometryComplete,
      pending: window.gspDebug.sourceScene.objectGraph.pendingOperations,
      circleCount: scene.circles.length,
      sourceCircleCount: window.gspDebug.sourceScene.circles.length,
      generatedCenter: generated ? { ...generated.center } : null,
      radius: generated ? Math.hypot(
        generated.radiusPoint.x - generated.center.x,
        generated.radiusPoint.y - generated.center.y,
      ) : null,
    };
  });

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    env.updateScene((draft) => {
      const constraint = draft.points[5].constraint;
      if (constraint && 't' in constraint) constraint.t = 0.5;
    }, 'graph');
  });

  const after = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const generated = scene.circles[1];
    return {
      circleCount: scene.circles.length,
      generatedCenter: generated ? { ...generated.center } : null,
      radius: generated ? Math.hypot(
        generated.radiusPoint.x - generated.center.x,
        generated.radiusPoint.y - generated.center.y,
      ) : null,
    };
  });

  expect(before.complete).toBe(true);
  expect(before.pending).toEqual([]);
  expect(before.circleCount).toBe(21);
  expect(after.circleCount).toBe(21);
  expect(after.radius).toBeCloseTo(before.radius!, 9);
  expect(before.generatedCenter).not.toBeNull();
  expect(after.generatedCenter).not.toBeNull();
  expect(Math.hypot(
    after.generatedCenter!.x - before.generatedCenter!.x,
    after.generatedCenter!.y - before.generatedCenter!.y,
  )).toBeGreaterThan(1);
});

test('parameterized point iteration evaluates its typed point programs', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/李章博作品/动画演示立体几何轨迹形成（李章博）.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const source = window.gspDebug.sourceScene;
    const scene = window.gspDebug.runtime.scene;
    const operations = source.objectGraph.nodes
      .filter((node: any) => node.id.startsWith('point-iteration:'))
      .map((node: any) => ({
        kind: node.definition.op.kind,
        outputId: node.definition.op.program.outputId,
        stateCount: node.definition.op.program.stateSourceIds.length,
      }));
    const generated = scene.points.filter((point: any) => point.debug == null);
    return {
      complete: source.objectGraph.geometryComplete,
      pending: source.objectGraph.pendingOperations,
      operations,
      sourcePointCount: source.points.length,
      runtimePointCount: scene.points.length,
      generatedCount: generated.length,
      allFinite: generated.every((point: any) => (
        Number.isFinite(point.x) && Number.isFinite(point.y)
      )),
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.operations).toHaveLength(3);
  expect(result.operations.every((operation: any) => (
    operation.kind === 'point-iteration'
      && operation.outputId.startsWith('point:')
      && operation.stateCount === 1
  ))).toBe(true);
  expect(result.runtimePointCount).toBeGreaterThan(result.sourcePointCount);
  expect(result.generatedCount).toBeGreaterThanOrEqual(1197);
  expect(result.allFinite).toBe(true);
});
