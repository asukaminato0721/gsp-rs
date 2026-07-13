import { test, expect } from '@playwright/test';
import path from 'node:path';

test('refraction sample updates its ray iterations from the light-count parameter', async ({ page }) => {
  const file = path.resolve('tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).html');
  await page.goto(`file://${file}`);

  const before = await page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    return {
      parameter: runtime.dynamics.parameters.find((parameter: { name: string }) => parameter.name === '光线条数'),
      lines: runtime.scene.lines.length,
      polygons: runtime.scene.polygons.length,
    };
  });
  expect(before.parameter?.value).toBe(8);

  await page.locator('#parameter-controls input').evaluate((element) => {
    const input = element as HTMLInputElement;
    input.value = '4';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });

  const after = await page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    return {
      parameter: runtime.dynamics.parameters.find((parameter: { name: string }) => parameter.name === '光线条数'),
      lines: runtime.scene.lines.length,
      polygons: runtime.scene.polygons.length,
    };
  });
  expect(after.parameter?.value).toBe(4);
  expect(after.lines).toBeLessThan(before.lines);
  expect(after.polygons).toBeLessThan(before.polygons);
});

test('refraction iteration arrows follow the dragged medium point', async ({ page }) => {
  const file = path.resolve('tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).html');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const before = JSON.parse(window.gspDebug.json());
    const mediumIndex = before.scene.points.findIndex(
      (point: { debug?: { groupOrdinal?: number } }) => point.debug?.groupOrdinal === 17,
    );
    const beforeLines = before.scene.lines.map((line: { points: Array<{ x: number; y: number }> }) => line.points);
    const beforePolygons = before.scene.polygons.map(
      (polygon: { points: Array<{ x: number; y: number }> }) => polygon.points,
    );
    const mediumPolygonIndex = before.scene.polygons.findIndex(
      (polygon: { debug?: { groupOrdinal?: number } }) => polygon.debug?.groupOrdinal === 12,
    );
    const medium = before.scene.points[mediumIndex];

    drag.beginDrag(env, 1, { x: medium.x, y: medium.y }, mediumIndex, null, null, null, null);
    drag.updateDraggedPoint(env, { x: medium.x + 80, y: medium.y });

    const after = JSON.parse(window.gspDebug.json());
    const movedLineCount = after.scene.lines.filter(
      (line: { points: Array<{ x: number; y: number }> }, index: number) =>
        JSON.stringify(line.points) !== JSON.stringify(beforeLines[index]),
    ).length;
    const movedPolygonCount = after.scene.polygons.filter(
      (polygon: { points: Array<{ x: number; y: number }> }, index: number) =>
        JSON.stringify(polygon.points) !== JSON.stringify(beforePolygons[index]),
    ).length;
    return {
      mediumBeforeX: medium.x,
      mediumAfterX: after.scene.points[mediumIndex].x,
      refractedControlBeforeX: before.scene.points[29].x,
      refractedControlAfterX: after.scene.points[29].x,
      mediumColorBefore: before.scene.polygons[mediumPolygonIndex].color,
      mediumColorAfter: after.scene.polygons[mediumPolygonIndex].color,
      iteratedArrowColors: Array.from(new Set(
        after.scene.polygons.slice(8).map(
          (polygon: { color: [number, number, number, number] }) => JSON.stringify(polygon.color),
        ),
      )).sort(),
      movedLineCount,
      movedPolygonCount,
    };
  });

  expect(result.mediumAfterX).toBeGreaterThan(result.mediumBeforeX + 70);
  expect(result.refractedControlAfterX).not.toBeCloseTo(result.refractedControlBeforeX);
  expect(result.mediumColorAfter).not.toEqual(result.mediumColorBefore);
  expect(result.iteratedArrowColors).toEqual([
    JSON.stringify([0, 0, 255, 255]),
    JSON.stringify([255, 0, 0, 255]),
    JSON.stringify([255, 0, 255, 255]),
  ].sort());
  expect(result.movedLineCount).toBeGreaterThan(8);
  expect(result.movedPolygonCount).toBeGreaterThan(8);
});

test('refraction iteration arrows follow the dragged ray-spacing point', async ({ page }) => {
  const file = path.resolve('tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).html');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const before = JSON.parse(window.gspDebug.json());
    const controlIndex = before.scene.points.findIndex(
      (point: { debug?: { groupOrdinal?: number } }) => point.debug?.groupOrdinal === 27,
    );
    const beforeLines = before.scene.lines.map((line: { points: Array<{ x: number; y: number }> }) => line.points);
    const beforePolygons = before.scene.polygons.map(
      (polygon: { points: Array<{ x: number; y: number }> }) => polygon.points,
    );
    const control = before.scene.points[controlIndex];

    drag.beginDrag(env, 2, { x: control.x, y: control.y }, controlIndex, null, null, null, null);
    drag.updateDraggedPoint(env, { x: control.x + 40, y: control.y });

    const after = JSON.parse(window.gspDebug.json());
    return {
      controlBeforeX: control.x,
      controlAfterX: after.scene.points[controlIndex].x,
      movedLineCount: after.scene.lines.filter(
        (line: { points: Array<{ x: number; y: number }> }, index: number) =>
          JSON.stringify(line.points) !== JSON.stringify(beforeLines[index]),
      ).length,
      movedPolygonCount: after.scene.polygons.filter(
        (polygon: { points: Array<{ x: number; y: number }> }, index: number) =>
          JSON.stringify(polygon.points) !== JSON.stringify(beforePolygons[index]),
      ).length,
    };
  });

  expect(result.controlAfterX).toBeGreaterThan(result.controlBeforeX + 30);
  expect(result.movedLineCount).toBeGreaterThan(8);
  expect(result.movedPolygonCount).toBeGreaterThan(8);
});
