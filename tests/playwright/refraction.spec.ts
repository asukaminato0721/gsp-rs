import { test, expect } from '@playwright/test';
import path from 'node:path';

test('refraction sample preserves payload background and rich-text styling without synthetic geometry', async ({ page }) => {
  const file = path.resolve('tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).html');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    const title = runtime.scene.labels.find(
      (label: { debug?: { groupOrdinal?: number } }) => label.debug?.groupOrdinal === 126,
    );
    const richLabel = document.querySelector<HTMLElement>('.scene-rich-label[data-gsp-group="126"]');
    const styledTitle = richLabel?.querySelector<HTMLElement>('.scene-rich-line:first-child span');
    const upperMediumPolygon = document.querySelector<SVGElement>('[data-gsp-group="11"]');
    const tableText = Array.from(
      document.querySelectorAll<SVGTextElement>('[data-gsp-group="83"] text'),
      (node) => node.textContent,
    );
    return {
      background: runtime.scene.backgroundColor,
      canvasBackground: getComputedStyle(document.querySelector('#view') as Element).backgroundColor,
      hasSyntheticHexagon: runtime.scene.lines.some(
        (line: { debug?: unknown; color: number[]; points: unknown[] }) =>
          !line.debug
          && JSON.stringify(line.color) === JSON.stringify([30, 30, 30, 255])
          && line.points.length === 7,
      ),
      titleColor: title?.color,
      titleFontSize: title?.fontSize,
      titleFontFamily: title?.fontFamily,
      titleScreenSpace: title?.screenSpace,
      inlineTitleColor: styledTitle ? getComputedStyle(styledTitle).color : null,
      inlineTitleFontSize: styledTitle ? getComputedStyle(styledTitle).fontSize : null,
      upperMediumPolygonStroke: upperMediumPolygon?.getAttribute('stroke'),
      tableText,
    };
  });

  expect(result.background).toEqual([253, 224, 181, 255]);
  expect(result.canvasBackground).toBe('rgb(253, 224, 181)');
  expect(result.hasSyntheticHexagon).toBe(false);
  expect(result.titleColor).toEqual([0, 0, 255, 255]);
  expect(result.titleFontSize).toBe(24);
  expect(result.titleFontFamily).toBe('Times New Roman');
  expect(result.titleScreenSpace).toBe(true);
  expect(result.inlineTitleColor).toBe('rgb(0, 128, 0)');
  expect(result.inlineTitleFontSize).toBe('48px');
  expect(result.upperMediumPolygonStroke).toBe('none');
  expect(result.tableText).toEqual([
    '入射角θ₁', '折射角θ₂', 'sinθ₁/sinθ₂', '43.11°', '24.62°', '1.64',
  ]);
});

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

test('refraction show-hide button toggles the reflected ray and every iterated image', async ({ page }) => {
  const file = path.resolve('tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).html');
  await page.goto(`file://${file}`);

  const visibilityState = () => page.evaluate(() => {
    const runtime = JSON.parse(window.gspDebug.json());
    const button = runtime.buttons.find(
      (candidate: { text: string }) => candidate.text.includes('反射光线'),
    );
    const action = button.action;
    const red = JSON.stringify([255, 0, 0, 255]);
    const generatedRedLines = runtime.scene.lines.filter(
      (line: { debug?: unknown; color: number[] }) => !line.debug && JSON.stringify(line.color) === red,
    );
    const generatedRedPolygons = runtime.scene.polygons.filter(
      (polygon: { debug?: unknown; color: number[] }) =>
        !polygon.debug && JSON.stringify(polygon.color) === red,
    );
    return {
      text: button.text,
      directVisible: [
        ...action.lineIndices.map((index: number) => runtime.scene.lines[index].visible),
        ...action.polygonIndices.map((index: number) => runtime.scene.polygons[index].visible),
      ],
      lineIterationVisible: action.lineIterationIndices.map(
        (index: number) => runtime.scene.lineIterations[index].visible,
      ),
      polygonIterationVisible: action.polygonIterationIndices.map(
        (index: number) => runtime.scene.polygonIterations[index].visible,
      ),
      generatedRedVisible: [...generatedRedLines, ...generatedRedPolygons].map(
        (shape: { visible: boolean }) => shape.visible,
      ),
    };
  });

  const before = await visibilityState();
  expect(before.text).toBe('隐藏反射光线');
  expect(before.directVisible.every(Boolean)).toBe(true);
  expect(before.lineIterationVisible).toEqual([true]);
  expect(before.polygonIterationVisible).toEqual([true]);
  expect(before.generatedRedVisible.length).toBeGreaterThan(8);
  expect(before.generatedRedVisible.every(Boolean)).toBe(true);

  await page.getByRole('button', { name: '隐藏反射光线' }).click();
  const hidden = await visibilityState();
  expect(hidden.text).toBe('显示反射光线');
  expect(hidden.directVisible.every((visible: boolean) => !visible)).toBe(true);
  expect(hidden.lineIterationVisible).toEqual([false]);
  expect(hidden.polygonIterationVisible).toEqual([false]);
  expect(hidden.generatedRedVisible.every((visible: boolean) => !visible)).toBe(true);

  await page.locator('#parameter-controls input').evaluate((element) => {
    const input = element as HTMLInputElement;
    input.value = '4';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  const rebuiltWhileHidden = await visibilityState();
  expect(rebuiltWhileHidden.generatedRedVisible.length).toBeGreaterThan(4);
  expect(rebuiltWhileHidden.generatedRedVisible.every((visible: boolean) => !visible)).toBe(true);

  await page.getByRole('button', { name: '显示反射光线' }).click();
  const shown = await visibilityState();
  expect(shown.text).toBe('隐藏反射光线');
  expect(shown.directVisible.every(Boolean)).toBe(true);
  expect(shown.lineIterationVisible).toEqual([true]);
  expect(shown.polygonIterationVisible).toEqual([true]);
  expect(shown.generatedRedVisible.every(Boolean)).toBe(true);
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
    const tableValuesBefore = before.scene.iterationTables[0].rows[0].values;

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
      tableValuesBefore,
      tableValuesAfter: after.scene.iterationTables[0].rows[0].values,
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
  expect(result.tableValuesAfter[0]).toBeCloseTo(result.tableValuesBefore[0]);
  expect(result.tableValuesAfter[1]).not.toBeCloseTo(result.tableValuesBefore[1]);
  expect(result.tableValuesAfter[2]).not.toBeCloseTo(result.tableValuesBefore[2]);
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
