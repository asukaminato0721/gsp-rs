import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('parabola optics colorized iterations stay live for point drag and N', async ({ page }) => {
  const file = compileFixtureToTempHtml(
    'tests/Samples/个人专栏/贺基旭作品/20171231抛物线的光学性质_hjx4882.gsp',
  );
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const pointToSegmentDistance = (point: any, start: any, end: any) => {
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const lenSq = dx * dx + dy * dy;
      if (lenSq <= 1e-9) return Math.hypot(point.x - start.x, point.y - start.y);
      const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSq));
      return Math.hypot(point.x - (start.x + dx * t), point.y - (start.y + dy * t));
    };
    const nearestPolylineDistance = (point: any, points: any[]) =>
      Math.min(...points.slice(0, -1).map((start, index) =>
        pointToSegmentDistance(point, start, points[index + 1])));
    const lineDistance = (left: any[], right: any[]) =>
      Math.max(...left.map((point, index) =>
        Math.hypot(point.x - right[index].x, point.y - right[index].y)));
    const snapshot = () => {
      const scene = window.gspDebug.runtime.scene;
      const traceLineIndex = scene.lines.findIndex((line: any) =>
        line.binding?.kind === 'point-trace' && line.debug?.groupOrdinal === 11);
      const traceLine = scene.lines[traceLineIndex];
      const locusPointIndex = scene.points.findIndex((point: any) =>
        point.constraint?.kind === 'polyline' && point.constraint.functionKey === 11);
      const locusPoint = scene.points[locusPointIndex];
      const spectrumGroups = [30, 31].map((groupOrdinal) => {
        const lines = scene.lines.filter((line: any) =>
          line.debug?.groupOrdinal === groupOrdinal && line.binding?.kind === 'colorized-spectrum');
        const visible = lines.filter((line: any) =>
          line.visible !== false && line.points.every((point: any) =>
            Number.isFinite(point.x) && Number.isFinite(point.y)));
        return {
          total: lines.length,
          visible: visible.length,
          firstFour: visible.slice(0, 4).map((line: any) =>
            line.points.map((point: any) => ({ x: point.x, y: point.y }))),
        };
      });
      return {
        traceLineIndex,
        locusPointIndex,
        driverIndex: traceLine?.binding?.driverIndex,
        point4Index: scene.points.findIndex((point: any) => point.debug?.groupOrdinal === 12),
        labelTexts: scene.labels.map((label: any) => label.text),
        parameterControlValues: Array.from(document.querySelectorAll<HTMLInputElement>('#parameter-controls input'))
          .map((input) => input.value),
        traceCount: traceLine?.points?.length ?? 0,
        locusDistance: traceLine && locusPoint
          ? nearestPolylineDistance(locusPoint, traceLine.points)
          : Number.POSITIVE_INFINITY,
        spectrumGroups,
      };
    };
    const maxLineDelta = (beforeLines: any[][], afterLines: any[][]) =>
      Math.max(...beforeLines.map((line, index) => lineDistance(line, afterLines[index])));
    const maxAbsDy = (lines: any[][]) =>
      Math.max(...lines.map((line) => Math.abs(line[1].y - line[0].y)));

    const before = snapshot();
    const movedRootIndex = before.point4Index;
    if (typeof dynamics?.sourcePointRootId === 'function') {
      env.markDependencyRootsDirty(dynamics.sourcePointRootId(movedRootIndex));
    }
    env.updateScene((draft: any) => {
      draft.points[movedRootIndex].x += 60;
      draft.points[movedRootIndex].y += 35;
    }, 'graph');
    const afterDrag = snapshot();

    env.updateDynamics((draft: any) => {
      draft.parameters.find((parameter: any) => parameter.name === 'N').value = 14;
    });
    dynamics?.syncDynamicScene?.(env, ['N']);
    dynamics?.buildParameterControls?.(env);
    const afterN = snapshot();

    return {
      before,
      afterDrag,
      afterN,
      segmentSpectrumDelta: maxLineDelta(
        before.spectrumGroups[0].firstFour,
        afterDrag.spectrumGroups[0].firstFour,
      ),
      raySpectrumDelta: maxLineDelta(
        before.spectrumGroups[1].firstFour,
        afterDrag.spectrumGroups[1].firstFour,
      ),
      beforeRayParallelDelta: maxAbsDy(before.spectrumGroups[1].firstFour),
      nSpacingDelta: lineDistance(
        afterDrag.spectrumGroups[0].firstFour[1],
        afterN.spectrumGroups[0].firstFour[1],
      ),
    };
  });

  expect(result.before.traceLineIndex).toBeGreaterThanOrEqual(0);
  expect(result.before.locusPointIndex).toBeGreaterThanOrEqual(0);
  expect(result.before.point4Index).toBeGreaterThanOrEqual(0);
  expect(result.before.labelTexts).toContain('5在L₁上的值 = 0.03');
  expect(result.before.labelTexts).toContain('5在L₁上的值 + 1 / N = 0.06');
  expect(result.before.labelTexts).toContain('t₁ + 0.1 = 0.10');
  expect(result.before.parameterControlValues).toEqual(['28', '0']);
  expect(result.before.traceCount).toBeGreaterThanOrEqual(100);
  expect(result.before.locusDistance).toBeLessThan(1);
  expect(result.before.spectrumGroups[0].visible).toBe(28);
  expect(result.before.spectrumGroups[1].visible).toBe(28);
  expect(result.beforeRayParallelDelta).toBeLessThan(1);
  expect(result.segmentSpectrumDelta).toBeGreaterThan(30);
  expect(result.raySpectrumDelta).toBeGreaterThan(30);
  expect(result.afterDrag.locusDistance).toBeLessThan(1);
  for (const line of result.afterDrag.spectrumGroups[1].firstFour) {
    expect(line[1].x).toBeGreaterThan(line[0].x);
  }
  expect(result.afterN.parameterControlValues).toEqual(['14', '0']);
  expect(result.afterN.labelTexts).toContain('5在L₁上的值 + 1 / N = 0.10');
  expect(result.afterN.spectrumGroups[0].total).toBe(28);
  expect(result.afterN.spectrumGroups[1].total).toBe(28);
  expect(result.afterN.spectrumGroups[0].visible).toBe(14);
  expect(result.afterN.spectrumGroups[1].visible).toBe(14);
  expect(result.nSpacingDelta).toBeGreaterThan(1);
});
