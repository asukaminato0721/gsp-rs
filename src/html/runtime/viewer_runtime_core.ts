(function() {
  type RuntimeCoreWasmExports = {
    memory: WebAssembly.Memory;
    gsp_runtime_abi_version: () => number;
    gsp_normalize_angle_delta: (from: number, to: number) => number;
    gsp_lerp_point: (startX: number, startY: number, endX: number, endY: number, t: number) => number;
    gsp_rotate_around: (pointX: number, pointY: number, centerX: number, centerY: number, radians: number) => number;
    gsp_scale_around: (pointX: number, pointY: number, centerX: number, centerY: number, factor: number) => number;
    gsp_reflect_across_line: (pointX: number, pointY: number, startX: number, startY: number, endX: number, endY: number) => number;
    gsp_project_to_line_like: (pointX: number, pointY: number, startX: number, startY: number, endX: number, endY: number, lineKind: number) => number;
    gsp_angle_bisector_direction: (startX: number, startY: number, vertexX: number, vertexY: number, endX: number, endY: number) => number;
    gsp_measured_rotation_radians: (startX: number, startY: number, vertexX: number, vertexY: number, endX: number, endY: number) => number;
    gsp_scale_by_three_point_ratio: (sourceX: number, sourceY: number, centerX: number, centerY: number, originX: number, originY: number, denominatorX: number, denominatorY: number, numeratorX: number, numeratorY: number, signed: number, clampToUnit: number) => number;
    gsp_clip_line_to_bounds: (startX: number, startY: number, endX: number, endY: number, minX: number, maxX: number, minY: number, maxY: number) => number;
    gsp_clip_ray_to_bounds: (startX: number, startY: number, endX: number, endY: number, minX: number, maxX: number, minY: number, maxY: number) => number;
    gsp_three_point_arc_geometry: (startX: number, startY: number, midX: number, midY: number, endX: number, endY: number) => number;
    gsp_point_on_three_point_arc: (startX: number, startY: number, midX: number, midY: number, endX: number, endY: number, t: number, complement: number) => number;
    gsp_circle_arc_control_points: (centerX: number, centerY: number, startX: number, startY: number, endX: number, endY: number, yUp: number) => number;
    gsp_point_on_circle_arc: (centerX: number, centerY: number, startX: number, startY: number, endX: number, endY: number, t: number, yUp: number) => number;
    gsp_project_to_three_point_arc: (pointX: number, pointY: number, startX: number, startY: number, midX: number, midY: number, endX: number, endY: number) => number;
    gsp_project_to_circle_arc: (pointX: number, pointY: number, centerX: number, centerY: number, startX: number, startY: number, endX: number, endY: number, yUp: number) => number;
    gsp_line_line_intersection: (leftStartX: number, leftStartY: number, leftEndX: number, leftEndY: number, leftKind: number, rightStartX: number, rightStartY: number, rightEndX: number, rightEndY: number, rightKind: number) => number;
    gsp_line_circle_intersections: (startX: number, startY: number, endX: number, endY: number, lineKind: number, centerX: number, centerY: number, radius: number) => number;
    gsp_circle_circle_intersections: (leftX: number, leftY: number, leftRadius: number, rightX: number, rightY: number, rightRadius: number) => number;
    gsp_point_circle_tangents: (pointX: number, pointY: number, centerX: number, centerY: number, radius: number) => number;
    gsp_compile_dependency_plan: (pointer: number, length: number) => number;
    gsp_evaluate_object_graph: (pointer: number, length: number) => number;
    gsp_json_result_ptr: () => number;
    gsp_json_result_len: () => number;
    gsp_inverse_point_transform: (pointer: number, length: number) => number;
    gsp_dependency_topo_order: (handle: number) => number;
    gsp_dependency_affected: (handle: number, rootsPointer: number, rootsLength: number) => number;
    gsp_last_error_ptr: () => number;
    gsp_last_error_len: () => number;
    gsp_sample_expression: (handle: number, xMin: number, xMax: number, sampleCount: number, plotMode: number) => number;
    gsp_sample_parametric_curve: (xHandle: number, yHandle: number, valueMin: number, valueMax: number, sampleCount: number) => number;
    gsp_sample_coordinate_trace: (xHandle: number, yHandle: number, xParameterIndex: number, yParameterIndex: number, sourceX: number, sourceY: number, valueMin: number, valueMax: number, sampleCount: number, useMidpoints: number, mode: number) => number;
    gsp_sample_custom_transform_trace: (distanceHandle: number, angleHandle: number, originX: number, originY: number, axisEndX: number, axisEndY: number, valueMin: number, valueMax: number, traceMax: number, sampleCount: number, distanceScale: number, angleDegreesScale: number) => number;
    gsp_sample_circle_arc: (centerX: number, centerY: number, startX: number, startY: number, endX: number, endY: number, steps: number, yUp: number) => number;
    gsp_sample_three_point_arc: (startX: number, startY: number, midX: number, midY: number, endX: number, endY: number, steps: number, complement: number) => number;
    gsp_translation_iteration_deltas: (depth: number, primaryDx: number, primaryDy: number, secondaryDx: number, secondaryDy: number, hasSecondary: number, bidirectional: number, includeOrigin: number) => number;
    gsp_rotate_iteration_points: (pointsPointer: number, pointCount: number, centerX: number, centerY: number, angleRadians: number, depth: number) => number;
    gsp_affine_iteration_segment: (pointsPointer: number, pointCount: number, depth: number) => number;
    gsp_branching_iteration_segments: (pointsPointer: number, pointCount: number, depth: number) => number;
    gsp_line_polyline_intersection: (lineStartX: number, lineStartY: number, lineEndX: number, lineEndY: number, lineKind: number, pointsPointer: number, pointCount: number, sampleHint: number, variant: number) => number;
    gsp_choose_point_candidate: (pointsPointer: number, pointCount: number, referenceX: number, referenceY: number, hasReference: number, variant: number) => number;
    gsp_line_circle_intersection_candidate: (startX: number, startY: number, endX: number, endY: number, lineKind: number, centerX: number, centerY: number, radius: number, variant: number) => number;
    gsp_point_distance: (leftX: number, leftY: number, rightX: number, rightY: number, valueScale: number) => number;
    gsp_point_distance_ratio: (originX: number, originY: number, denominatorX: number, denominatorY: number, numeratorX: number, numeratorY: number, clampToUnit: number) => number;
    gsp_point_angle_degrees: (startX: number, startY: number, vertexX: number, vertexY: number, endX: number, endY: number) => number;
    gsp_polygon_area: (pointsPointer: number, pointCount: number, valueScale: number) => number;
    gsp_batch_result_ptr: () => number;
    gsp_batch_result_len: () => number;
    gsp_geometry_result_x: (index: number) => number;
    gsp_geometry_result_y: (index: number) => number;
    gsp_geometry_result_scalar: (index: number) => number;
    gsp_alloc_bytes: (length: number) => number;
    gsp_free_bytes: (pointer: number, length: number) => void;
    gsp_compile_expression: (pointer: number, length: number) => number;
    gsp_expression_parameter_count: (handle: number) => number;
    gsp_expression_parameter_name_ptr: (handle: number, index: number) => number;
    gsp_expression_parameter_name_len: (handle: number, index: number) => number;
    gsp_expression_set_parameter: (handle: number, index: number, value: number) => number;
    gsp_evaluate_expression: (handle: number, x: number) => number;
    gsp_evaluate_expression_with_driver: (handle: number, x: number, driverValue: number) => number;
    gsp_iterate_expression: (handle: number, parameterIndex: number, initialValue: number, count: number, x: number) => number;
  };

  type CompiledExpression = {
    handle: number;
    parameterNames: string[];
  };

  type RuntimeDependencyNodeInput = {
    id: string;
    dependsOn: string[];
  };

  type RuntimeDependencyPlan = {
    topoOrder: number[];
    affected: (dirtyRootIds: string[]) => number[];
  };

  const wasmElement = document.getElementById("gsp-runtime-core-wasm");
  if (!(wasmElement instanceof HTMLScriptElement)) {
    throw new Error("gsp-rs runtime core payload is missing");
  }
  if (typeof WebAssembly !== "object") {
    throw new Error("This browser does not support the gsp-rs WebAssembly runtime");
  }

  const encoded = (wasmElement.textContent || "").replace(/\s/g, "");
  const decoded = atob(encoded);
  const bytes = new Uint8Array(decoded.length);
  for (let index = 0; index < decoded.length; index += 1) {
    bytes[index] = decoded.charCodeAt(index);
  }

  const module = new WebAssembly.Module(bytes);
  const instance = new WebAssembly.Instance(module, {});
  const wasm = instance.exports as unknown as RuntimeCoreWasmExports;
  if (wasm.gsp_runtime_abi_version() !== 7) {
    throw new Error("Unsupported gsp-rs runtime core ABI");
  }

  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8");
  const expressionCache = new WeakMap<object, CompiledExpression>();

  function evaluateObjectGraph(input: unknown): unknown[] {
    const bytes = encoder.encode(JSON.stringify(input));
    const pointer = wasm.gsp_alloc_bytes(bytes.length);
    if (!pointer) throw new Error("Unable to allocate object graph input");
    new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
    try {
      const length = wasm.gsp_evaluate_object_graph(pointer, bytes.length);
      if (!length) {
        throw new Error(lastRuntimeError() || "Object graph evaluation failed");
      }
      const outputPointer = wasm.gsp_json_result_ptr();
      const outputLength = wasm.gsp_json_result_len();
      if (!outputPointer || outputLength !== length) {
        throw new Error("Object graph returned an invalid JSON result");
      }
      return JSON.parse(decoder.decode(
        new Uint8Array(wasm.memory.buffer, outputPointer, outputLength),
      ));
    } finally {
      wasm.gsp_free_bytes(pointer, bytes.length);
    }
  }

  function lineKindCode(kind: string): number {
    if (kind === "segment") return 0;
    if (kind === "line") return 1;
    if (kind === "ray") return 2;
    throw new Error(`Unsupported line kind: ${kind}`);
  }

  function geometryResults(count: number): Point[] {
    const points: Point[] = [];
    for (let index = 0; index < count; index += 1) {
      const x = wasm.gsp_geometry_result_x(index);
      const y = wasm.gsp_geometry_result_y(index);
      if (Number.isFinite(x) && Number.isFinite(y)) points.push({ x, y });
    }
    return points;
  }

  function geometryResult(count: number): Point | null {
    return geometryResults(count)[0] ?? null;
  }

  function lerpPoint(start: Point, end: Point, t: number): Point {
    return geometryResult(wasm.gsp_lerp_point(start.x, start.y, end.x, end.y, t))!;
  }

  function rotateAround(point: Point, center: Point, radians: number): Point {
    return geometryResult(wasm.gsp_rotate_around(point.x, point.y, center.x, center.y, radians))!;
  }

  function scaleAround(point: Point, center: Point, factor: number): Point {
    return geometryResult(wasm.gsp_scale_around(point.x, point.y, center.x, center.y, factor))!;
  }

  function reflectAcrossLine(point: Point, lineStart: Point, lineEnd: Point): Point | null {
    return geometryResult(wasm.gsp_reflect_across_line(
      point.x,
      point.y,
      lineStart.x,
      lineStart.y,
      lineEnd.x,
      lineEnd.y,
    ));
  }

  function projectionResult(count: number): RuntimeProjection | null {
    const projected = geometryResult(count);
    if (!projected) return null;
    const t = wasm.gsp_geometry_result_scalar(0);
    const distanceSquared = wasm.gsp_geometry_result_scalar(1);
    return Number.isFinite(t) && Number.isFinite(distanceSquared)
      ? { t, projected, distanceSquared }
      : null;
  }

  function projectToLineLike(point: Point, start: Point, end: Point, kind: RuntimeLineKind): RuntimeProjection | null {
    return projectionResult(wasm.gsp_project_to_line_like(
      point.x, point.y, start.x, start.y, end.x, end.y, lineKindCode(kind),
    ));
  }

  function angleBisectorDirection(start: Point, vertex: Point, end: Point): Point | null {
    return geometryResult(wasm.gsp_angle_bisector_direction(
      start.x, start.y, vertex.x, vertex.y, end.x, end.y,
    ));
  }

  function measuredRotationRadians(start: Point, vertex: Point, end: Point): number | null {
    const value = wasm.gsp_measured_rotation_radians(
      start.x, start.y, vertex.x, vertex.y, end.x, end.y,
    );
    return Number.isFinite(value) ? value : null;
  }

  function scaleByThreePointRatio(
    source: Point,
    center: Point,
    ratioOrigin: Point,
    ratioDenominator: Point,
    ratioNumerator: Point,
    signed: boolean,
    clampToUnit: boolean,
  ): Point | null {
    return geometryResult(wasm.gsp_scale_by_three_point_ratio(
      source.x, source.y, center.x, center.y, ratioOrigin.x, ratioOrigin.y,
      ratioDenominator.x, ratioDenominator.y, ratioNumerator.x, ratioNumerator.y,
      signed ? 1 : 0, clampToUnit ? 1 : 0,
    ));
  }

  function clipToBounds(
    start: Point,
    end: Point,
    bounds: RuntimeBounds,
    rayOnly: boolean,
  ): Point[] | null {
    const clip = rayOnly ? wasm.gsp_clip_ray_to_bounds : wasm.gsp_clip_line_to_bounds;
    const points = geometryResults(clip(
      start.x, start.y, end.x, end.y,
      bounds.minX, bounds.maxX, bounds.minY, bounds.maxY,
    ));
    return points.length === 2 ? points : null;
  }

  function threePointArcGeometry(start: Point, mid: Point, end: Point): RuntimeArcGeometry | null {
    const center = geometryResult(wasm.gsp_three_point_arc_geometry(
      start.x, start.y, mid.x, mid.y, end.x, end.y,
    ));
    if (!center) return null;
    const [radius, startAngle, midAngle, endAngle, ccwSpan, ccwMid] =
      Array.from({ length: 6 }, (_, index) => wasm.gsp_geometry_result_scalar(index));
    if (![radius, startAngle, midAngle, endAngle, ccwSpan, ccwMid].every(Number.isFinite)) return null;
    return { start, mid, end, center, radius, startAngle, midAngle, endAngle, ccwSpan, ccwMid };
  }

  function pointOnThreePointArc(start: Point, mid: Point, end: Point, t: number, complement: boolean): Point | null {
    return geometryResult(wasm.gsp_point_on_three_point_arc(
      start.x, start.y, mid.x, mid.y, end.x, end.y, t, complement ? 1 : 0,
    ));
  }

  function circleArcControlPoints(center: Point, start: Point, end: Point, yUp: boolean): [Point, Point, Point] | null {
    const points = geometryResults(wasm.gsp_circle_arc_control_points(
      center.x, center.y, start.x, start.y, end.x, end.y, yUp ? 1 : 0,
    ));
    return points.length === 3 ? [points[0], points[1], points[2]] : null;
  }

  function pointOnCircleArc(center: Point, start: Point, end: Point, t: number, yUp: boolean): Point | null {
    return geometryResult(wasm.gsp_point_on_circle_arc(
      center.x, center.y, start.x, start.y, end.x, end.y, t, yUp ? 1 : 0,
    ));
  }

  function projectToThreePointArc(point: Point, start: Point, mid: Point, end: Point): RuntimeProjection | null {
    return projectionResult(wasm.gsp_project_to_three_point_arc(
      point.x, point.y, start.x, start.y, mid.x, mid.y, end.x, end.y,
    ));
  }

  function projectToCircleArc(point: Point, center: Point, start: Point, end: Point, yUp: boolean): RuntimeProjection | null {
    return projectionResult(wasm.gsp_project_to_circle_arc(
      point.x, point.y, center.x, center.y, start.x, start.y, end.x, end.y, yUp ? 1 : 0,
    ));
  }

  function lineLineIntersection(
    leftStart: Point,
    leftEnd: Point,
    leftKind: string,
    rightStart: Point,
    rightEnd: Point,
    rightKind: string,
  ): Point | null {
    return geometryResult(wasm.gsp_line_line_intersection(
      leftStart.x,
      leftStart.y,
      leftEnd.x,
      leftEnd.y,
      lineKindCode(leftKind),
      rightStart.x,
      rightStart.y,
      rightEnd.x,
      rightEnd.y,
      lineKindCode(rightKind),
    ));
  }

  function lineCircleIntersections(
    start: Point,
    end: Point,
    lineKind: string,
    center: Point,
    radius: number,
  ): Point[] {
    return geometryResults(wasm.gsp_line_circle_intersections(
      start.x,
      start.y,
      end.x,
      end.y,
      lineKindCode(lineKind),
      center.x,
      center.y,
      radius,
    ));
  }

  function circleCircleIntersections(
    leftCenter: Point,
    leftRadius: number,
    rightCenter: Point,
    rightRadius: number,
  ): Point[] {
    return geometryResults(wasm.gsp_circle_circle_intersections(
      leftCenter.x,
      leftCenter.y,
      leftRadius,
      rightCenter.x,
      rightCenter.y,
      rightRadius,
    ));
  }

  function pointCircleTangents(point: Point, center: Point, radius: number): Point[] {
    return geometryResults(wasm.gsp_point_circle_tangents(
      point.x,
      point.y,
      center.x,
      center.y,
      radius,
    ));
  }

  function withInputBytes<T>(bytes: Uint8Array, callback: (pointer: number) => T): T {
    const pointer = wasm.gsp_alloc_bytes(bytes.length);
    if (!pointer) throw new Error("Unable to allocate gsp-rs runtime core input");
    try {
      new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
      return callback(pointer);
    } finally {
      wasm.gsp_free_bytes(pointer, bytes.length);
    }
  }

  function batchScalars(expectedLength: number): number[] {
    const length = wasm.gsp_batch_result_len();
    if (length !== expectedLength || length === 0) return [];
    const pointer = wasm.gsp_batch_result_ptr();
    if (!pointer) return [];
    return Array.from(new Float64Array(wasm.memory.buffer, pointer, length));
  }

  function batchPoints(expectedLength: number): Point[] {
    const values = batchScalars(expectedLength);
    const points: Point[] = [];
    for (let index = 0; index + 1 < values.length; index += 2) {
      const x = values[index];
      const y = values[index + 1];
      if (Number.isFinite(x) && Number.isFinite(y)) points.push({ x, y });
    }
    return points;
  }

  function batchIndices(expectedLength: number): number[] {
    return batchScalars(expectedLength).filter((value) => Number.isInteger(value) && value >= 0);
  }

  function lastRuntimeError(): string {
    const length = wasm.gsp_last_error_len();
    const pointer = wasm.gsp_last_error_ptr();
    return pointer && length
      ? decoder.decode(new Uint8Array(wasm.memory.buffer, pointer, length))
      : "Unable to compile gsp-rs dependency plan";
  }

  function createDependencyPlan(nodes: RuntimeDependencyNodeInput[]): RuntimeDependencyPlan {
    const encoded = encoder.encode(JSON.stringify(
      nodes.map((node) => ({ id: node.id, dependsOn: node.dependsOn })),
    ));
    const handle = withInputBytes(encoded, (pointer) =>
      wasm.gsp_compile_dependency_plan(pointer, encoded.length));
    if (!handle) throw new Error(lastRuntimeError());
    const topoOrder = batchIndices(wasm.gsp_dependency_topo_order(handle));
    if (topoOrder.length !== nodes.length) {
      throw new Error("Incomplete gsp-rs dependency plan");
    }
    return {
      topoOrder,
      affected(dirtyRootIds: string[]) {
        const roots = encoder.encode(JSON.stringify(dirtyRootIds));
        return withInputBytes(roots, (pointer) =>
          batchIndices(wasm.gsp_dependency_affected(handle, pointer, roots.length)));
      },
    };
  }

  function inversePointTransform(
    world: Point,
    matrixApply: PointTransformJson[],
    points: RuntimeScenePointJson[],
    parameters: Map<string, number>,
  ): Point | null {
    const input = encoder.encode(JSON.stringify({
      world,
      matrixApply,
      points,
      parameters: Object.fromEntries(parameters),
    }));
    return withInputBytes(input, (pointer) =>
      geometryResult(wasm.gsp_inverse_point_transform(pointer, input.length)));
  }

  function setExpressionParameters(compiled: CompiledExpression, parameters: Map<string, number>) {
    for (let index = 0; index < compiled.parameterNames.length; index += 1) {
      const value = parameters.get(compiled.parameterNames[index]);
      wasm.gsp_expression_set_parameter(compiled.handle, index, value ?? Number.NaN);
    }
  }

  function sampleFunction(
    expr: FunctionExprJson | FunctionAstJson,
    parameters: Map<string, number>,
    xMin: number,
    xMax: number,
    sampleCount: number,
    plotMode: "cartesian" | "polar",
  ): Point[][] {
    const compiled = compileExpression(expr);
    setExpressionParameters(compiled, parameters);
    const length = wasm.gsp_sample_expression(
      compiled.handle,
      xMin,
      xMax,
      sampleCount,
      plotMode === "polar" ? 1 : 0,
    );
    const values = batchScalars(length);
    const segments: Point[][] = [];
    let segment: Point[] = [];
    for (let index = 0; index + 1 < values.length; index += 2) {
      const x = values[index];
      const y = values[index + 1];
      if (Number.isFinite(x) && Number.isFinite(y)) {
        segment.push({ x, y });
      } else {
        if (segment.length >= 2) segments.push(segment);
        segment = [];
      }
    }
    if (segment.length >= 2) segments.push(segment);
    return segments;
  }

  function sampleParametricCurve(
    xExpr: FunctionExprJson | FunctionAstJson,
    yExpr: FunctionExprJson | FunctionAstJson,
    parameters: Map<string, number>,
    valueMin: number,
    valueMax: number,
    sampleCount: number,
  ): Point[] {
    const compiledX = compileExpression(xExpr);
    const compiledY = compileExpression(yExpr);
    setExpressionParameters(compiledX, parameters);
    setExpressionParameters(compiledY, parameters);
    const length = wasm.gsp_sample_parametric_curve(
      compiledX.handle,
      compiledY.handle,
      valueMin,
      valueMax,
      sampleCount,
    );
    return batchPoints(length);
  }

  function sampleCoordinateTrace(
    xExpr: FunctionExprJson | FunctionAstJson,
    yExpr: FunctionExprJson | FunctionAstJson | null,
    parameters: Map<string, number>,
    xParameterName: string | null,
    yParameterName: string | null,
    source: Point,
    valueMin: number,
    valueMax: number,
    sampleCount: number,
    useMidpoints: boolean,
    mode: "horizontal" | "vertical" | "two-dimensional",
  ): Point[] {
    const compiledX = compileExpression(xExpr);
    const compiledY = yExpr ? compileExpression(yExpr) : null;
    setExpressionParameters(compiledX, parameters);
    if (compiledY) setExpressionParameters(compiledY, parameters);
    const xParameterIndex = xParameterName
      ? compiledX.parameterNames.indexOf(xParameterName)
      : -1;
    const yParameterIndex = yParameterName && compiledY
      ? compiledY.parameterNames.indexOf(yParameterName)
      : -1;
    const modeCode = mode === "horizontal" ? 0 : mode === "vertical" ? 1 : 2;
    const length = wasm.gsp_sample_coordinate_trace(
      compiledX.handle,
      compiledY?.handle ?? 0,
      xParameterIndex >= 0 ? xParameterIndex : 0xffffffff,
      yParameterIndex >= 0 ? yParameterIndex : 0xffffffff,
      source.x,
      source.y,
      valueMin,
      valueMax,
      sampleCount,
      useMidpoints ? 1 : 0,
      modeCode,
    );
    return batchPoints(length);
  }

  function sampleCustomTransformTrace(
    distanceExpr: FunctionExprJson | FunctionAstJson,
    angleExpr: FunctionExprJson | FunctionAstJson,
    parameters: Map<string, number>,
    origin: Point,
    axisEnd: Point,
    valueMin: number,
    valueMax: number,
    traceMax: number,
    sampleCount: number,
    distanceScale: number,
    angleDegreesScale: number,
  ): Point[] {
    const distance = compileExpression(distanceExpr);
    const angle = compileExpression(angleExpr);
    setExpressionParameters(distance, parameters);
    setExpressionParameters(angle, parameters);
    const length = wasm.gsp_sample_custom_transform_trace(
      distance.handle,
      angle.handle,
      origin.x,
      origin.y,
      axisEnd.x,
      axisEnd.y,
      valueMin,
      valueMax,
      traceMax,
      sampleCount,
      distanceScale,
      angleDegreesScale,
    );
    return batchPoints(length);
  }

  function customTransformPoint(
    distanceExpr: FunctionExprJson | FunctionAstJson,
    angleExpr: FunctionExprJson | FunctionAstJson,
    parameters: Map<string, number>,
    origin: Point,
    axisEnd: Point,
    value: number,
    distanceScale: number,
    angleDegreesScale: number,
  ): Point | null {
    return sampleCustomTransformTrace(
      distanceExpr,
      angleExpr,
      parameters,
      origin,
      axisEnd,
      value,
      value,
      value,
      1,
      distanceScale,
      angleDegreesScale,
    )[0] ?? null;
  }

  function sampleCircleArc(center: Point, start: Point, end: Point, steps: number, yUp: boolean): Point[] | null {
    const length = wasm.gsp_sample_circle_arc(
      center.x, center.y, start.x, start.y, end.x, end.y, steps, yUp ? 1 : 0,
    );
    const points = batchPoints(length);
    return points.length === steps + 1 ? points : null;
  }

  function sampleThreePointArc(start: Point, mid: Point, end: Point, steps: number, complement: boolean): Point[] | null {
    const length = wasm.gsp_sample_three_point_arc(
      start.x, start.y, mid.x, mid.y, end.x, end.y, steps, complement ? 1 : 0,
    );
    const points = batchPoints(length);
    return points.length === steps + 1 ? points : null;
  }

  function translationIterationDeltas(
    depth: number,
    primary: Point,
    secondary: Point | null,
    bidirectional: boolean,
    includeOrigin: boolean,
  ): Point[] {
    const length = wasm.gsp_translation_iteration_deltas(
      depth,
      primary.x,
      primary.y,
      secondary?.x ?? 0,
      secondary?.y ?? 0,
      secondary ? 1 : 0,
      bidirectional ? 1 : 0,
      includeOrigin ? 1 : 0,
    );
    return batchPoints(length);
  }

  function rotateIterationPoints(
    points: Point[],
    center: Point,
    angleRadians: number,
    depth: number,
  ): Point[][] {
    if (points.length === 0 || depth <= 0) return [];
    return withInputPoints(points, (pointer) => {
      const length = wasm.gsp_rotate_iteration_points(
        pointer,
        points.length,
        center.x,
        center.y,
        angleRadians,
        depth,
      );
      const output = batchPoints(length);
      return Array.from({ length: depth }, (_, index) =>
        output.slice(index * points.length, (index + 1) * points.length));
    });
  }

  function pointsAsSegments(points: Point[]): [Point, Point][] {
    const segments: [Point, Point][] = [];
    for (let index = 0; index + 1 < points.length; index += 2) {
      segments.push([points[index], points[index + 1]]);
    }
    return segments;
  }

  function affineIterationSegments(
    start: Point,
    end: Point,
    sourceTriangle: [Point, Point, Point],
    targetTriangle: [Point, Point, Point],
    depth: number,
  ): [Point, Point][] | null {
    return withInputPoints(
      [start, end, ...sourceTriangle, ...targetTriangle],
      (pointer) => {
        const length = wasm.gsp_affine_iteration_segment(pointer, 8, depth);
        const points = batchPoints(length);
        return points.length === depth * 2 ? pointsAsSegments(points) : null;
      },
    );
  }

  function branchingIterationSegments(
    start: Point,
    end: Point,
    targetSegments: [Point, Point][],
    depth: number,
  ): [Point, Point][] | null {
    if (targetSegments.length === 0) return null;
    const input = [start, end, ...targetSegments.flat()];
    return withInputPoints(input, (pointer) => {
      const length = wasm.gsp_branching_iteration_segments(pointer, input.length, depth);
      const points = batchPoints(length);
      return points.length > 0 || depth === 0 ? pointsAsSegments(points) : null;
    });
  }

  function withInputPoints<T>(points: Point[], callback: (pointer: number) => T): T {
    const byteLength = points.length * 16;
    const pointer = wasm.gsp_alloc_bytes(byteLength);
    if (!pointer) throw new Error("Unable to allocate gsp-rs runtime core point input");
    try {
      const view = new DataView(wasm.memory.buffer, pointer, byteLength);
      points.forEach((point, index) => {
        view.setFloat64(index * 16, point.x, true);
        view.setFloat64(index * 16 + 8, point.y, true);
      });
      return callback(pointer);
    } finally {
      wasm.gsp_free_bytes(pointer, byteLength);
    }
  }

  function linePolylineIntersection(
    lineStart: Point,
    lineEnd: Point,
    lineKind: RuntimeLineKind,
    points: Point[],
    sampleHint: number | null,
    variant: number,
  ): Point | null {
    if (points.length < 2 || !Number.isInteger(variant) || variant < 0) return null;
    return withInputPoints(points, (pointer) => geometryResult(wasm.gsp_line_polyline_intersection(
      lineStart.x,
      lineStart.y,
      lineEnd.x,
      lineEnd.y,
      lineKindCode(lineKind),
      pointer,
      points.length,
      typeof sampleHint === "number" && Number.isFinite(sampleHint) ? sampleHint : Number.NaN,
      variant,
    )));
  }

  function choosePointCandidate(
    candidates: Point[],
    reference: Point | null,
    variant: number,
  ): Point | null {
    if (candidates.length === 0 || !Number.isInteger(variant) || variant < 0) return null;
    const hasReference = !!reference && Number.isFinite(reference.x) && Number.isFinite(reference.y);
    return withInputPoints(candidates, (pointer) => geometryResult(wasm.gsp_choose_point_candidate(
      pointer,
      candidates.length,
      hasReference ? reference.x : 0,
      hasReference ? reference.y : 0,
      hasReference ? 1 : 0,
      variant,
    )));
  }

  function lineCircleIntersectionCandidate(
    start: Point,
    end: Point,
    lineKind: RuntimeLineKind,
    center: Point,
    radius: number,
    variant: number,
  ): Point | null {
    if (!Number.isInteger(variant) || variant < 0) return null;
    return geometryResult(wasm.gsp_line_circle_intersection_candidate(
      start.x,
      start.y,
      end.x,
      end.y,
      lineKindCode(lineKind),
      center.x,
      center.y,
      radius,
      variant,
    ));
  }

  function pointDistance(left: Point, right: Point, valueScale: number): number | null {
    const value = wasm.gsp_point_distance(left.x, left.y, right.x, right.y, valueScale);
    return Number.isFinite(value) ? value : null;
  }

  function pointDistanceRatio(origin: Point, denominator: Point, numerator: Point, clampToUnit: boolean): number | null {
    const value = wasm.gsp_point_distance_ratio(
      origin.x, origin.y, denominator.x, denominator.y, numerator.x, numerator.y,
      clampToUnit ? 1 : 0,
    );
    return Number.isFinite(value) ? value : null;
  }

  function pointAngleDegrees(start: Point, vertex: Point, end: Point): number | null {
    const value = wasm.gsp_point_angle_degrees(start.x, start.y, vertex.x, vertex.y, end.x, end.y);
    return Number.isFinite(value) ? value : null;
  }

  function polygonArea(points: Point[], valueScale: number): number | null {
    if (points.length < 3) return null;
    return withInputPoints(points, (pointer) => {
      const value = wasm.gsp_polygon_area(pointer, points.length, valueScale);
      return Number.isFinite(value) ? value : null;
    });
  }

  function compileExpression(expr: FunctionExprJson | FunctionAstJson): CompiledExpression {
    const cached = expressionCache.get(expr);
    if (cached) return cached;

    const json = encoder.encode(JSON.stringify(expr));
    const pointer = wasm.gsp_alloc_bytes(json.length);
    if (!pointer) throw new Error("Unable to allocate gsp-rs runtime core input");
    let handle = 0;
    try {
      new Uint8Array(wasm.memory.buffer, pointer, json.length).set(json);
      handle = wasm.gsp_compile_expression(pointer, json.length);
    } finally {
      wasm.gsp_free_bytes(pointer, json.length);
    }
    if (!handle) throw new Error("Invalid expression in gsp-rs payload");

    const parameterNames: string[] = [];
    const parameterCount = wasm.gsp_expression_parameter_count(handle);
    for (let index = 0; index < parameterCount; index += 1) {
      const namePointer = wasm.gsp_expression_parameter_name_ptr(handle, index);
      const nameLength = wasm.gsp_expression_parameter_name_len(handle, index);
      const nameBytes = new Uint8Array(wasm.memory.buffer, namePointer, nameLength);
      parameterNames.push(decoder.decode(nameBytes));
    }
    const compiled = { handle, parameterNames };
    expressionCache.set(expr, compiled);
    return compiled;
  }

  function evaluateExpr(
    expr: FunctionExprJson | FunctionAstJson,
    x: number,
    parameters: Map<string, number>,
  ): number | null {
    const compiled = compileExpression(expr);
    setExpressionParameters(compiled, parameters);
    const value = wasm.gsp_evaluate_expression(compiled.handle, x);
    return Number.isFinite(value) ? value : null;
  }

  function evaluateExprWithDriver(
    expr: FunctionExprJson | FunctionAstJson,
    x: number,
    parameters: Map<string, number>,
    driverValue: number,
  ): number | null {
    const compiled = compileExpression(expr);
    setExpressionParameters(compiled, parameters);
    const value = wasm.gsp_evaluate_expression_with_driver(compiled.handle, x, driverValue);
    return Number.isFinite(value) ? value : null;
  }

  function iterateExpression(
    expr: FunctionExprJson | FunctionAstJson,
    parameterName: string,
    initialValue: number,
    parameters: Map<string, number>,
    count: number,
  ): number[] {
    const compiled = compileExpression(expr);
    setExpressionParameters(compiled, parameters);
    const parameterIndex = compiled.parameterNames.indexOf(parameterName);
    const length = wasm.gsp_iterate_expression(
      compiled.handle,
      parameterIndex >= 0 ? parameterIndex : 0xffffffff,
      initialValue,
      count,
      0,
    );
    return batchScalars(length);
  }

  window.GspRuntimeCore = {
    normalizeAngleDelta: wasm.gsp_normalize_angle_delta,
    lerpPoint,
    rotateAround,
    scaleAround,
    reflectAcrossLine,
    projectToLineLike,
    angleBisectorDirection,
    measuredRotationRadians,
    scaleByThreePointRatio,
    clipLineToBounds: (start, end, bounds) => clipToBounds(start, end, bounds, false),
    clipRayToBounds: (start, end, bounds) => clipToBounds(start, end, bounds, true),
    threePointArcGeometry,
    pointOnThreePointArc,
    circleArcControlPoints,
    pointOnCircleArc,
    projectToThreePointArc,
    projectToCircleArc,
    lineLineIntersection,
    lineCircleIntersections,
    circleCircleIntersections,
    pointCircleTangents,
    createDependencyPlan,
    evaluateObjectGraph,
    inversePointTransform,
    sampleFunction,
    sampleParametricCurve,
    sampleCoordinateTrace,
    sampleCustomTransformTrace,
    customTransformPoint,
    sampleCircleArc,
    sampleThreePointArc,
    translationIterationDeltas,
    rotateIterationPoints,
    affineIterationSegments,
    branchingIterationSegments,
    linePolylineIntersection,
    choosePointCandidate,
    lineCircleIntersectionCandidate,
    pointDistance,
    pointDistanceRatio,
    pointAngleDegrees,
    polygonArea,
    evaluateExpr,
    evaluateExprWithDriver,
    iterateExpression,
  };
})();
