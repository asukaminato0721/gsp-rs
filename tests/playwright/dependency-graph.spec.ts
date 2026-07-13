import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

function compilePointFixture(): string {
  const root = process.cwd();
  const source = path.resolve(root, 'tests/fixtures/gsp/static/point.gsp');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-dependency-graph-'));
  const fixture = path.join(directory, 'point.gsp');
  fs.copyFileSync(source, fixture);
  execFileSync(path.resolve(root, 'target/debug/gsp-rs'), ['--html', fixture], {
    cwd: root,
    stdio: 'pipe',
  });
  return fixture.replace(/\.gsp$/i, '.html');
}

test('dependency graph uses typed references, resolves arbitrary-depth chains, and rejects cycles', async ({ page }) => {
  await page.goto(`file://${compilePointFixture()}`);

  const result = await page.evaluate(() => {
    const createRuntime = () => window.GspViewerModules.dynamicsDependencyGraph!
      .createDependencyGraphRuntime({
        applyBaseDynamicUpdates() {},
        parameterMapForScene() { return new Map(); },
        refreshDerivedPoints() {},
        refreshDynamicLabels() {},
        refreshIterationGeometry() {},
      });
    const baseScene = structuredClone(window.gspDebug!.sourceScene);
    const parsedParameter = (name: string) => ({
      kind: 'parsed',
      expr: { kind: 'parameter', name, value: 0 },
    });
    const label = (resultName: string, sourceName: string) => ({
      anchor: { x: 0, y: 0 },
      text: '',
      richMarkup: null,
      color: [0, 0, 0, 255],
      fontSize: null,
      fontFamily: null,
      visible: true,
      binding: {
        kind: 'expression-value',
        parameterName: sourceName,
        resultName,
        exprLabel: resultName,
        expr: parsedParameter(sourceName),
      },
      hotspots: [],
      screenSpace: false,
      debug: null,
    });

    const typedScene = structuredClone(baseScene) as any;
    typedScene.lineIterations = [{
      kind: 'rotate',
      visible: true,
      sourceIndex: 0,
      centerIndex: 0,
      angleExpr: { kind: 'constant', value: 30 },
      depth: 1,
      parameterName: null,
      depthParameterName: null,
      color: [0, 0, 0, 255],
      dashed: false,
      strokeWidth: 1,
    }];
    typedScene.labels = Array.from({ length: 6 }, (_, index) =>
      label(`p${index + 1}`, `p${index}`));
    const typedEnv = {
      sourceScene: typedScene,
      currentDynamics: () => ({
        parameters: [{ name: 'p0', value: 1, unit: null }],
        functions: [],
      }),
    } as any;
    const graph = createRuntime().describeDependencyGraph(typedEnv) as Array<{
      id: string;
      dependsOn: string[];
    }>;
    const lineIterationDeps = graph.find((node) => node.id === 'line-iteration:0')?.dependsOn || [];
    const deepestLabelDeps = graph.find((node) => node.id === 'label:5')?.dependsOn || [];

    const cyclicScene = structuredClone(baseScene) as any;
    const pointTemplate = cyclicScene.points[0];
    cyclicScene.points = [0, 1].map((index) => ({
      ...pointTemplate,
      binding: {
        kind: 'midpoint',
        startIndex: 1 - index,
        endIndex: 1 - index,
      },
      constraint: null,
    }));
    const cyclicEnv = {
      sourceScene: cyclicScene,
      currentDynamics: () => ({ parameters: [], functions: [] }),
    } as any;
    let cycleError = '';
    try {
      createRuntime().describeDependencyGraph(cyclicEnv);
    } catch (error) {
      cycleError = String(error);
    }
    return { lineIterationDeps, deepestLabelDeps, cycleError };
  });

  expect(result.lineIterationDeps).toContain('source-line:0');
  expect(result.deepestLabelDeps).toContain('param:p0');
  expect(result.cycleError).toContain('cyclic scene dependency graph');
});
