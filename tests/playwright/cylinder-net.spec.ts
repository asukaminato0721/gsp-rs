import { test, expect } from '@playwright/test';
import path from 'node:path';

test('cylinder net move buttons keep the calculated radius and traces live', async ({ page }) => {
  const file = path.resolve(
    'tests/Samples/个人专栏/侯仰顺作品/圆柱侧面展开图(蚂蚁制作).html',
  );
  await page.goto(`file://${file}`);

  const read = () => page.evaluate(() => {
    const scene = JSON.parse(window.gspDebug.json()).scene;
    const point = (ordinal: number) => scene.points.find(
      (candidate: any) => candidate.debug?.groupOrdinal === ordinal,
    );
    const line = (ordinal: number) => scene.lines.find(
      (candidate: any) => candidate.debug?.groupOrdinal === ordinal,
    );
    const circle = (ordinal: number) => scene.circles.find(
      (candidate: any) => candidate.debug?.groupOrdinal === ordinal,
    );
    const segmentLength = (ordinal: number) => {
      const segment = line(ordinal);
      return segment?.points?.length >= 2
        ? Math.hypot(
          segment.points[1].x - segment.points[0].x,
          segment.points[1].y - segment.points[0].y,
        )
        : null;
    };
    return {
      driver: point(9),
      calculatedEndpoint: point(7),
      circle: circle(17),
      trace: line(108),
      tracePoint: point(111),
      traceIntersection: point(113),
      visibleSideLength: segmentLength(118),
      segmentTracePaintOrder: scene.lines
        .filter((candidate: any) => candidate.binding?.kind === 'segment-trace')
        .map((candidate: any) => candidate.debug?.groupOrdinal),
      segmentTraces: [74, 75, 92, 115].map((ordinal) => {
        const trace = line(ordinal);
        const points = (trace?.segments ?? []).flat();
        const xValues = points.map((point: any) => point.x);
        return {
          ordinal,
          bindingKind: trace?.binding?.kind,
          segmentCount: trace?.segments?.length ?? 0,
          maxSegmentLength: Math.max(0, ...(trace?.segments ?? []).map(
            (segment: any[]) => segment.length >= 2
              ? Math.hypot(segment[1].x - segment[0].x, segment[1].y - segment[0].y)
              : 0,
          )),
          xSpan: xValues.length > 0
            ? Math.max(...xValues) - Math.min(...xValues)
            : 0,
          color: trace?.color,
        };
      }),
      buttons: scene.buttons.map((button: any) => ({
        text: button.text,
        kind: button.action?.kind,
      })),
    };
  });

  const before = await read();
  expect(before.calculatedEndpoint.binding?.kind).toBe('polar-offset');
  expect(before.circle.binding?.kind).toBe('expression-radius-circle');
  expect(before.trace.binding?.kind).toBe('point-trace');
  expect(before.trace.points.length).toBe(500);
  expect(before.tracePoint.constraint?.kind).toBe('polyline');
  expect(before.traceIntersection.constraint?.kind).toBe('line-trace-intersection');
  expect(before.segmentTraces.map((trace) => trace.bindingKind)).toEqual([
    'segment-trace',
    'segment-trace',
    'segment-trace',
    'segment-trace',
  ]);
  expect(before.buttons).toEqual(expect.arrayContaining([
    { text: '展开', kind: 'move-point' },
    { text: '还原', kind: 'move-point' },
  ]));

  await page.getByRole('button', { name: '展开' }).click();
  await page.waitForTimeout(1_200);
  const expanded = await read();
  expect(Math.hypot(
    expanded.driver.x - before.driver.x,
    expanded.driver.y - before.driver.y,
  )).toBeGreaterThan(1);
  expect(expanded.visibleSideLength).toBeGreaterThan(1);
  expect(expanded.trace.points).not.toEqual(before.trace.points);
  expect(expanded.segmentTraces.some((trace) => trace.segmentCount > 100)).toBe(true);

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const pointIndex = window.gspDebug.runtime.scene.points.findIndex(
      (point: any) => point.debug?.groupOrdinal === 9,
    );
    env.markDependencyRootsDirty?.([dynamics.sourcePointRootId(pointIndex)]);
    env.updateScene((draft: any) => {
      dynamics.applyNormalizedParameterToPoint(
        draft.points[pointIndex],
        draft,
        1 / Math.PI,
      );
    }, 'graph');
  });
  await page.waitForTimeout(100);
  const referenceState = await read();
  expect(referenceState.segmentTracePaintOrder).toEqual([92, 75, 74, 115]);
  expect(referenceState.segmentTraces.map((trace) => trace.color)).toEqual([
    [102, 102, 178, 255],
    [192, 192, 192, 255],
    [192, 192, 192, 255],
    [255, 255, 0, 255],
  ]);
  expect(referenceState.segmentTraces.every((trace) => trace.segmentCount > 200)).toBe(true);
  expect(referenceState.segmentTraces.every((trace) => trace.maxSegmentLength > 100)).toBe(true);
  expect(referenceState.segmentTraces[0].xSpan).toBeGreaterThan(20);
  expect(referenceState.segmentTraces[1].xSpan).toBeGreaterThan(70);
  expect(referenceState.segmentTraces[2].xSpan).toBeGreaterThan(100);
  expect(referenceState.segmentTraces[3].xSpan).toBeGreaterThan(100);
});
