import { test, expect } from '@playwright/test';
import { compileFixtureToTempHtml } from './compile-fixture';

test('two explicit circle interiors remain independent while circles move', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/fixtures/未实现/(inRm)两圆之交.gsp');
  await page.goto(`file://${file}`);

  const initial = await page.evaluate(() => ({
    filledCircleCount: Array.from(document.querySelectorAll('circle[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none' && Number(element.getAttribute('r')) > 1)
      .length,
    filledPathCount: Array.from(document.querySelectorAll('path[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none')
      .length,
  }));
  expect(initial).toEqual({ filledCircleCount: 2, filledPathCount: 0 });

  await page.evaluate(() => {
    const env = window.gspDebug.viewerEnv;
    env.updateScene((draft) => {
      draft.circles[3].radiusPoint = { x: 320, y: 182 };
    });
  });

  const intersecting = await page.evaluate(() => ({
    filledCircleCount: Array.from(document.querySelectorAll('circle[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none' && Number(element.getAttribute('r')) > 1)
      .length,
    filledPathCount: Array.from(document.querySelectorAll('path[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none')
      .length,
    filledCircleRadii: Array.from(document.querySelectorAll('circle[data-gsp-kind=circles]'))
      .filter((element) => element.getAttribute('fill') !== 'none')
      .map((element) => Number(element.getAttribute('r'))),
  }));
  expect(intersecting.filledCircleCount).toBe(2);
  expect(intersecting.filledPathCount).toBe(0);
  expect(intersecting.filledCircleRadii).toHaveLength(2);
});
