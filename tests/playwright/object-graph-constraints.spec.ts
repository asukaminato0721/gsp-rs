import { expect, test } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('a payload angle anchor and reflected arc load as a complete object graph', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/周维波作品/角平分线的尺规作图（雪山飞狐）.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const source = window.gspDebug.sourceScene;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = (ordinal: number) => scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === ordinal,
    );
    const arcIndex = (ordinal: number) => scene().arcs.findIndex(
      (arc: any) => arc.debug?.groupOrdinal === ordinal,
    );
    const controlledIndex = pointIndex(71);
    const reflectedArcIndex = arcIndex(61);
    const resultArcIndex = arcIndex(72);
    return {
      complete: source.objectGraph.geometryComplete,
      pending: source.objectGraph.pendingOperations,
      controlledIndex,
      reflectedArcIndex,
      resultArcIndex,
      controlledOp: source.objectGraph.nodes.find(
        (node: any) => node.id === `point:${controlledIndex}`,
      )?.definition.op.kind,
      reflectedArcOp: source.objectGraph.nodes.find(
        (node: any) => node.id === `arc:${reflectedArcIndex}`,
      )?.definition.op.kind,
      controlled: { ...scene().points[controlledIndex] },
      reflectedStart: { ...scene().arcs[reflectedArcIndex].points[0] },
      resultEnd: { ...scene().arcs[resultArcIndex].points[2] },
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.controlledIndex).toBeGreaterThanOrEqual(0);
  expect(result.reflectedArcIndex).toBeGreaterThanOrEqual(0);
  expect(result.resultArcIndex).toBeGreaterThanOrEqual(0);
  expect(result.controlledOp).toBe('point-on-arc');
  expect(result.reflectedArcOp).toBe('reflect-shape-across-line');
  expect(result.controlled.x).toBeCloseTo(result.reflectedStart.x, 6);
  expect(result.controlled.y).toBeCloseTo(result.reflectedStart.y, 6);
  expect(result.resultEnd.x).toBeCloseTo(result.controlled.x, 6);
  expect(result.resultEnd.y).toBeCloseTo(result.controlled.y, 6);
});

test('unnamed payload anchors retain exact parameter-controlled point dependencies', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/钟科作品/正N边形内滚动（颗粒）.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const source = window.gspDebug.sourceScene;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = (ordinal: number) => scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === ordinal,
    );
    const anchorIndex = pointIndex(13);
    const targetIndex = pointIndex(40);
    return {
      pending: source.objectGraph.pendingOperations,
      anchorIndex,
      targetIndex,
      targetOp: source.objectGraph.nodes.find((node: any) => node.id === `point:${targetIndex}`)
        ?.definition.op.kind,
      scalarOp: source.objectGraph.nodes.find(
        (node: any) => node.id === `scalar:point:${targetIndex}:constraint-parameter`,
      )?.definition.op.kind,
      scalarParents: source.objectGraph.nodes.find(
        (node: any) => node.id === `scalar:point:${targetIndex}:constraint-parameter`,
      )?.definition.parents,
    };
  });

  expect(result.pending.every((pending: string) => !pending.startsWith('graph-validation:'))).toBe(true);
  expect(result.anchorIndex).toBeGreaterThanOrEqual(0);
  expect(result.targetIndex).toBeGreaterThanOrEqual(0);
  expect(result.targetOp).toBe('point-on-line');
  expect(result.scalarOp).toBe('evaluate-expression');
  expect(result.scalarParents.some((parent: string) => parent.endsWith(':source:1'))).toBe(true);
});

test('a point on a rotated ray keeps its center arc live', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/孟令岩作品/投骰子模拟试验（1）（石岩）.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const source = window.gspDebug.sourceScene;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === 340,
    );
    const arcIndex = scene().arcs.findIndex(
      (arc: any) => arc.debug?.groupOrdinal === 347,
    );
    const before = scene().arcs[arcIndex].points.map((point: any) => ({ ...point }));

    env.updateScene((draft) => {
      const constraint = draft.points[pointIndex].constraint;
      if (constraint && 't' in constraint) constraint.t += 3;
    }, 'graph');

    return {
      complete: source.objectGraph.geometryComplete,
      pending: source.objectGraph.pendingOperations,
      pointIndex,
      arcIndex,
      pointOp: source.objectGraph.nodes.find((node: any) => node.id === `point:${pointIndex}`)
        ?.definition.op.kind,
      domainOp: source.objectGraph.nodes.find(
        (node: any) => node.id === `domain:point:${pointIndex}`,
      )?.definition.op.kind,
      before,
      after: scene().arcs[arcIndex].points.map((point: any) => ({ ...point })),
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.pointIndex).toBeGreaterThanOrEqual(0);
  expect(result.arcIndex).toBeGreaterThanOrEqual(0);
  expect(result.pointOp).toBe('point-on-line');
  expect(result.domainOp).toBe('rotate-shape-degrees');
  expect(Math.hypot(
    result.after[0].x - result.before[0].x,
    result.after[0].y - result.before[0].y,
  )).toBeGreaterThan(1);
});

