import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-external-tangent-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('two-circle external tangent stays interactive after radius change', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/方小庆作品/两圆之外切线(inRm).gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    function tangentSnapshot() {
      const scene = window.gspDebug.runtime.scene;
      const tangentLine = scene.lines[1].points;
      const leftCenter = scene.points[3];
      const leftRadiusPoint = scene.points[0];
      const rightCenter = scene.points[2];
      const rightRadiusPoint = scene.points[1];
      const rightTangentPoint = scene.points[10];
      const distancePointToLine = (
        point: { x: number; y: number },
        start: { x: number; y: number },
        end: { x: number; y: number },
      ) => {
        const dx = end.x - start.x;
        const dy = end.y - start.y;
        return Math.abs(dy * point.x - dx * point.y + end.x * start.y - end.y * start.x)
          / Math.hypot(dx, dy);
      };

      return {
        tangentLine,
        helperX: scene.points[7].x,
        rightTangentPointX: rightTangentPoint.x,
        leftRadius: Math.hypot(leftRadiusPoint.x - leftCenter.x, leftRadiusPoint.y - leftCenter.y),
        rightRadius: Math.hypot(rightRadiusPoint.x - rightCenter.x, rightRadiusPoint.y - rightCenter.y),
        leftDistance: distancePointToLine(leftCenter, tangentLine[0], tangentLine[1]),
        rightDistance: distancePointToLine(rightCenter, tangentLine[0], tangentLine[1]),
        rightTangentDistance: Math.hypot(
          rightTangentPoint.x - rightCenter.x,
          rightTangentPoint.y - rightCenter.y,
        ),
      };
    }

    const before = tangentSnapshot();
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(0));
    }
    env.updateScene((draft) => {
      draft.points[0].x += 80;
      draft.points[0].y -= 20;
    }, 'graph');
    return { before, after: tangentSnapshot() };
  });

  expect(result.after.helperX).toBeGreaterThan(1000);
  expect(result.after.rightTangentPointX).not.toBeCloseTo(result.before.rightTangentPointX, 6);
  expect(result.after.leftDistance).toBeCloseTo(result.after.leftRadius, 6);
  expect(result.after.rightDistance).toBeCloseTo(result.after.rightRadius, 6);
  expect(result.after.rightTangentDistance).toBeCloseTo(result.after.rightRadius, 6);
});

test('two-circle common tangents follow line-hosted radius points', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/方小庆作品/两圆之公切线(inRm).gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const distancePointToLine = (
      point: { x: number; y: number },
      start: { x: number; y: number },
      end: { x: number; y: number },
    ) => {
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      return Math.abs(dy * point.x - dx * point.y + end.x * start.y - end.y * start.x)
        / Math.hypot(dx, dy);
    };

    function tangentSnapshot() {
      const scene = window.gspDebug.runtime.scene;
      const leftCenter = scene.points[0];
      const rightCenter = scene.points[1];
      const leftRadiusPoint = scene.points[2];
      const rightRadiusPoint = scene.points[3];
      const leftRadius = Math.hypot(leftRadiusPoint.x - leftCenter.x, leftRadiusPoint.y - leftCenter.y);
      const rightRadius = Math.hypot(rightRadiusPoint.x - rightCenter.x, rightRadiusPoint.y - rightCenter.y);
      const tangentLines = [scene.lines[2], scene.lines[5]].map((line) => ({
        leftDistance: distancePointToLine(leftCenter, line.points[0], line.points[1]),
        rightDistance: distancePointToLine(rightCenter, line.points[0], line.points[1]),
      }));

      return {
        linePointX: leftRadiusPoint.x,
        helperX: scene.points[5].x,
        upperIntersectionY: scene.points[6].y,
        lowerIntersectionY: scene.points[9].y,
        leftRadius,
        rightRadius,
        tangentLines,
      };
    }

    const before = tangentSnapshot();
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(2));
    }
    env.updateScene((draft) => {
      draft.points[2].constraint.t = 0.72;
    }, 'graph');
    return { before, after: tangentSnapshot() };
  });

  expect(result.after.linePointX).not.toBeCloseTo(result.before.linePointX, 6);
  expect(result.after.helperX).not.toBeCloseTo(result.before.helperX, 6);
  expect(result.after.upperIntersectionY).not.toBeCloseTo(result.before.upperIntersectionY, 6);
  expect(result.after.lowerIntersectionY).not.toBeCloseTo(result.before.lowerIntersectionY, 6);
  for (const tangentLine of result.after.tangentLines) {
    expect(tangentLine.leftDistance).toBeCloseTo(result.after.leftRadius, 6);
    expect(tangentLine.rightDistance).toBeCloseTo(result.after.rightRadius, 6);
  }
});
