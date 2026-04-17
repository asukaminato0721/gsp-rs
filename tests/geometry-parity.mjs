import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const vectorsPath = path.join(repoRoot, "src/html/generated/geometry_parity_vectors.json");
const vectorFile = JSON.parse(fs.readFileSync(vectorsPath, "utf8"));
const modules = loadViewerModules(repoRoot);
const geometry = modules.geometry;
const scene = modules.scene;
const epsilon = 1e-6;

runPointCases("lerpPoint", vectorFile.lerpPoint, (testCase) =>
  geometry.lerpPoint(testCase.start, testCase.end, testCase.t),
);
runPointCases("rotateAround", vectorFile.rotateAround, (testCase) =>
  geometry.rotateAround(testCase.point, testCase.center, testCase.radians),
);
runPointCases("scaleAround", vectorFile.scaleAround, (testCase) =>
  geometry.scaleAround(testCase.point, testCase.center, testCase.factor),
);
runNullablePointCases("reflectAcrossLine", vectorFile.reflectAcrossLine, (testCase) =>
  geometry.reflectAcrossLine(testCase.point, testCase.lineStart, testCase.lineEnd),
);
runNullableSegmentCases("clipLineToBounds", vectorFile.clipLineToBounds, (testCase) =>
  geometry.clipLineToBounds(testCase.start, testCase.end, testCase.bounds),
);
runNullableSegmentCases("clipRayToBounds", vectorFile.clipRayToBounds, (testCase) =>
  geometry.clipRayToBounds(testCase.start, testCase.end, testCase.bounds),
);
runArcGeometryCases(vectorFile.threePointArcGeometry, (testCase) =>
  scene._threePointArcGeometry(testCase.start, testCase.mid, testCase.end),
);
runNullablePointCases("pointOnThreePointArc", vectorFile.pointOnThreePointArc, (testCase) =>
  scene.pointOnThreePointArc(testCase.start, testCase.mid, testCase.end, testCase.t),
);
runNullablePointCases(
  "pointOnThreePointArcComplement",
  vectorFile.pointOnThreePointArcComplement,
  (testCase) => scene._pointOnThreePointArcComplement(
    testCase.start,
    testCase.mid,
    testCase.end,
    testCase.t,
  ),
);
runNullablePointCases("pointOnCircleArc", vectorFile.pointOnCircleArc, (testCase) =>
  scene.pointOnCircleArc(testCase.center, testCase.start, testCase.end, testCase.t, false),
);

console.log("geometry parity passed");

function loadViewerModules(rootDir) {
  const context = vm.createContext({
    console,
    Math,
    Number,
    Array,
    Object,
    JSON,
    Map,
    Set,
    String,
    Boolean,
    Date,
  });
  context.window = { GspViewerModules: {} };
  context.globalThis = context;
  context.window.window = context.window;
  context.window.globalThis = context.window;

  const scripts = [
    "src/html/viewer_geometry.js",
    "src/html/viewer_scene_basic.js",
    "src/html/viewer_scene_circular.js",
  ];
  for (const relativePath of scripts) {
    const filename = path.join(rootDir, relativePath);
    const source = fs.readFileSync(filename, "utf8");
    vm.runInContext(source, context, { filename });
  }
  return context.window.GspViewerModules;
}

function runPointCases(label, cases, resolver) {
  for (const testCase of cases) {
    approxPoint(resolver(testCase), testCase.expected, `${label}:${testCase.name}`);
  }
}

function runNullablePointCases(label, cases, resolver) {
  for (const testCase of cases) {
    approxNullablePoint(resolver(testCase), testCase.expected, `${label}:${testCase.name}`);
  }
}

function runNullableSegmentCases(label, cases, resolver) {
  for (const testCase of cases) {
    approxNullableSegment(resolver(testCase), testCase.expected, `${label}:${testCase.name}`);
  }
}

function runArcGeometryCases(cases, resolver) {
  for (const testCase of cases) {
    approxNullableArcGeometry(
      resolver(testCase),
      testCase.expected,
      `threePointArcGeometry:${testCase.name}`,
    );
  }
}

function approxNullablePoint(actual, expected, label) {
  if (expected === null) {
    assert.equal(actual, null, `${label} expected null`);
    return;
  }
  assert.notEqual(actual, null, `${label} expected a point`);
  approxPoint(actual, expected, label);
}

function approxPoint(actual, expected, label) {
  approxNumber(actual.x, expected.x, `${label}.x`);
  approxNumber(actual.y, expected.y, `${label}.y`);
}

function approxNullableSegment(actual, expected, label) {
  if (expected === null) {
    assert.equal(actual, null, `${label} expected null`);
    return;
  }
  assert.ok(Array.isArray(actual), `${label} expected an array`);
  assert.equal(actual.length, 2, `${label} expected a 2-point segment`);
  approxPoint(actual[0], expected[0], `${label}[0]`);
  approxPoint(actual[1], expected[1], `${label}[1]`);
}

function approxNullableArcGeometry(actual, expected, label) {
  if (expected === null) {
    assert.equal(actual, null, `${label} expected null`);
    return;
  }
  assert.notEqual(actual, null, `${label} expected geometry`);
  approxPoint(actual.center, expected.center, `${label}.center`);
  approxNumber(actual.radius, expected.radius, `${label}.radius`);
  approxNumber(actual.startAngle, expected.startAngle, `${label}.startAngle`);
  approxNumber(actual.endAngle, expected.endAngle, `${label}.endAngle`);
  assert.equal(
    arcCounterclockwise(actual),
    expected.counterclockwise,
    `${label}.counterclockwise`,
  );
}

function arcCounterclockwise(geometry) {
  return geometry.ccwMid > geometry.ccwSpan + 1e-9;
}

function approxNumber(actual, expected, label) {
  assert.ok(
    Math.abs(actual - expected) <= epsilon,
    `${label} expected ${expected}, got ${actual}`,
  );
}
