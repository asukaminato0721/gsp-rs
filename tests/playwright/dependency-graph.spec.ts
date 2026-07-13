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

test('dependency graph consumes Rust metadata while WASM resolves deep chains and rejects cycles', async ({ page }) => {
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
    const sourceScene = window.gspDebug!.sourceScene;
    const env = {
      sourceScene,
      currentDynamics: () => ({ parameters: [], functions: [] }),
    } as any;
    const graph = createRuntime().describeDependencyGraph(env) as Array<{
      id: string;
      dependsOn: string[];
    }>;
    const exportedGraph = sourceScene.dependencyGraph.nodes;
    const exportedIds = exportedGraph.map((node) => node.id);
    const describedIds = graph.map((node) => node.id);

    const deepNodes = [
      { id: 'source-line:0', dependsOn: [] as string[] },
      ...Array.from({ length: 8 }, (_, index) => ({
        id: `node:${index}`,
        dependsOn: [index === 0 ? 'source-line:0' : `node:${index - 1}`],
      })),
    ];
    const deepPlan = window.GspRuntimeCore.createDependencyPlan(deepNodes);
    const affectedIds = deepPlan.affected(['source-line:0']).map((index) => deepNodes[index].id);

    let cycleError = '';
    try {
      window.GspRuntimeCore.createDependencyPlan([
        { id: 'left', dependsOn: ['right'] },
        { id: 'right', dependsOn: ['left'] },
      ]);
    } catch (error) {
      cycleError = String(error);
    }
    return { exportedIds, describedIds, affectedIds, cycleError };
  });

  expect(result.describedIds).toEqual(result.exportedIds);
  expect(result.affectedIds).toContain('node:7');
  expect(result.cycleError).toContain('cyclic scene dependency graph');
});
