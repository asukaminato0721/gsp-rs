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
  };

  type CompiledExpression = {
    handle: number;
    parameterNames: string[];
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
  if (wasm.gsp_runtime_abi_version() !== 3) {
    throw new Error("Unsupported gsp-rs runtime core ABI");
  }

  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8");
  const expressionCache = new WeakMap<object, CompiledExpression>();

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
    for (let index = 0; index < compiled.parameterNames.length; index += 1) {
      const value = parameters.get(compiled.parameterNames[index]);
      wasm.gsp_expression_set_parameter(compiled.handle, index, value ?? Number.NaN);
    }
    const value = wasm.gsp_evaluate_expression(compiled.handle, x);
    return Number.isFinite(value) ? value : null;
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
    evaluateExpr,
  };
})();
