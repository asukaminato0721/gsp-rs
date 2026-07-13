import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

function compilePointFixture(): string {
  const root = process.cwd();
  const source = path.resolve(root, 'tests/fixtures/gsp/static/point.gsp');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-runtime-core-'));
  const fixture = path.join(directory, 'point.gsp');
  fs.copyFileSync(source, fixture);
  execFileSync(path.resolve(root, 'target/debug/gsp-rs'), ['--html', fixture], {
    cwd: root,
    stdio: 'pipe',
  });
  return fixture.replace(/\.gsp$/i, '.html');
}

test('standalone file HTML evaluates expressions in the embedded Rust runtime core', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(String(error)));
  await page.goto(`file://${compilePointFixture()}`);

  const values = await page.evaluate(() => {
    const evaluate = window.GspViewerModules.dynamicsExpression!.evaluateExpr;
    const empty = new Map<string, number>();
    const parameter = (name: string, value: number) => ({ kind: 'parameter', name, value });
    const constant = (value: number) => ({ kind: 'constant', value });
    const parsed = (expr: object) => ({ kind: 'parsed', expr });
    return {
      constant: evaluate(constant(7) as any, 0, empty),
      identity: evaluate({ kind: 'identity' } as any, 2.5, empty),
      parameterDefault: evaluate(parsed(parameter('a', 3)) as any, 0, empty),
      parameterOverride: evaluate(parsed(parameter('a', 3)) as any, 0, new Map([['a', 9]])),
      degrees: evaluate(parsed({
        kind: 'unary',
        op: 'sin',
        expr: { kind: 'binary', lhs: { kind: 'pi-angle' }, op: 'div', rhs: constant(2) },
      }) as any, 0, empty),
      radians: evaluate(parsed({ kind: 'unary', op: 'cos', expr: { kind: 'variable' } }) as any, Math.PI, empty),
      tangentDiscontinuity: evaluate(parsed({ kind: 'unary', op: 'tan', expr: { kind: 'variable' } }) as any, Math.PI / 2, empty),
      divisionByZero: evaluate(parsed({ kind: 'binary', lhs: constant(1), op: 'div', rhs: constant(0) }) as any, 0, empty),
      invalidSqrt: evaluate(parsed({ kind: 'unary', op: 'sqrt', expr: constant(-1) }) as any, 0, empty),
      rustRounding: evaluate(parsed({ kind: 'unary', op: 'round', expr: constant(-1.5) }) as any, 0, empty),
      sampledFunction: window.GspRuntimeCore.sampleFunction(
        parsed({
          kind: 'binary',
          lhs: constant(1),
          op: 'div',
          rhs: { kind: 'variable' },
        }) as any,
        empty,
        -2,
        2,
        5,
        'cartesian',
      ),
      sampledParametric: window.GspRuntimeCore.sampleParametricCurve(
        { kind: 'identity' } as any,
        parsed({
          kind: 'binary',
          lhs: { kind: 'variable' },
          op: 'pow',
          rhs: constant(2),
        }) as any,
        empty,
        -1,
        1,
        3,
      ),
      polylineHit: window.GspRuntimeCore.linePolylineIntersection(
        { x: -3, y: 0 },
        { x: 3, y: 0 },
        'line',
        [
          { x: -2, y: -1 },
          { x: -1, y: 1 },
          { x: 1, y: -1 },
          { x: 2, y: 1 },
        ],
        null,
        1,
      ),
      distance: window.GspRuntimeCore.pointDistance({ x: 0, y: 0 }, { x: 3, y: 4 }, 2),
      ratio: window.GspRuntimeCore.pointDistanceRatio(
        { x: 0, y: 0 },
        { x: 2, y: 0 },
        { x: 3, y: 0 },
        true,
      ),
      angle: window.GspRuntimeCore.pointAngleDegrees(
        { x: 1, y: 0 },
        { x: 0, y: 0 },
        { x: 0, y: 1 },
      ),
      area: window.GspRuntimeCore.polygonArea(
        [{ x: 0, y: 0 }, { x: 4, y: 0 }, { x: 0, y: 3 }],
        1,
      ),
      sampledArc: window.GspRuntimeCore.sampleCircleArc(
        { x: 0, y: 0 },
        { x: 1, y: 0 },
        { x: 0, y: 1 },
        1,
        false,
      )?.map((point) => ({
        x: Math.abs(point.x) < 1e-12 ? 0 : point.x,
        y: Math.abs(point.y) < 1e-12 ? 0 : point.y,
      })),
      iteration: window.GspRuntimeCore.iterateExpression(
        parsed({
          kind: 'binary',
          lhs: parameter('n', 0),
          op: 'add',
          rhs: constant(1),
        }) as any,
        'n',
        0,
        empty,
        4,
      ),
      resolvedPoints: window.GspRuntimeCore.resolvePointConstraints([
        { x: 0, y: 0, constraint: null },
        { x: 0, y: 0, constraint: { kind: 'offset', originIndex: 0, dx: 4, dy: 2 } },
        { x: 0, y: 0, constraint: { kind: 'segment', startIndex: 0, endIndex: 1, t: 0.25 } },
        { x: 3, y: 5, constraint: { kind: 'line-function-intersection' } },
      ] as any, [1, 2, 3], false, new Map()),
    };
  });

  expect(errors).toEqual([]);
  expect(values).toEqual({
    constant: 7,
    identity: 2.5,
    parameterDefault: 3,
    parameterOverride: 9,
    degrees: 1,
    radians: -1,
    tangentDiscontinuity: null,
    divisionByZero: null,
    invalidSqrt: null,
    rustRounding: -2,
    sampledFunction: [
      [{ x: -2, y: -0.5 }, { x: -1, y: -1 }],
      [{ x: 1, y: 1 }, { x: 2, y: 0.5 }],
    ],
    sampledParametric: [
      { x: -1, y: 1 },
      { x: 0, y: 0 },
      { x: 1, y: 1 },
    ],
    polylineHit: { x: 0, y: 0 },
    distance: 10,
    ratio: 1,
    angle: 90,
    area: 6,
    sampledArc: [
      { x: 1, y: 0 },
      { x: 0, y: 1 },
    ],
    iteration: [1, 2, 3, 4],
    resolvedPoints: [
      { x: 0, y: 0 },
      { x: 4, y: 2 },
      { x: 1, y: 0.5 },
      null,
    ],
  });
});
