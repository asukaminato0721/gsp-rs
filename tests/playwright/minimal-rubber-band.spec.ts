import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-minimal-rubber-band-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

test('minimal rubber-band fixture keeps three-point ratio dilation live', async ({ page }) => {
  const file = compileFixtureToTempHtml('tests/Samples/个人专栏/方小庆作品/最简橡皮筋(inRm).gsp');
  await page.goto(`file://${file}`);

  const result = await page.evaluate(() => {
    const scene = window.gspDebug.runtime.scene;
    const env = window.gspDebug.viewerEnv;
    const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
    const beforeSegments = scene.lines
      .filter((line) => line.binding?.kind === 'segment')
      .map((line) => ({ binding: line.binding, points: line.points.map((point) => ({ ...point })) }));

    if (typeof rootId === 'function') {
      env.markDependencyRootsDirty(rootId(1));
    }
    env.updateScene((draft) => {
      draft.points[1].x += 40;
      draft.points[1].y -= 15;
    }, 'graph');

    const afterScene = window.gspDebug.runtime.scene;
    const segmentLines = afterScene.lines
      .filter((line) => line.binding?.kind === 'segment')
      .map((line) => ({ binding: line.binding, points: line.points.map((point) => ({ ...point })) }));
    const bendPoint = afterScene.points[7];
    const sourcePoint2 = afterScene.points[1];
    return {
      helper6: { x: scene.points[5].x, y: scene.points[5].y },
      beforeSegments,
      segmentLines,
      bendPoint: { x: bendPoint.x, y: bendPoint.y },
      sourcePoint2: { x: sourcePoint2.x, y: sourcePoint2.y },
      scaleByRatioCount: afterScene.points.filter((point) => point.binding?.kind === 'scale-by-ratio').length,
    };
  });

  expect(result.helper6.x).toBeCloseTo(375.457041, 3);
  expect(result.helper6.y).toBeCloseTo(150.581478, 3);
  expect(result.beforeSegments).toHaveLength(2);
  expect(result.segmentLines).toHaveLength(2);
  const segmentEndpointPairs = result.segmentLines.map((line) =>
    [line.binding.startIndex, line.binding.endIndex].sort((left, right) => left - right).join(':'));
  expect(segmentEndpointPairs).toContain('2:7');
  expect(segmentEndpointPairs).toContain('1:7');
  expect(result.bendPoint.x).toBeCloseTo(300, 3);
  expect(result.bendPoint.y).toBeCloseTo(212, 3);
  const bendSegment = result.segmentLines.find((line) =>
    [line.binding.startIndex, line.binding.endIndex].sort((left, right) => left - right).join(':') === '1:7');
  expect(bendSegment?.points[0].x).toBeCloseTo(result.bendPoint.x, 3);
  expect(bendSegment?.points[0].y).toBeCloseTo(result.bendPoint.y, 3);
  expect(bendSegment?.points[1].x).toBeCloseTo(result.sourcePoint2.x, 3);
  expect(bendSegment?.points[1].y).toBeCloseTo(result.sourcePoint2.y, 3);
  expect(result.scaleByRatioCount).toBe(2);
});
