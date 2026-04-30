import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-simple-axis-coordinate-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  execFileSync('cargo', ['run', '--', '--no-upload', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}

const samples = [
  {
    path: 'tests/Samples/简易数轴与坐标系/制作坐标系1步骤分解/步骤1.gsp',
    buttons: 0,
  },
  {
    path: 'tests/Samples/简易数轴与坐标系/制作坐标系1步骤分解/步骤2.gsp',
    buttons: 0,
  },
  {
    path: 'tests/Samples/简易数轴与坐标系/制作坐标系1步骤分解/步骤3.gsp',
    buttons: 0,
  },
  {
    path: 'tests/Samples/简易数轴与坐标系/制作坐标系1步骤分解/步骤4.gsp',
    buttons: 0,
  },
  {
    path: 'tests/Samples/简易数轴与坐标系/最简坐标系/样本1.gsp',
    buttons: 2,
  },
  {
    path: 'tests/Samples/简易数轴与坐标系/最简坐标系/样本2.gsp',
    buttons: 0,
  },
  {
    path: 'tests/Samples/简易数轴与坐标系/数轴.gsp',
    buttons: 4,
    hiddenLabels: ["m₁ = 4.43", "m₂ = 1", "C'"],
    visibleLabels: ['-3', '-2', '-1', '0', '1', '2', '3'],
    axisTickCount: 7,
  },
];

for (const sample of samples) {
  test(`simple axis/coordinate sample compiles and stays interactive: ${sample.path}`, async ({ page }) => {
    const file = compileFixtureToTempHtml(sample.path);
    const pageErrors: string[] = [];
    page.on('pageerror', (error) => pageErrors.push(String(error)));

    await page.goto(`file://${file}`);
    await page.waitForTimeout(100);

    const result = await page.evaluate((shouldCheckAxisDrag) => {
      const sourceScene = window.gspDebug.sourceScene;
      const resolvePoint = (handle: any) => {
        const scene = window.gspDebug.runtime.scene;
        if (typeof handle?.pointIndex === 'number') {
          return scene.points[handle.pointIndex];
        }
        return handle;
      };
      const axisState = () => ({
        visibleLabels: window.gspDebug.runtime.scene.labels
          .filter((label: any) => label.visible !== false)
          .map((label: any) => label.text),
        numericLabels: window.gspDebug.runtime.scene.labels
          .filter((label: any) => label.visible !== false && /^-?\d+$/.test(label.text))
          .map((label: any) => Number(label.text)),
        numericLabelAnchors: Object.fromEntries(window.gspDebug.runtime.scene.labels
          .filter((label: any) => label.visible !== false && /^-?\d+$/.test(label.text))
          .map((label: any) => [label.text, window.gspDebug.viewerEnv.resolvePoint(label.anchor)])),
        axisTickXs: window.gspDebug.runtime.scene.lines
          .filter((line: any) => {
            const [start, end] = (line.points ?? []).map(resolvePoint);
            const color = line.color ?? [];
            return line.visible !== false
              && Math.abs((start?.x ?? 0) - (end?.x ?? 1)) < 1e-6
              && Math.abs((start?.y ?? 0) - (end?.y ?? 0)) > 20
              && color[2] === 255;
            })
          .map((line: any) => resolvePoint(line.points[0]).x)
          .sort((left: number, right: number) => left - right),
        tickHeight: Math.max(...window.gspDebug.runtime.scene.lines
          .filter((line: any) => {
            const [start, end] = (line.points ?? []).map(resolvePoint);
            const color = line.color ?? [];
            return line.visible !== false
              && Math.abs((start?.x ?? 0) - (end?.x ?? 1)) < 1e-6
              && color[2] === 255;
          })
          .map((line: any) => {
            const [start, end] = (line.points ?? []).map(resolvePoint);
            return Math.abs((start?.y ?? 0) - (end?.y ?? 0));
          })),
        labelControl: (() => {
          const index = window.gspDebug.runtime.scene.points.findIndex((point: any) => {
            const color = point.color ?? [];
            return point.visible !== false && color[0] === 0 && color[1] === 255 && color[2] === 255;
          });
          return { index, y: window.gspDebug.runtime.scene.points[index]?.y ?? null };
        })(),
        tickControl: (() => {
          const index = window.gspDebug.runtime.scene.points.findIndex((point: any) => {
            const color = point.color ?? [];
            return point.visible !== false && color[0] === 255 && color[1] === 128 && color[2] === 0;
          });
          return { index, y: window.gspDebug.runtime.scene.points[index]?.y ?? null };
        })(),
        arrow: (() => {
          const scene = window.gspDebug.runtime.scene;
          const arrowControlIndex = scene.points.findIndex((point: any) => {
            const color = point.color ?? [];
            return point.visible !== false
              && point.draggable !== false
              && point.constraint?.kind === 'segment'
              && color[0] === 255
              && color[1] === 0
              && color[2] === 0;
          });
          const arrowPolygon = scene.polygons.find((polygon: any) =>
            polygon.visible !== false && polygon.binding?.kind === 'point-polygon'
          );
          const vertexIndices = arrowPolygon?.binding?.vertexIndices ?? [];
          const tip = scene.points[vertexIndices[1]];
          const base = scene.points[vertexIndices[3]];
          return {
            controlIndex: arrowControlIndex,
            controlX: scene.points[arrowControlIndex]?.x ?? null,
            headLength: tip && base ? Math.hypot(tip.x - base.x, tip.y - base.y) : null,
          };
        })(),
      });
      const initialAxis = axisState();
      const scene = window.gspDebug.runtime.scene;
      const firstInteractivePointIndex = scene.points.findIndex((point: any) => point.draggable !== false);
      let movedPoint = null;

      if (firstInteractivePointIndex >= 0) {
        const before = scene.points[firstInteractivePointIndex];
        const rootId = window.GspViewerModules.dynamics?.sourcePointRootId;
        if (typeof rootId === 'function') {
          window.gspDebug.viewerEnv.markDependencyRootsDirty(rootId(firstInteractivePointIndex));
        }
        window.gspDebug.viewerEnv.updateScene((draft: any) => {
          draft.points[firstInteractivePointIndex].x += 0.25;
          draft.points[firstInteractivePointIndex].y -= 0.2;
        }, 'graph');
        const after = window.gspDebug.runtime.scene.points[firstInteractivePointIndex];
        movedPoint = {
          before: { x: before.x, y: before.y },
          after: { x: after.x, y: after.y },
        };
      }
      let axisDrag = null;
      if (shouldCheckAxisDrag) {
        const drag = window.GspViewerModules.drag;
        const env = window.gspDebug.viewerEnv;
        const dragPoint = (pointIndex: number, dx: number, dy: number) => {
          const point = env.currentScene().points[pointIndex];
          drag.beginDrag(env, 1, { x: point.x, y: point.y }, pointIndex, null, null, null, null);
          drag.updateDraggedPoint(env, { x: point.x + dx, y: point.y + dy });
          env.dragState.val = null;
        };
        dragPoint(0, 20, 10);
        const afterZeroDrag = axisState();
        dragPoint(3, 100, 0);
        const afterRightDrag = axisState();
        if (afterRightDrag.labelControl.index >= 0) {
          dragPoint(afterRightDrag.labelControl.index, 0, 20);
        }
        const afterLabelControlDrag = axisState();
        if (afterLabelControlDrag.tickControl.index >= 0) {
          dragPoint(afterLabelControlDrag.tickControl.index, 0, 20);
        }
        const afterTickControlDrag = axisState();
        if (afterRightDrag.arrow.controlIndex >= 0) {
          dragPoint(afterRightDrag.arrow.controlIndex, 20, 0);
        }
        const afterArrowControlDrag = axisState();
        axisDrag = {
          initial: initialAxis,
          afterZeroDrag,
          afterRightDrag,
          afterLabelControlDrag,
          afterTickControlDrag,
          afterArrowControlDrag,
        };
      }

      return {
        sourcePoints: sourceScene.points.length,
        sourceLines: sourceScene.lines.length,
        sourceLabels: sourceScene.labels.length,
        runtimePoints: scene.points.length,
        runtimeLines: scene.lines.length,
        runtimeLabels: scene.labels.length,
        runtimeButtons: scene.buttons.length,
        draggablePoints: scene.points.filter((point: any) => point.draggable !== false).length,
        svgElementCount: document.querySelectorAll('svg *').length,
        visibleLabels: scene.labels
          .filter((label: any) => label.visible !== false)
          .map((label: any) => label.text),
        axisTickXs: initialAxis.axisTickXs,
        movedPoint,
        axisDrag,
      };
    }, sample.path.endsWith('/数轴.gsp'));

    expect(pageErrors).toEqual([]);
    expect(result.sourcePoints).toBeGreaterThan(0);
    expect(result.sourceLines).toBeGreaterThan(0);
    expect(result.sourceLabels).toBeGreaterThan(0);
    expect(result.runtimePoints).toBe(result.sourcePoints);
    expect(result.runtimeLines).toBeGreaterThan(0);
    expect(result.runtimeLabels).toBeGreaterThan(0);
    expect(result.runtimeButtons).toBe(sample.buttons);
    expect(result.draggablePoints).toBeGreaterThan(0);
    expect(result.svgElementCount).toBeGreaterThan(0);
    for (const hiddenLabel of sample.hiddenLabels ?? []) {
      expect(result.visibleLabels).not.toContain(hiddenLabel);
    }
    for (const visibleLabel of sample.visibleLabels ?? []) {
      expect(result.visibleLabels).toContain(visibleLabel);
    }
    if (sample.axisTickCount !== undefined) {
      expect(result.axisTickXs).toHaveLength(sample.axisTickCount);
      const spacings = result.axisTickXs.slice(1).map((x: number, index: number) => x - result.axisTickXs[index]);
      for (const spacing of spacings) {
        expect(spacing).toBeCloseTo(spacings[0], 3);
      }
      expect(result.axisDrag?.afterZeroDrag.axisTickXs).toHaveLength(sample.axisTickCount);
      const tickShift = result.axisDrag.afterZeroDrag.axisTickXs[0] - result.axisDrag.initial.axisTickXs[0];
      for (const visibleLabel of sample.visibleLabels ?? []) {
        expect(result.axisDrag?.afterZeroDrag.visibleLabels).toContain(visibleLabel);
        const before = result.axisDrag?.initial.numericLabelAnchors[visibleLabel];
        const after = result.axisDrag?.afterZeroDrag.numericLabelAnchors[visibleLabel];
        expect(after?.x - before?.x).toBeCloseTo(tickShift, 3);
      }
      const beforeZero = result.axisDrag?.initial.numericLabelAnchors['0'];
      const afterZero = result.axisDrag?.afterZeroDrag.numericLabelAnchors['0'];
      expect(afterZero?.y).toBeCloseTo(beforeZero?.y ?? 0, 3);
      expect(result.axisDrag?.afterRightDrag.axisTickXs.length).toBeGreaterThan(sample.axisTickCount);
      expect(Math.max(...(result.axisDrag?.afterRightDrag.numericLabels ?? []))).toBeGreaterThan(3);
      expect(result.axisDrag?.initial.labelControl.index).toBeGreaterThanOrEqual(0);
      expect(result.axisDrag?.initial.tickControl.index).toBeGreaterThanOrEqual(0);
      expect(result.axisDrag?.afterLabelControlDrag.numericLabelAnchors['0']?.y).toBeGreaterThan(
        (result.axisDrag?.afterRightDrag.numericLabelAnchors['0']?.y ?? 0) + 10,
      );
      expect(result.axisDrag?.afterTickControlDrag.tickHeight).toBeLessThan(
        (result.axisDrag?.afterLabelControlDrag.tickHeight ?? 0) - 10,
      );
      expect(result.axisDrag?.initial.arrow.controlIndex).toBeGreaterThanOrEqual(0);
      expect(result.axisDrag?.afterRightDrag.arrow.headLength).toBeCloseTo(
        result.axisDrag?.afterZeroDrag.arrow.headLength ?? 0,
        3,
      );
      expect(result.axisDrag?.afterArrowControlDrag.arrow.controlX).toBeGreaterThan(
        (result.axisDrag?.afterRightDrag.arrow.controlX ?? 0) + 10,
      );
      expect(result.axisDrag?.afterArrowControlDrag.arrow.headLength).toBeLessThan(
        (result.axisDrag?.afterRightDrag.arrow.headLength ?? 0) - 4,
      );
    }
    expect(result.movedPoint).not.toBeNull();
    expect(result.movedPoint?.after.x).toBeCloseTo((result.movedPoint?.before.x ?? 0) + 0.25, 3);
    expect(result.movedPoint?.after.y).toBeCloseTo((result.movedPoint?.before.y ?? 0) - 0.2, 3);
  });
}