test('a hidden offset anchor keeps its parameter-rotated arc live', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/孟令岩作品/认识π.gsp',
  )}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const source = window.gspDebug.sourceScene;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === 50,
    );
    const arcIndex = scene().arcs.findIndex(
      (arc: any) => arc.debug?.groupOrdinal === 56,
    );
    const before = scene().arcs[arcIndex].points.map((point: any) => ({ ...point }));

    env.updateScene((draft) => {
      const constraint = draft.points[pointIndex].constraint;
      if (constraint && 't' in constraint) constraint.t = 0.75;
    }, 'graph');

    return {
      complete: source.objectGraph.geometryComplete,
      pending: source.objectGraph.pendingOperations,
      pointIndex,
      arcIndex,
      before,
      after: scene().arcs[arcIndex].points.map((point: any) => ({ ...point })),
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.pointIndex).toBeGreaterThanOrEqual(0);
  expect(result.arcIndex).toBeGreaterThanOrEqual(0);
  expect(Math.hypot(
    result.after[0].x - result.before[0].x,
    result.after[0].y - result.before[0].y,
  )).toBeGreaterThan(1);
});

test('a function point drives both sides of the point and segment trace construction', async ({ page }) => {
  await page.goto(`file://${compileFixtureToTempHtml(
    'tests/Samples/个人专栏/贺基旭作品/y=x^2的轴对称性(hjx4882).gsp',
  )}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const source = window.gspDebug.sourceScene;
    const scene = () => window.gspDebug.runtime.scene;
    const pointIndex = scene().points.findIndex(
      (point: any) => point.debug?.groupOrdinal === 10,
    );
    const segmentTraceIndex = scene().lines.findIndex(
      (line: any) => line.debug?.groupOrdinal === 21,
    );
    const upperTraceIndex = scene().lines.findIndex(
      (line: any) => line.debug?.groupOrdinal === 24,
    );
    const lowerTraceIndex = scene().lines.findIndex(
      (line: any) => line.debug?.groupOrdinal === 22,
    );
    const beforePoint = { ...scene().points[pointIndex] };
    const beforeUpper = { ...scene().lines[upperTraceIndex].points[250] };
    const beforeLower = { ...scene().lines[lowerTraceIndex].points[250] };

    env.updateScene((draft) => {
      const constraint = draft.points[pointIndex].constraint;
      if (constraint?.kind === 'polyline') {
        constraint.segmentIndex = 550;
        constraint.t = 0.5;
      }
    }, 'graph');
    const afterPoint = { ...scene().points[pointIndex] };

    env.updateDynamics((draft) => {
      const parameter = draft.parameters.find((candidate: any) => candidate.name === 'a');
      if (parameter) parameter.value = 1;
    });
    window.GspViewerModules.dynamics!.syncDynamicScene?.(env, ['a']);

    const segmentTrace = scene().lines[segmentTraceIndex];
    return {
      complete: source.objectGraph.geometryComplete,
      pending: source.objectGraph.pendingOperations,
      pointIndex,
      segmentTraceIndex,
      upperTraceIndex,
      lowerTraceIndex,
      segmentTraceOp: source.objectGraph.nodes.find(
        (node: any) => node.id === `line:${segmentTraceIndex}`,
      )?.definition.op.kind,
      beforePoint,
      afterPoint,
      beforeUpper,
      afterUpper: { ...scene().lines[upperTraceIndex].points[250] },
      beforeLower,
      afterLower: { ...scene().lines[lowerTraceIndex].points[250] },
      segmentPointCount: segmentTrace.points.length,
      segmentCount: segmentTrace.segments?.length ?? 0,
    };
  });

  expect(result.complete).toBe(true);
  expect(result.pending).toEqual([]);
  expect(result.pointIndex).toBeGreaterThanOrEqual(0);
  expect(result.segmentTraceIndex).toBeGreaterThanOrEqual(0);
  expect(result.upperTraceIndex).toBeGreaterThanOrEqual(0);
  expect(result.lowerTraceIndex).toBeGreaterThanOrEqual(0);
  expect(result.segmentTraceOp).toBe('zip-point-traces');
  expect(result.segmentPointCount).toBe(2000);
  expect(result.segmentCount).toBe(1000);
  expect(Math.hypot(
    result.afterPoint.x - result.beforePoint.x,
    result.afterPoint.y - result.beforePoint.y,
  )).toBeGreaterThan(1);
  expect(Math.hypot(
    result.afterUpper.x - result.beforeUpper.x,
    result.afterUpper.y - result.beforeUpper.y,
  )).toBeGreaterThan(1);
  expect(Math.hypot(
    result.afterLower.x - result.beforeLower.x,
    result.afterLower.y - result.beforeLower.y,
  )).toBeGreaterThan(1);
});
