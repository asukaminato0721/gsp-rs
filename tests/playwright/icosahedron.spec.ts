import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('icosahedron projection rotates when dragging point A', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/向忠作品/正二十面体.gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const pointAIndex = 6;
    const projectedPointIndex = 14;
    const parametersBefore = dynamics.parameterMapForScene(env, scene);
    const before = {
      theta: parametersBefore.get('θ[Ov]'),
      phi: parametersBefore.get('φ[Ov]'),
      projected: {
        x: scene.points[projectedPointIndex].x,
        y: scene.points[projectedPointIndex].y,
      },
    };

    return {
      before,
      graphMode: window.gspDebug.sourceScene.graphMode,
      savedViewport: window.gspDebug.sourceScene.savedViewport,
      yUp: window.gspDebug.sourceScene.yUp,
      gridChildren: document.getElementById('grid-layer')?.childElementCount ?? -1,
      blackPanelCount: (window.gspDebug.sourceScene.polygons || [])
        .filter((polygon) => polygon.visible !== false
          && polygon.debug?.groupOrdinal === 41
          && polygon.binding?.kind === 'point-polygon'
          && polygon.color?.[0] === 0
          && polygon.color?.[1] === 0
          && polygon.color?.[2] === 0
          && polygon.color?.[3] === 255)
        .length,
      controls: (window.gspDebug.sourceScene.parameters || [])
        .filter((parameter) => parameter.visible !== false)
        .map((parameter) => [parameter.name, parameter.value]),
    };
  });

  expect(result.graphMode).toBe(false);
  expect(result.savedViewport).toBe(false);
  expect(result.yUp).toBe(true);
  expect(result.gridChildren).toBe(0);
  expect(result.blackPanelCount).toBeGreaterThanOrEqual(1);
  const controls = new Map(result.controls);
  for (const [name, value] of [
    ['r', 8],
    ['H₁', 1],
    ['H₂', 0.85],
    ['H₃', 0.65],
    ['H₄', 0.45],
    ['H[5]', 0.25],
  ] as const) {
    expect(controls.get(name)).toBeCloseTo(value, 8);
  }

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    const drag = window.GspViewerModules.drag;
    const point = env.currentScene().points[6];
    drag.beginDrag(env, 1, env.toScreen(point), 6, null, null, null, null);
    drag.updateDraggedPoint(env, { x: point.x + 0.58, y: point.y + 0.36 });
    env.dragState.val = null;
  });

  const after = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const dynamics = window.GspViewerModules.dynamics;
    const pointAIndex = 6;
    const projectedPointIndex = 14;
    const parametersAfter = dynamics.parameterMapForScene(env, scene);
    return {
      theta: parametersAfter.get('θ[Ov]'),
      phi: parametersAfter.get('φ[Ov]'),
      pointA: {
        x: scene.points[pointAIndex].x,
        y: scene.points[pointAIndex].y,
      },
      projected: {
        x: scene.points[projectedPointIndex].x,
        y: scene.points[projectedPointIndex].y,
      },
    };
  });

  expect(result.before.theta).toBeCloseTo(1.8575, 6);
  expect(result.before.phi).toBeCloseTo(-3.7279166667, 6);
  expect(after.theta).toBeCloseTo(after.pointA.x, 6);
  expect(after.phi).toBeCloseTo(after.pointA.y, 6);
  expect(after.theta).not.toBeCloseTo(result.before.theta, 6);
  expect(after.phi).not.toBeCloseTo(result.before.phi, 6);
  expect(Math.hypot(
    after.projected.x - result.before.projected.x,
    after.projected.y - result.before.projected.y,
  )).toBeGreaterThan(0.05);
});
