(function() {
  const modules = (
    window.GspViewerModules || (window.GspViewerModules = {})
  ) as Partial<ViewerModules> & { geometry: ViewerGeometryModule };
  const geometry = modules.geometry;
  const {
    rotateAround,
    reflectAcrossLine,
    clipParametricLineToBounds,
    angleBisectorDirection,
    measuredRotationRadians,
  } = geometry;
  type ViewBounds = { minX: number; maxX: number; minY: number; maxY: number; spanX?: number; spanY?: number };

  function isFiniteNumber(value: unknown): value is number {
    return typeof value === "number" && Number.isFinite(value);
  }

  function resolveRotateTransformAngleDegrees(transform: { angleDegrees?: number; parameterName?: string | null; angleExpr?: FunctionExprJson | null; angleStartIndex?: number | null; angleVertexIndex?: number | null; angleEndIndex?: number | null; angleParameterPointIndex?: number | null; angleParameterStartIndex?: number | null; angleParameterEndIndex?: number | null; angleParameterScale?: number | null }, parameters: Map<string, number>, resolvePoint: (index: number) => Point | null | undefined) {
    if (
      typeof transform.angleParameterPointIndex === "number"
      && typeof transform.angleParameterStartIndex === "number"
      && typeof transform.angleParameterEndIndex === "number"
    ) {
      const point = resolvePoint(transform.angleParameterPointIndex);
      const start = resolvePoint(transform.angleParameterStartIndex);
      const end = resolvePoint(transform.angleParameterEndIndex);
      if (!point || !start || !end) return null;
      const projection = window.GspRuntimeCore.projectToLineLike(point, start, end, "segment");
      return projection ? projection.t * (transform.angleParameterScale ?? 1) : null;
    }
    if (
      typeof transform.angleStartIndex === "number"
      && typeof transform.angleVertexIndex === "number"
      && typeof transform.angleEndIndex === "number"
    ) {
      const start = resolvePoint(transform.angleStartIndex);
      const vertex = resolvePoint(transform.angleVertexIndex);
      const end = resolvePoint(transform.angleEndIndex);
      if (!start || !vertex || !end) return null;
      const radians = measuredRotationRadians(start, vertex, end);
      return radians === null ? null : radians * 180 / Math.PI;
    }
    if (transform.angleExpr) {
      return evaluateExpr(transform.angleExpr, 0, parameters);
    }
    if (transform.parameterName) {
      return parameters.get(transform.parameterName) ?? null;
    }
    return transform.angleDegrees;
  }


  function usesVerboseParameterLabel(label: RuntimeLabelJson) {
    return typeof label.text === "string" && label.text.includes("在");
  }


  const { evaluateExpr } = modules.dynamicsExpression;
  const {
    deriveExpressionLabelParameters,
    deriveLabelParameters,
    parameterMapForScene,
  } = modules.dynamicsParameters.createDynamicsParameters({
    discreteIterationDepth,
    evaluateExpr,
    isDiscreteIterationParameterName,
    labelParameterValueFromBinding,
    pointAngleValue,
    pointDistanceRatioValue,
    pointDistanceValue,
    pointIterationDepth,
    polygonAreaValue,
  });
  const {
    parameterRootId,
    sourcePointRootId,
    describeDependencyGraph,
    runDependencyGraph,
  } = modules.dynamicsDependencyGraph.createDependencyGraphRuntime({
    applyBaseDynamicUpdates,
    parameterMapForScene,
    refreshDerivedPoints,
    refreshDynamicLabels,
    refreshIterationGeometry,
  });

  function wrapUnitInterval(value: number) {
    return ((value % 1) + 1) % 1;
  }


  function circleParameterFromPoint(scene: ViewerSceneData, pointIndex: number) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (constraint?.kind !== "circle" && constraint?.kind !== "circular-constraint") {
      return null;
    }
    const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
    const tau = Math.PI * 2;
    return ((pointAngle % tau) + tau) % tau / tau;
  }


  function pointDistanceRatioValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const origin = scene.points[binding.originIndex];
    const denominator = scene.points[binding.denominatorIndex];
    const numerator = scene.points[binding.numeratorIndex];
    if (!origin || !denominator || !numerator) return null;
    return window.GspRuntimeCore.pointDistanceRatio(
      origin,
      denominator,
      numerator,
      binding.clampToUnit === true,
    );
  }

  function pointDistanceValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const left = scene.points[binding.leftIndex];
    const right = scene.points[binding.rightIndex];
    if (!left || !right) return null;
    return window.GspRuntimeCore.pointDistance(left, right, binding.valueScale ?? 1);
  }


  function pointAngleValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const start = scene.points[binding.startIndex];
    const vertex = scene.points[binding.vertexIndex];
    const end = scene.points[binding.endIndex];
    if (!start || !vertex || !end) return null;
    return window.GspRuntimeCore.pointAngleDegrees(start, vertex, end);
  }


  function polygonAreaValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const points = binding.pointIndices.map((index: number) => scene.points[index]);
    if (points.length < 3 || points.some((point) => !point)) return null;
    return window.GspRuntimeCore.polygonArea(points, binding.valueScale ?? 1);
  }


  function pointCoordinatesInBasis(point: Point | null | undefined, origin: Point | null | undefined, xUnit: Point | null | undefined, yUnit: Point | null | undefined) {
    if (!point || !origin || !xUnit || !yUnit) return null;
    const xAxisX = xUnit.x - origin.x;
    const xAxisY = xUnit.y - origin.y;
    const yAxisX = yUnit.x - origin.x;
    const yAxisY = yUnit.y - origin.y;
    const pointX = point.x - origin.x;
    const pointY = point.y - origin.y;
    const det = xAxisX * yAxisY - xAxisY * yAxisX;
    if (!Number.isFinite(det) || Math.abs(det) <= 1e-9) return null;
    return {
      x: (pointX * yAxisY - pointY * yAxisX) / det,
      y: (xAxisX * pointY - xAxisY * pointX) / det,
    };
  }


  function lineProjectionParameterFromBinding(scene: ViewerSceneData, binding: { pointIndex?: number; startIndex?: number; endIndex?: number; lineKind?: RuntimeLineKind }) {
    const point = scene.points[binding.pointIndex];
    const start = scene.points[binding.startIndex];
    const end = scene.points[binding.endIndex];
    return lineProjectionParameterFromPoints(point, start, end, binding.lineKind);
  }


  function lineProjectionParameterFromPoints(point: Point | null | undefined, start: Point | null | undefined, end: Point | null | undefined, lineKind: RuntimeLineKind = "segment") {
    if (!point || !start || !end) return null;
    return window.GspRuntimeCore.projectToLineLike(point, start, end, lineKind)?.t ?? null;
  }


  function polygonBoundaryParameterFromPoint(scene: ViewerSceneData, pointIndex: number) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (
      !constraint
      || (
        constraint.kind !== "polygon-boundary"
        && constraint.kind !== "polygon-boundary-parameter"
        && constraint.kind !== "translated-polygon-boundary"
      )
      || constraint.vertexIndices.length < 2
    ) {
      return null;
    }

    if (constraint.kind === "polygon-boundary-parameter") {
      return wrapUnitInterval(constraint.parameter);
    }
    const count = constraint.vertexIndices.length;
    let perimeter = 0;
    let traveled = 0;
    for (let index = 0; index < count; index += 1) {
      const start = scene.points[constraint.vertexIndices[index]];
      const end = scene.points[constraint.vertexIndices[(index + 1) % count]];
      if (!start || !end) {
        return null;
      }
      const length = Math.hypot(end.x - start.x, end.y - start.y);
      perimeter += length;
      if (index < constraint.edgeIndex) {
        traveled += length;
      } else if (index === constraint.edgeIndex) {
        traveled += length * Math.max(0, Math.min(1, constraint.t));
      }
    }

    return perimeter > 1e-9 ? traveled / perimeter : null;
  }


  function polylineConstraintPoints(
    scene: ViewerSceneData,
    constraint: Extract<RuntimePointConstraintJson, { kind: "polyline" }>,
  ) {
    if (typeof constraint.functionKey === "number") {
      const hostLine = scene.lines.find((line) =>
        line?.binding?.kind === "arc-boundary" && line.binding.hostKey === constraint.functionKey
        || line?.debug?.groupOrdinal === constraint.functionKey
          && (
            line?.binding?.kind === "point-trace"
            || line?.binding?.kind === "coordinate-trace"
            || line?.binding?.kind === "custom-transform-trace"
          )
      );
      if (hostLine?.points?.length >= 2) {
        return hostLine.points;
      }
    }
    return constraint.points;
  }


  function polylineParameterFromPoint(scene: ViewerSceneData, pointIndex: number) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (constraint?.kind !== "polyline" || !Array.isArray(constraint.points) || constraint.points.length < 2) {
      return null;
    }
    const points = polylineConstraintPoints(scene, constraint);
    if (!Array.isArray(points) || points.length < 2) return null;
    return Number.isFinite(constraint.parameter)
      ? wrapUnitInterval(constraint.parameter)
      : null;
  }


  const POINT_CONSTRAINT_PARAMETER_READERS = {
    segment: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    line: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    "line-constraint": (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    ray: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    "ray-constraint": (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    polyline: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    "polygon-boundary": polygonBoundaryParameterFromPoint,
    "polygon-boundary-parameter": polygonBoundaryParameterFromPoint,
    "translated-polygon-boundary": polygonBoundaryParameterFromPoint,
    circle: circleParameterFromPoint,
    "circle-arc": (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    arc: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
  };


  const POINT_CONSTRAINT_PARAMETER_APPLIERS = {
    segment(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = Math.max(0, Math.min(1, value));
    },
    line(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = value;
    },
    "line-constraint"(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = value;
    },
    ray(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = Math.max(0, value);
    },
    "ray-constraint"(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = Math.max(0, value);
    },
    polyline(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number) {
      const constraint = point.constraint as Extract<
        RuntimePointConstraintJson,
        { kind: "polyline" }
      >;
      const pointCount = polylineConstraintPoints(scene, constraint)?.length ?? 0;
      if (pointCount < 2) return;
      constraint.parameter = wrapUnitInterval(value);
      const scaled = constraint.parameter * (pointCount - 1);
      constraint.segmentIndex = Math.min(
        pointCount - 2,
        Math.floor(scaled),
      );
      constraint.t = scaled - constraint.segmentIndex;
    },
    "polygon-boundary"(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number) {
      const wrapped = wrapUnitInterval(value);
      const count = point.constraint.vertexIndices.length;
      if (count < 2) return;
      const lengths = [];
      let perimeter = 0;
      for (let i = 0; i < count; i += 1) {
        const start = scene.points[point.constraint.vertexIndices[i]];
        const end = scene.points[point.constraint.vertexIndices[(i + 1) % count]];
        if (!start || !end) return;
        const length = Math.hypot(end.x - start.x, end.y - start.y);
        lengths.push(length);
        perimeter += length;
      }
      if (perimeter <= 1e-9) return;
      const target = wrapped * perimeter;
      let traveled = 0;
      for (let edgeIndex = 0; edgeIndex < lengths.length; edgeIndex += 1) {
        const length = lengths[edgeIndex];
        if (traveled + length >= target || edgeIndex === lengths.length - 1) {
          point.constraint.edgeIndex = edgeIndex;
          point.constraint.t = length <= 1e-9 ? 0 : Math.max(0, Math.min(1, (target - traveled) / length));
          return;
        }
        traveled += length;
      }
    },
    "polygon-boundary-parameter"(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.parameter = wrapUnitInterval(value);
    },
    "translated-polygon-boundary"(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number) {
      POINT_CONSTRAINT_PARAMETER_APPLIERS["polygon-boundary"](point, scene, value);
    },
    circle(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      const wrapped = wrapUnitInterval(value);
      const angle = Math.PI * 2 * wrapped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    },
    "circular-constraint"(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      const wrapped = wrapUnitInterval(value);
      const angle = Math.PI * 2 * wrapped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    },
    "circle-arc"(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = Math.max(0, Math.min(1, value));
    },
    arc(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = Math.max(0, Math.min(1, value));
    },
  };


  function parameterValueFromPoint(scene: ViewerSceneData, pointIndex: number) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (!constraint) return null;
    const readParameter = POINT_CONSTRAINT_PARAMETER_READERS[constraint.kind];
    return readParameter ? readParameter(scene, pointIndex) : null;
  }

  function labelParameterValueFromBinding(scene: ViewerSceneData, binding: LabelBindingJson) {
    if (binding.kind === "line-projection-parameter") {
      return lineProjectionParameterFromBinding(scene, binding);
    }
    if (binding.kind === "polyline-parameter") {
      return polylineParameterFromPoint(scene, binding.pointIndex);
    }
    if (binding.kind === "polygon-boundary-parameter") {
      return polygonBoundaryParameterFromPoint(scene, binding.pointIndex);
    }
    return "pointIndex" in binding && typeof binding.pointIndex === "number"
      ? parameterValueFromPoint(scene, binding.pointIndex)
      : null;
  }


  function applyNormalizedParameterToPoint(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number | null | undefined) {
    if (typeof value !== "number") return;
    if (!point.constraint) return;
    const applyParameter = POINT_CONSTRAINT_PARAMETER_APPLIERS[point.constraint.kind];
    if (applyParameter) {
      applyParameter(point, scene, value);
    }
  }


  function pointIterationDepth(family: { depth: number; parameterName?: string | null; depthParameterName?: string | null; depthExpr?: FunctionExprJson | null }, parameters: Map<string, number>) {
    const rawValue = family.depthParameterName
      ? parameters.get(family.depthParameterName)
      : family.depthExpr
        ? evaluateExpr(family.depthExpr, 0, parameters)
      : family.parameterName
        ? parameters.get(family.parameterName)
        : family.depth;
    const fallback = Number.isFinite(family.depth) ? family.depth : 0;
    const depth = typeof rawValue === "number" && Number.isFinite(rawValue) ? rawValue : fallback;
    return discreteIterationDepth(depth);
  }


  function discreteIterationDepth(value: number | null | undefined) {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return 0;
    }
    return Math.max(0, Math.floor(value + 1e-9));
  }


  function collectDiscreteIterationParameterNames(scene: ViewerSceneData | SceneData | null | undefined) {
    const names = new Set<string>();
    const add = ( name: unknown) => {
      if (typeof name === "string" && name.length > 0) {
        names.add(name);
      }
    };
    (scene?.pointIterations || []).forEach((family) => {
      add(family.depthParameterName);
    });
    (scene?.circleIterations || []).forEach((family) => add(family.depthParameterName));
    (scene?.lineIterations || []).forEach((family) => {
      if ("parameterName" in family) {
        add(family.parameterName);
      }
      if ("depthParameterName" in family) {
        add(family.depthParameterName);
      }
    });
    (scene?.lines || []).forEach((line) => {
      if (line.binding?.kind === "colorized-spectrum") {
        add(line.binding.depthParameterName);
      }
    });
    (scene?.polygonIterations || []).forEach((family) => {
      if ("parameterName" in family) {
        add(family.parameterName);
      }
    });
    (scene?.labelIterations || []).forEach((family) => add(family.depthParameterName));
    (scene?.iterationTables || []).forEach((table) => add(table.depthParameterName));
    return names;
  }


  function isDiscreteIterationParameterName(scene: ViewerSceneData | SceneData | null | undefined, name: string) {
    return collectDiscreteIterationParameterNames(scene).has(name);
  }


  function formatSequenceValue(value: number) {
    if (!Number.isFinite(value)) {
      return "-";
    }
    return Math.abs(value - Math.round(value)) < 0.005
      ? String(Math.round(value))
      : value.toFixed(2);
  }


  function formatExpressionLabelValue(label: RuntimeLabelJson, value: number | null, env: ViewerEnv) {
    if (value === null) {
      return "未定义";
    }
    return label.binding.exprLabel.includes("°") || label.binding.degreeValue
      ? `${value.toFixed(2)}°`
      : env.formatNumber(value);
  }


  function evaluateRecursiveExpression(expr: FunctionExprJson | FunctionAstJson, parameterName: string, currentValue: number, parameters: Map<string, number>) {
    const nextParameters = new Map<string, number>(parameters);
    nextParameters.set(parameterName, currentValue);
    return evaluateExpr(expr, 0, nextParameters);
  }


  function evaluateRichTextValueRef(scene: ViewerSceneData, ref: RichTextExpressionRefJson, parameters: Map<string, number>) {
    if (ref.kind === "parameter" && typeof ref.name === "string") {
      const value = parameters.get(ref.name);
      return typeof value === "number" && Number.isFinite(value) ? value : null;
    }
    if (ref.kind === "expression") {
      return evaluateExpr(ref.expr, 0, parameters);
    }
    if (ref.kind !== "iteration-state") {
      return null;
    }
    const stateNames = Array.isArray(ref.stateParameterNames) ? ref.stateParameterNames : [];
    const stateExprs = Array.isArray(ref.stateExprs) ? ref.stateExprs : [];
    if (stateNames.length === 0 || stateNames.length !== stateExprs.length) {
      return null;
    }
    const rawDepth = ref.depthExpr
      ? evaluateExpr(ref.depthExpr, 0, parameters)
      : ref.depth;
    const depth = discreteIterationDepth(Number.isFinite(rawDepth) ? rawDepth : ref.depth);
    const state = new Map<string, number>(parameters);
    for (let step = 0; step < depth; step += 1) {
      const derived = deriveExpressionLabelParameters(scene, state);

      const updates = [];
      stateExprs.forEach(( expr,  index: number) => {
        const name = stateNames[index];
        const value = evaluateExpr(expr, 0, derived);
        if (typeof name === "string" && Number.isFinite(value)) {
          updates.push([name,  (value)]);
        }
      });
      if (updates.length === 0) {
        break;
      }
      updates.forEach(([name, value]) => state.set(name, value));
    }
    const value = state.get(ref.targetParameterName);
    return typeof value === "number" && Number.isFinite(value) ? value : null;
  }


  const {
    buildExpressionRichMarkup,
    buildRatioValueRichMarkup,
    buildPlainTextRichMarkup,
    replaceRichMarkupPathValues,
    replaceTemplateTextRanges,
  } = modules.dynamicsRichText;
  function resolveLineConstraintPoints(resolvePointAt: (pointIndex: number) => Point | null, bounds: ViewBounds, constraint: LineConstraintJson) {
    if (!constraint) return null;
    if (constraint.kind === "segment") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? [start, end] : null;
    }
    if (constraint.kind === "line") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? clipParametricLineToBounds(start, end, bounds, false) : null;
    }
    if (constraint.kind === "ray") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? clipParametricLineToBounds(start, end, bounds, true) : null;
    }
    if (constraint.kind === "perpendicular-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x - dy / len, y: through.y + dx / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "parallel-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x + dx / len, y: through.y + dy / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "perpendicular-to" || constraint.kind === "parallel-to") {
      const through = resolvePointAt(constraint.throughIndex);
      const base = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      if (!through || !base || base.length < 2) return null;
      const dx = base[1].x - base[0].x;
      const dy = base[1].y - base[0].y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        constraint.kind === "perpendicular-to"
          ? { x: through.x - dy / len, y: through.y + dx / len }
          : { x: through.x + dx / len, y: through.y + dy / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolvePointAt(constraint.startIndex);
      const vertex = resolvePointAt(constraint.vertexIndex);
      const end = resolvePointAt(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      if (!direction) return null;
      return clipParametricLineToBounds(
        vertex,
        { x: vertex.x + direction.x, y: vertex.y + direction.y },
        bounds,
        true,
      );
    }
    if (constraint.kind === "translated") {

      const source = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      const vectorStart = resolvePointAt(constraint.vectorStartIndex);
      const vectorEnd = resolvePointAt(constraint.vectorEndIndex);
      if (!source || !vectorStart || !vectorEnd) return null;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      return source.map(( point) => ({ x: point.x + dx, y: point.y + dy }));
    }
    if (constraint.kind === "translated-delta") {
      const source = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      if (!source) return null;
      return source.map((point) => ({
        x: point.x + constraint.dx,
        y: point.y + constraint.dy,
      }));
    }
    if (constraint.kind === "reflected") {
      const source = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      const axis = resolveLineConstraintParameterPoints(resolvePointAt, constraint.axis);
      if (!source || !axis) return null;
      const reflected = source.map((point) => reflectAcrossLine(point, axis[0], axis[1]));
      return reflected.every((point): point is Point => point !== null) ? reflected : null;
    }
    if (constraint.kind === "rotated") {
      const source = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      const center = resolvePointAt(constraint.centerIndex);
      if (!source || !center) return null;
      const angleDegrees = resolveRotateTransformAngleDegrees(
        constraint,
        new Map(),
        resolvePointAt,
      );
      if (!isFiniteNumber(angleDegrees)) return null;
      const radians = angleDegrees * Math.PI / 180;
      return source.map((point) => rotateAround(point, center, radians));
    }
    return null;
  }


  function resolveLineConstraintParameterPoints(resolvePointAt: (pointIndex: number) => Point | null, constraint: LineConstraintJson) {
    if (!constraint) return null;
    if (
      constraint.kind === "segment"
      || constraint.kind === "line"
      || constraint.kind === "ray"
    ) {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? [start, end] : null;
    }
    if (constraint.kind === "perpendicular-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return [
        through,
        { x: through.x - dy, y: through.y + dx },
      ];
    }
    if (constraint.kind === "parallel-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return [
        through,
        { x: through.x + dx, y: through.y + dy },
      ];
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolvePointAt(constraint.startIndex);
      const vertex = resolvePointAt(constraint.vertexIndex);
      const end = resolvePointAt(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      return direction
        ? [vertex, { x: vertex.x + direction.x, y: vertex.y + direction.y }]
        : null;
    }
    if (constraint.kind === "translated") {
      const source = resolveLineConstraintParameterPoints(resolvePointAt, constraint.line);
      const vectorStart = resolvePointAt(constraint.vectorStartIndex);
      const vectorEnd = resolvePointAt(constraint.vectorEndIndex);
      if (!source || !vectorStart || !vectorEnd) return null;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      return source.map(( point) => ({ x: point.x + dx, y: point.y + dy }));
    }
    if (constraint.kind === "translated-delta") {
      const source = resolveLineConstraintParameterPoints(resolvePointAt, constraint.line);
      if (!source) return null;
      return source.map((point) => ({
        x: point.x + constraint.dx,
        y: point.y + constraint.dy,
      }));
    }
    if (constraint.kind === "reflected") {
      const source = resolveLineConstraintParameterPoints(resolvePointAt, constraint.line);
      const axis = resolveLineConstraintParameterPoints(resolvePointAt, constraint.axis);
      if (!source || !axis) return null;
      const reflected = source.map((point) => reflectAcrossLine(point, axis[0], axis[1]));
      return reflected.every((point): point is Point => point !== null) ? reflected : null;
    }
    if (constraint.kind === "rotated") {
      const source = resolveLineConstraintParameterPoints(resolvePointAt, constraint.line);
      const center = resolvePointAt(constraint.centerIndex);
      if (!source || !center) return null;
      const angleDegrees = resolveRotateTransformAngleDegrees(
        constraint,
        new Map(),
        resolvePointAt,
      );
      if (!isFiniteNumber(angleDegrees)) return null;
      const radians = angleDegrees * Math.PI / 180;
      return source.map((point) => rotateAround(point, center, radians));
    }
    return null;
  }


  const DYNAMIC_LABEL_REFRESHERS: Record<string, DynamicLabelRefresher> = {
    "point-anchor"(_env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const point = scene.points[label.binding.pointIndex];
      if (!point) return;
      const anchor = {
        x: point.x + (label.binding.anchorDx || 0),
        y: point.y + (label.binding.anchorDy || 0),
      };
      if (Number.isFinite(label.binding.anchorYPointIndex)) {
        const yPoint = scene.points[label.binding.anchorYPointIndex];
        if (yPoint) {
          anchor.y = yPoint.y + (label.binding.anchorYDy || 0);
        }
      }
      label.anchor = anchor;
    },
    "parameter-value"(env: ViewerEnv, _scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const value = parameters.get(label.binding.name);
      if (value !== null && value !== undefined) {
        label.text = `${label.binding.name} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "point-expression-value"(_env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const currentValue = parameters.get(label.binding.parameterName);
      if (!isFiniteNumber(currentValue)) return;
      DYNAMIC_LABEL_REFRESHERS["point-anchor"](_env, scene, label, parameters);
      const value = evaluateRecursiveExpression(
        label.binding.expr,
        label.binding.parameterName,
        currentValue,
        parameters,
      );
      if (value !== null) {
        label.text = formatSequenceValue(value);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "sequence-expression-value"(_env: ViewerEnv, _scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const stateValue = parameters.get(label.binding.parameterName);
      if (isFiniteNumber(stateValue)) {
        label.text = formatSequenceValue(stateValue);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
        return;
      }
      const currentValue = parameters.get(label.binding.parameterName);
      if (!isFiniteNumber(currentValue)) return;
      const depth = pointIterationDepth({
        depth: label.binding.depth,
        parameterName: label.binding.depthParameterName,
      }, parameters);

      const values = window.GspRuntimeCore.iterateExpression(
        label.binding.expr,
        label.binding.parameterName,
        currentValue,
        parameters,
        depth + 1,
      );
      if (values.length !== depth + 1) return;
      label.text = formatSequenceValue(values[values.length - 1]);
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "rich-text-expression-values"(_env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {

      const valuesBySlot = new Map<number, string>();

      const replacements: Array<{ line: number; start: number; end: number; valueText: string }> = [];
      (label.binding.refs || []).forEach(( ref) => {
        const value = evaluateRichTextValueRef(scene, ref, parameters);
        const valueText = value !== null ? formatSequenceValue(value) : "未定义";
        valuesBySlot.set(ref.slot, valueText);
        replacements.push({
          line: ref.line,
          start: ref.start,
          end: ref.end,
          valueText,
        });
      });
      label.text = replaceTemplateTextRanges(label.binding.templateText || label.text || "", replacements);
      label.richMarkup = replaceRichMarkupPathValues(label.binding.templateRichMarkup, valuesBySlot)
        || buildPlainTextRichMarkup(label.text);
    },
    "point-coordinate-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const point = scene.points[label.binding.pointIndex];
      if (!point) return;
      const coordinates = pointCoordinatesInBasis(
        point,
        scene.points[label.binding.originIndex],
        scene.points[label.binding.xUnitIndex],
        scene.points[label.binding.yUnitIndex],
      ) ?? point;
      label.text = `${label.binding.pointName}: (${env.formatNumber(coordinates.x)}, ${env.formatNumber(coordinates.y)})`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "point-distance-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = pointDistanceValue(scene, label.binding);
      if (value === null) return;
      label.text = `${label.binding.name} = ${env.formatNumber(value)}${label.binding.valueSuffix || ""}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "point-angle-value"(_env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = pointAngleValue(scene, label.binding);
      if (value === null) return;
      label.text = `${label.binding.name} = ${value.toFixed(2)}${label.binding.valueSuffix || ""}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "polygon-area-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = polygonAreaValue(scene, label.binding);
      if (value === null) return;
      label.text = `${label.binding.name} = ${env.formatNumber(value)}${label.binding.valueSuffix || ""}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "point-distance-ratio-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = pointDistanceRatioValue(scene, label.binding);
      if (value === null) return;
      const valueText = env.formatNumber(value);
      label.text = `${label.binding.name} = ${valueText}`;
      label.richMarkup = buildRatioValueRichMarkup(label.binding.name, valueText)
        || buildPlainTextRichMarkup(label.text);
    },
    "point-axis-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const point = scene.points[label.binding.pointIndex];
      if (!point) return;
      const coordinates = pointCoordinatesInBasis(
        point,
        scene.points[label.binding.originIndex],
        scene.points[label.binding.xUnitIndex],
        scene.points[label.binding.yUnitIndex],
      );
      const value = label.binding.axis === "vertical"
        ? (coordinates?.y ?? point.y)
        : (coordinates?.x ?? point.x);
      label.text = `${label.binding.name} = ${env.formatAxisNumber(value)}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "scalar-alias"(_env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const source = scene.labels.find(
        (candidate) => candidate.debug?.groupOrdinal === label.binding.sourceGroupOrdinal,
      );
      if (!source) return;
      const separator = source.text.indexOf("=");
      label.text = separator >= 0
        ? `${label.binding.name} ${source.text.slice(separator)}`
        : source.text;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "expression-value"(env: ViewerEnv, _scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const value = evaluateExpr(label.binding.expr, 0, parameters);
      const valueText = formatExpressionLabelValue(label, value, env);
      label.richMarkup = buildExpressionRichMarkup(
        label.binding.exprLabel,
        valueText,
      );
      if (value !== null) {
        label.text = `${label.binding.exprLabel} = ${valueText}`;
      } else {
        label.text = `${label.binding.exprLabel} = 未定义`;
      }
    },
    "point-bound-expression-value"(env: ViewerEnv, _scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const value = evaluateExpr(label.binding.expr, 0, parameters);
      const valueText = formatExpressionLabelValue(label, value, env);
      label.richMarkup = buildExpressionRichMarkup(
        label.binding.exprLabel,
        valueText,
      );
      if (value !== null) {
        label.text = `${label.binding.exprLabel} = ${valueText}`;
      } else {
        label.text = `${label.binding.exprLabel} = 未定义`;
      }
    },
    "polygon-boundary-parameter"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = polygonBoundaryParameterFromPoint(scene, label.binding.pointIndex);
      if (value !== null) {
        label.text = label.binding.polygonName
          ? `${label.binding.pointName}在${label.binding.polygonName}上的值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "line-projection-parameter"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = lineProjectionParameterFromBinding(scene, label.binding);
      if (value !== null) {
        label.text = usesVerboseParameterLabel(label)
          ? `${label.binding.pointName}在${label.binding.objectName}上的t值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "polyline-parameter"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = polylineParameterFromPoint(scene, label.binding.pointIndex);
      if (value !== null) {
        label.text = usesVerboseParameterLabel(label)
          ? `${label.binding.pointName}在${label.binding.objectName}上的值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "circle-parameter"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const point = scene.points[label.binding.pointIndex];
      const constraint = point?.constraint;
      if (constraint?.kind !== "circle") return;
      const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
      const tau = Math.PI * 2;
      const value = ((pointAngle % tau) + tau) % tau / tau;
      label.text = usesVerboseParameterLabel(label)
        ? `${label.binding.pointName}在⊙${label.binding.circleName}上的值 = ${env.formatNumber(value)}`
        : `${label.binding.pointName} = ${env.formatNumber(value)}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "angle-marker-value"(_env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const start = scene.points[label.binding.startIndex];
      const vertex = scene.points[label.binding.vertexIndex];
      const end = scene.points[label.binding.endIndex];
      if (!start || !vertex || !end) return;
      const value = window.GspRuntimeCore.pointAngleDegrees(start, vertex, end);
      if (Number.isFinite(value)) {
        label.text = value.toFixed(label.binding.decimals);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "custom-transform-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const value = parameterValueFromPoint(scene, label.binding.pointIndex);
      if (!isFiniteNumber(value)) return;
      const evaluated = window.GspRuntimeCore.evaluateExprWithDriver(
        label.binding.expr,
        value,
        parameters,
        value,
      );
      if (evaluated !== null) {
        label.text = `${label.binding.exprLabel} = ${env.formatNumber(evaluated * label.binding.valueScale)}${label.binding.valueSuffix}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
  };

  const objectGraphLabelValues = new WeakMap<object, Map<number, number>>();

  function refreshExpressionLabelFromGraphValue(
    env: ViewerEnv,
    label: RuntimeLabelJson,
    value: number,
  ) {
    if (
      label.binding?.kind !== "expression-value"
      && label.binding?.kind !== "point-bound-expression-value"
    ) {
      return false;
    }
    const valueText = formatExpressionLabelValue(label, value, env);
    label.text = `${label.binding.exprLabel} = ${valueText}`;
    label.richMarkup = buildExpressionRichMarkup(label.binding.exprLabel, valueText);
    return true;
  }
  function refreshDerivedPoints(env: ViewerEnv, scene: ViewerSceneData) {
    refreshGeometryFromObjectGraph(env, scene);
  }


  function refreshGeometryFromObjectGraph(env: ViewerEnv, scene: ViewerSceneData) {
    const graph = env.sourceScene.objectGraph;
    if (!graph?.geometryComplete) {
      throw new Error("scene-data is missing a complete Rust object graph");
    }
    const sourceValue = (source: ObjectGraphSourceJson) => {
      const binding = source.binding;
      if (binding.kind === "point-control") {
        const point = scene.points[binding.pointIndex];
        const constraint = point?.constraint;
        if (!constraint) return source.value;
        if (binding.control === "parameter" && typeof constraint.t === "number") {
          if (constraint.kind === "polyline") {
            const parameter = polylineParameterFromPoint(scene, binding.pointIndex);
            if (parameter !== null) return { kind: "scalar", value: parameter };
          }
          return { kind: "scalar", value: constraint.t };
        }
        if (binding.control === "unit-x" && typeof constraint.unitX === "number") {
          return { kind: "scalar", value: constraint.unitX };
        }
        if (binding.control === "unit-y" && typeof constraint.unitY === "number") {
          return { kind: "scalar", value: constraint.unitY };
        }
        if (binding.control === "boundary" && typeof constraint.parameter === "number") {
          return { kind: "scalar", value: constraint.parameter };
        }
        return source.value;
      }
      if (binding.kind === "parameter") {
        const parameter = env.currentDynamics().parameters.find((candidate) => candidate.name === binding.name);
        return parameter && Number.isFinite(parameter.value)
          ? { kind: "scalar", value: parameter.value }
          : source.value;
      }
      if (binding.kind === "point") {
        const point = scene.points[binding.pointIndex];
        return point ? { kind: "point", x: point.x, y: point.y } : source.value;
      }
      if (binding.kind === "line") {
        const line = scene.lines[binding.lineIndex];
        if (line && binding.lineKind && line.points.length >= 2) {
          return {
            kind: "line",
            line_kind: binding.lineKind,
            start: line.points[0],
            end: line.points[line.points.length - 1],
          };
        }
        return line ? { kind: "points", points: line.points } : source.value;
      }
      if (binding.kind === "circle") {
        const circle = scene.circles[binding.circleIndex];
        return circle ? {
          kind: "circle",
          center: circle.center,
          radius_point: circle.radiusPoint,
        } : source.value;
      }
      if (binding.kind === "polygon") {
        const polygon = scene.polygons[binding.polygonIndex];
        return polygon ? { kind: "points", points: polygon.points } : source.value;
      }
      return source.value;
    };
    const results = window.GspRuntimeCore.evaluateObjectGraph({
      nodes: graph.nodes,
      sources: graph.sources.map((source) => ({
        id: source.id,
        value: sourceValue(source),
      })),
    });
    const bounds = env.getViewBounds ? env.getViewBounds() : (scene.bounds || env.sourceScene.bounds);
    const pointIterationResults = new Map<number, Point[]>();
    const lineIterationResults = new Map<number, Point[]>();
    const circleIterationResults = new Map<number, Array<{ center: Point; radiusPoint: Point }>>();
    const polygonIterationResults = new Map<number, Point[][]>();
    const labelValues = new Map<number, number>();
    objectGraphLabelValues.set(scene, labelValues);
    results.forEach((result: any) => {
      const labelMatch = /^scalar:label:(\d+)$/.exec(result.id);
      if (
        labelMatch
        && result.value?.kind === "scalar"
        && Number.isFinite(result.value.value)
      ) {
        labelValues.set(Number(labelMatch[1]), result.value.value);
        return;
      }
      const [kind, rawIndex] = result.id.split(":");
      const index = Number(rawIndex);
      if (!Number.isInteger(index) || index < 0) return;
      if (kind === "point" && result.value?.kind === "point" && scene.points[index]) {
        scene.points[index].x = result.value.x;
        scene.points[index].y = result.value.y;
        return;
      }
      if (kind === "line" && scene.lines[index]) {
        if (result.value?.kind === "undefined") {
          scene.lines[index].visible = false;
          return;
        }
        scene.lines[index].visible = env.sourceScene.lines[index]?.visible !== false;
        if (result.value?.kind === "line") {
          const endpoints = result.value.line_kind === "line"
            ? window.GspRuntimeCore.clipLineToBounds(result.value.start, result.value.end, bounds)
            : result.value.line_kind === "ray"
              ? window.GspRuntimeCore.clipRayToBounds(result.value.start, result.value.end, bounds)
              : [result.value.start, result.value.end];
          if (endpoints) scene.lines[index].points = endpoints;
        } else if (
          result.value?.kind === "points"
          || result.value?.kind === "curve"
          || result.value?.kind === "sampled-curve"
        ) {
          scene.lines[index].points = result.value.points;
          if (scene.lines[index].binding?.kind === "segment-trace") {
            scene.lines[index].segments = Array.from(
              { length: Math.floor(result.value.points.length / 2) },
              (_, segmentIndex) => result.value.points.slice(segmentIndex * 2, segmentIndex * 2 + 2),
            );
          }
        }
        return;
      }
      if (kind === "circle" && result.value?.kind === "circle" && scene.circles[index]) {
        scene.circles[index].center = result.value.center;
        scene.circles[index].radiusPoint = result.value.radius_point;
        return;
      }
      if (kind === "circle-fill-color" && result.value?.kind === "color" && scene.circles[index]) {
        scene.circles[index].fillColor = result.value.color;
        return;
      }
      if (kind === "polygon" && result.value?.kind === "points" && scene.polygons[index]) {
        scene.polygons[index].points = result.value.points;
        return;
      }
      if (kind === "polygon-color" && result.value?.kind === "color" && scene.polygons[index]) {
        scene.polygons[index].color = result.value.color;
        return;
      }
      if (kind === "arc" && result.value?.kind === "arc" && scene.arcs[index]) {
        scene.arcs[index].points = [result.value.start, result.value.mid, result.value.end];
        scene.arcs[index].center = result.value.center ?? null;
        scene.arcs[index].counterclockwise = result.value.counterclockwise === true;
        return;
      }
      if (kind === "polygon-iteration" && result.value?.kind === "polygons") {
        polygonIterationResults.set(index, result.value.polygons);
        return;
      }
      if (kind === "point-iteration" && result.value?.kind === "points") {
        pointIterationResults.set(index, result.value.points);
        return;
      }
      if (kind === "line-iteration" && result.value?.kind === "points") {
        lineIterationResults.set(index, result.value.points);
        return;
      }
      if (kind === "circle-iteration" && result.value?.kind === "circles") {
        circleIterationResults.set(index, result.value.circles);
      }
    });

    const pointIterations = env.sourceScene.pointIterations || [];
    if (pointIterations.length > 0) {
      const standaloneCount = env.sourceScene.points
        .filter((point) => point?.binding?.kind === "parameter" && !point.constraint)
        .length;
      const baseCount = Math.max(0, env.sourceScene.points.length - standaloneCount);
      const standalonePoints = scene.points.slice(scene.points.length - standaloneCount);
      scene.points = scene.points.slice(0, baseCount);
      pointIterations.forEach((family, familyIndex) => {
        const points = pointIterationResults.get(familyIndex) || [];
        const sourceIndex = family.pointIndex;
        points.forEach((point) => {
          const template = scene.points[sourceIndex];
          scene.points.push({
            ...(template || {}),
            x: point.x,
            y: point.y,
            color: template?.color || [255, 60, 40, 255],
            visible: template?.visible !== false,
            draggable: false,
            constraint: null,
            binding: null,
            debug: null,
          });
        });
      });
      standalonePoints.forEach((point) => scene.points.push(point));
    }

    const lineIterations = env.sourceScene.lineIterations || [];
    if (lineIterations.length > 0) {
      const exportedDepth = lineIterations.reduce((sum, family) => {
        const depth = Math.max(0, family.depth || 0);
        if (family.kind === "parameterized-point-trace" || family.kind === "rotate") return sum;
        if (family.kind === "branching") {
          const branchCount = Array.isArray(family.targetSegments) ? family.targetSegments.length : 0;
          let total = 0;
          let width = branchCount;
          for (let step = 0; step < depth; step += 1) {
            total += width;
            width *= branchCount;
          }
          return sum + total;
        }
        if (family.kind === "affine") return sum + depth;
        if (family.bidirectional) {
          return sum + (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)
            ? 2 * depth * (depth + 1)
            : 2 * depth);
        }
        return sum + (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)
          ? ((depth + 1) * (depth + 2)) / 2 - 1
          : depth);
      }, 0);
      const baseCount = Math.max(0, env.sourceScene.lines.length - exportedDepth);
      scene.lines = scene.lines.slice(0, baseCount);
      lineIterations.forEach((family, familyIndex) => {
        const points = lineIterationResults.get(familyIndex) || [];
        for (let index = 0; index + 1 < points.length; index += 2) {
          scene.lines.push({
            points: [points[index], points[index + 1]],
            segments: null,
            color: family.color,
            dashed: !!family.dashed,
            strokeWidth: family.strokeWidth,
            visible: family.visible !== false,
            binding: null,
            debug: null,
          });
        }
      });
    }

    const circleIterations = env.sourceScene.circleIterations || [];
    if (circleIterations.length > 0) {
      const exportedDepth = circleIterations.reduce(
        (sum, family) => sum + Math.max(0, family.depth || 0),
        0,
      );
      const baseCount = Math.max(0, env.sourceScene.circles.length - exportedDepth);
      scene.circles = scene.circles.slice(0, baseCount);
      circleIterations.forEach((family, familyIndex) => {
        const source = scene.circles[family.sourceCircleIndex];
        if (!source) return;
        (circleIterationResults.get(familyIndex) || []).forEach((circle) => {
          scene.circles.push({
            ...source,
            center: circle.center,
            radiusPoint: circle.radiusPoint,
            visible: family.visible !== false,
            binding: null,
            fillColorBinding: null,
            debug: null,
          });
        });
      });
    }

    const polygonIterations = env.sourceScene.polygonIterations || [];
    if (polygonIterations.length > 0) {
      const generatedCount = polygonIterations.reduce((sum, family) => {
        const depth = Math.max(0, Math.round(family.depth || 0));
        if (family.kind === "coordinate-grid" || family.kind === "similarity") {
          return sum + depth;
        }
        if (family.bidirectional) {
          return sum + (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)
            ? 1 + 2 * depth * (depth + 1)
            : 1 + 2 * depth);
        }
        return sum + (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)
          ? ((depth + 1) * (depth + 2)) / 2
          : depth + 1);
      }, 0);
      const baseCount = Math.max(0, env.sourceScene.polygons.length - generatedCount);
      scene.polygons = scene.polygons.slice(0, baseCount);
      polygonIterations.forEach((family, familyIndex) => {
        const polygons = polygonIterationResults.get(familyIndex) || [];
        polygons.forEach((points) => {
          scene.polygons.push({
            points,
            color: family.color,
            colorBinding: null,
            visible: family.visible !== false,
            binding: null,
            debug: null,
          });
        });
      });
    }
  }


  function refreshDynamicLabels(env: ViewerEnv, scene: ViewerSceneData) {
    const parameters = parameterMapForScene(env, scene);
    const labelValues = objectGraphLabelValues.get(scene);
    scene.labels.forEach(( label, labelIndex) => {
      if (!label.binding) return;
      const graphValue = labelValues?.get(labelIndex);
      if (
        graphValue !== undefined
        && refreshExpressionLabelFromGraphValue(env, label, graphValue)
      ) {
        return;
      }
      const refreshLabel = DYNAMIC_LABEL_REFRESHERS[label.binding.kind];
      if (refreshLabel) {
        refreshLabel(env, scene, label, parameters);
      }
    });
  }


  function applyBaseDynamicUpdates(env: ViewerEnv, draft: ViewerSceneData, _parameters: Map<string, number>) {
    env.currentDynamics().parameters.forEach((parameter) => {
      if (typeof parameter.labelIndex === "number" && draft.labels[parameter.labelIndex]) {
        draft.labels[parameter.labelIndex].text =
          `${parameter.name} = ${env.formatNumber(parameter.value)}${parameterValueSuffix(parameter)}`;
      }
    });
    env.currentDynamics().functions.forEach((functionDef) => {
      if (typeof functionDef.labelIndex !== "number" || !draft.labels[functionDef.labelIndex]) return;
      const variableLabel = functionDef.domain.plotMode === "polar" ? "θ" : "x";
      const head = functionDef.domain.plotMode === "polar"
        ? (functionDef.derivative ? `r'(${variableLabel})` : "r")
        : (functionDef.derivative
          ? `${functionDef.name}'(${variableLabel})`
          : `${functionDef.name}(${variableLabel})`);
      const exprLabel = functionDef.domain.plotMode === "polar"
        ? functionDef.polarExprLabel
        : functionDef.exprLabel;
      draft.labels[functionDef.labelIndex].text = `${head} = ${exprLabel}`;
    });
  }


  function syncDynamicScene(env: ViewerEnv, dirtyParameterNames: string[]) {
    const names = Array.isArray(dirtyParameterNames) && dirtyParameterNames.length > 0
      ? dirtyParameterNames
      : env.currentDynamics().parameters.map((parameter) => parameter.name);
    env.markDependencyRootsDirty?.(
      names.map((name: string) => parameterRootId(name)),
    );
    env.updateScene(() => {}, "graph");
  }


  const {
    rebuildIteratedLabels,
    rebuildIterationTables,
  } = modules.dynamicsIterations.createDynamicsIterations({
    buildPlainTextRichMarkup,
    deriveExpressionLabelParameters,
    deriveLabelParameters,
    discreteIterationDepth,
    evaluateExpr,
    formatSequenceValue,
    isFiniteNumber,
    pointAngleValue,
    pointIterationDepth,
  });


  function refreshIterationGeometry(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    refreshDerivedPoints(env, scene);
    rebuildIteratedLabels(env, scene, parameters);
    rebuildIterationTables(env, scene, parameters);
  }


  function parameterValueSuffix(parameter: ParameterJson) {
    switch (parameter.unit) {
      case "degree":
        return "\u00B0";
      case "cm":
        return " cm";
      default:
        return "";
    }
  }


  function buildParameterControls(env: ViewerEnv) {
    const parameterControls = env.parameterControls;
    if (!parameterControls) {
      return;
    }
    parameterControls.replaceChildren();
    const controls = env.currentDynamics().parameters
      .map((parameter, index: number) => ({ parameter, index }))
      .filter(({ parameter }) => parameter.visible !== false)
      .map(({ parameter, index }) => {
        const isDiscrete = isDiscreteIterationParameterName(env.sourceScene, parameter.name);

        const inputAttrs: {
          type: string;
          step: string;
          min?: string;
          value: string;
          oninput: (event: Event) => void;
        } = {
          type: "number",
          step: isDiscrete ? "1" : "0.1",
          value: env.formatNumber(parameter.value),
          oninput: (event) => {
            const target = event.target as HTMLInputElement;
            let value = Number.parseFloat(target.value);
            if (Number.isFinite(value)) {
              if (isDiscrete) {
                value = discreteIterationDepth(value);
              }
              env.updateDynamics((draft: ViewerSceneData) => {
                draft.parameters[index].value = value;
              });
              syncDynamicScene(env, [parameter.name]);
            }
          },
        };
        if (isDiscrete) {
          inputAttrs.min = "0";
        }
        return env.labelTag(
          `${parameter.name} =`,
          env.inputTag( (inputAttrs)),
          parameterValueSuffix(parameter),
        );
      });
    if (controls.length > 0) {
      env.van.add(parameterControls, ...controls);
    }
  }

  modules.dynamics = {
    buildParameterControls,
    evaluateExpr,
    parameterMapForScene,
    parameterValueFromPoint,
    applyNormalizedParameterToPoint,
    refreshDerivedPoints,
    refreshDynamicLabels,
    refreshIterationGeometry,
    resolveLineConstraintPoints,
    resolveLineConstraintParameterPoints,
    parameterRootId,
    sourcePointRootId,
    runDependencyGraph,
    describeDependencyGraph,
    syncDynamicScene,
  };
})();
