(function() {
  const modules = (
    window.GspViewerModules || (window.GspViewerModules = {})
  ) as Partial<ViewerModules> & { geometry: ViewerGeometryModule };
  const geometry = modules.geometry;
  const {
    lerpPoint,
    rotateAround,
    scaleAround,
    reflectAcrossLine,
    clipParametricLineToBounds,
    clipLineToBounds,
    clipRayToBounds,
    angleBisectorDirection,
    measuredRotationRadians,
    scaleByThreePointRatio,
  } = geometry;
  type ViewBounds = { minX: number; maxX: number; minY: number; maxY: number; spanX?: number; spanY?: number };

  function isFiniteNumber(value: unknown) {
    return typeof value === "number" && Number.isFinite(value);
  }

  function hasPointIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { pointIndex: number }> {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }


  function hasLineIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { lineIndex: number }> {
    return !!handle && typeof handle === "object" && "lineIndex" in handle && typeof handle.lineIndex === "number";
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
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const lenSq = dx * dx + dy * dy;
      if (lenSq <= 1e-9) return null;
      const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSq));
      return t * (transform.angleParameterScale ?? 1);
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


  function resolveScaleTransformFactor(transform: { centerIndex?: number; factor?: number; parameterName?: string | null; factorExpr?: FunctionExprJson | null; factorParameterPointIndex?: number | null; factorParameterStartIndex?: number | null; factorParameterEndIndex?: number | null }, parameters: Map<string, number>, resolvePointAt: ((index: number) => Point | null | undefined) | null = null) {
    if (
      typeof transform.factorParameterPointIndex === "number"
      && typeof transform.factorParameterStartIndex === "number"
      && typeof transform.factorParameterEndIndex === "number"
      && typeof resolvePointAt === "function"
    ) {
      const point = resolvePointAt(transform.factorParameterPointIndex);
      const start = resolvePointAt(transform.factorParameterStartIndex);
      const end = resolvePointAt(transform.factorParameterEndIndex);
      const value = segmentProjectionParameterFromPoints(point, start, end);
      if (Number.isFinite(value)) return value;
    }
    if (transform.factorExpr) {
      return evaluateExpr(transform.factorExpr, 0, parameters);
    }
    if (transform.parameterName) {
      return parameters.get(transform.parameterName) ?? null;
    }
    return transform.factor;
  }


  function usesVerboseParameterLabel(label: RuntimeLabelJson) {
    return typeof label.text === "string" && label.text.includes("在");
  }


  function resolveRightAngleMarkerPoints(vertex: Point, first: Point, second: Point, shortestLen: number) {
    const side = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
    if (side <= 1e-9) return null;
    return [
      { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
      { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
      { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
    ];
  }


  function resolveArcAngleMarkerPoints(vertex: Point, first: Point, shortestLen: number, cross: number, dot: number, markerClass: number) {
    const classScale = 1 + 0.18 * Math.max(0, (markerClass || 1) - 1);
    const radius = Math.min(Math.max(shortestLen * 0.12, 10), 28) * classScale;
    const clampedRadius = Math.min(radius, shortestLen * 0.42);
    if (clampedRadius <= 1e-9) return null;
    const delta = Math.atan2(cross, dot);
    if (Math.abs(delta) <= 1e-6) return null;
    const startAngle = Math.atan2(first.y, first.x);
    const samples = 9;
    return Array.from({ length: samples }, (_, index: number) => {
      const t = index / (samples - 1);
      const angle = startAngle + delta * t;
      return {
        x: vertex.x + clampedRadius * Math.cos(angle),
        y: vertex.y + clampedRadius * Math.sin(angle),
      };
    });
  }


  function resolveAngleMarkerPoints(start: Point, vertex: Point, end: Point, markerClass: number) {
    const firstDx = start.x - vertex.x;
    const firstDy = start.y - vertex.y;
    const secondDx = end.x - vertex.x;
    const secondDy = end.y - vertex.y;
    const firstLen = Math.hypot(firstDx, firstDy);
    const secondLen = Math.hypot(secondDx, secondDy);
    const shortestLen = Math.min(firstLen, secondLen);
    if (firstLen <= 1e-9 || secondLen <= 1e-9 || shortestLen <= 1e-9) return null;
    const first = { x: firstDx / firstLen, y: firstDy / firstLen };
    const second = { x: secondDx / secondLen, y: secondDy / secondLen };
    const dot = Math.max(-1, Math.min(1, first.x * second.x + first.y * second.y));
    const cross = first.x * second.y - first.y * second.x;
    if (Math.abs(dot) <= 0.12) {
      return resolveRightAngleMarkerPoints(vertex, first, second, shortestLen);
    }
    return resolveArcAngleMarkerPoints(vertex, first, shortestLen, cross, dot, markerClass);
  }


  const { evaluateExpr, formatExpr, exprContainsPiAngle } = modules.dynamicsExpression;
  function deriveExpressionLabelParameters(scene: ViewerSceneData | null | undefined, seedParameters: Map<string, number>) {
    const parameters = new Map(seedParameters);
    if (!scene?.labels?.length) {
      return parameters;
    }
    const maxPasses = Math.max(16, scene.labels.length + 1);
    for (let pass = 0; pass < maxPasses; pass += 1) {
      let changed = false;
      scene.labels.forEach(( label) => {
        const binding = label.binding;
        if (!binding) {
          return;
        }
        if (
          (binding.kind === "segment-projection-parameter"
            || binding.kind === "polyline-parameter"
            || binding.kind === "polygon-boundary-parameter"
            || binding.kind === "circle-parameter")
          && typeof binding.pointName === "string"
        ) {
          const value = labelParameterValueFromBinding(scene, binding);
          const nextValue = isDiscreteIterationParameterName(scene, binding.pointName)
            ? discreteIterationDepth(value)
            : value;
          if (typeof nextValue === "number" && Number.isFinite(nextValue) && parameters.get(binding.pointName) !== nextValue) {
            parameters.set(binding.pointName, nextValue);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-distance-value"
          && typeof binding.name === "string"
        ) {
          const value = pointDistanceValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-angle-value"
          && typeof binding.name === "string"
        ) {
          const value = pointAngleValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "polygon-area-value"
          && typeof binding.name === "string"
        ) {
          const value = polygonAreaValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-distance-ratio-value"
          && typeof binding.name === "string"
        ) {
          const value = pointDistanceRatioValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-axis-value"
          && typeof binding.name === "string"
        ) {
          const point = scene.points[binding.pointIndex];
          if (!point) {
            return;
          }
          const value = binding.axis === "vertical" ? point.y : point.x;
          if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          (binding.kind === "expression-value" || binding.kind === "point-bound-expression-value")
          && (typeof binding.resultName === "string" || typeof binding.exprLabel === "string")
        ) {
          const value = evaluateExpr(binding.expr, 0, parameters);
          const resultNames = new Set<string>();
          if (typeof binding.resultName === "string") {
            resultNames.add(binding.resultName);
          }
          if (typeof binding.exprLabel === "string") {
            resultNames.add(binding.exprLabel);
          }
          resultNames.add(formatExpr(binding.expr, formatSequenceValue));
          if (typeof value === "number" && Number.isFinite(value)) {
            resultNames.forEach(( resultName: string) => {
              if (resultName && parameters.get(resultName) !== value) {
                parameters.set(resultName, value);
                changed = true;
              }
            });
          }
        }
      });
      if (!changed) {
        break;
      }
    }
    return parameters;
  }


  function deriveSequenceLabelParameters(scene: ViewerSceneData | null | undefined, seedParameters: Map<string, number>) {
    const sequenceLabels = (scene?.labels || [])
      .filter(( label) => label.binding?.kind === "sequence-expression-value");
    if (sequenceLabels.length === 0) {
      return seedParameters;
    }
    const parameters = new Map(seedParameters);
    const maxDepth = Math.max(
      ...sequenceLabels.map(( label) => pointIterationDepth({
        depth: label.binding.depth,
        parameterName: label.binding.depthParameterName,
      }, parameters)),
    );
    for (let step = 0; step <= maxDepth; step += 1) {
      const derived = deriveExpressionLabelParameters(scene, parameters);

      const updates = [];
      sequenceLabels.forEach(( label) => {
        const binding = label.binding;
        const depth = pointIterationDepth({
          depth: binding.depth,
          parameterName: binding.depthParameterName,
        }, derived);
        if (step > depth) {
          return;
        }
        const value = evaluateExpr(binding.expr, 0, derived);
        if (typeof value === "number" && Number.isFinite(value)) {
          updates.push([binding.parameterName, value]);
        }
      });
      if (updates.length === 0) {
        break;
      }
      updates.forEach(([name, value]) => parameters.set(name, value));
    }
    return deriveExpressionLabelParameters(scene, parameters);
  }


  function deriveLabelParameters(scene: ViewerSceneData | null | undefined, seedParameters: Map<string, number>) {
    return deriveSequenceLabelParameters(
      scene,
      deriveExpressionLabelParameters(scene, seedParameters),
    );
  }


  function parameterMapForScene(env: ViewerEnv, scene: ViewerSceneData) {
    return deriveLabelParameters(
      scene,
      new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value])),
    );
  }
  const {
    parameterRootId,
    sourcePointRootId,
    collectExprParameterNames,
    describeDependencyGraph,
    runDependencyGraph,
  } = modules.dynamicsDependencyGraph.createDependencyGraphRuntime({
    applyBaseDynamicUpdates,
    parameterMapForScene,
    refreshDerivedPoints,
    refreshDynamicLabels,
    refreshIterationGeometry,
  });

  function sampleDynamicFunction(functionDef: FunctionJson, parameters: Map<string, number>) {
    const segments = [];
    let points = [];
    const last = Math.max(1, functionDef.domain.sampleCount - 1);
    for (let index = 0; index < functionDef.domain.sampleCount; index += 1) {
      const t = index / last;
      const x = functionDef.domain.xMin + (functionDef.domain.xMax - functionDef.domain.xMin) * t;
      const y = evaluateExpr(functionDef.expr, x, parameters);
      if (y === null) {
        if (points.length >= 2) {
          segments.push(points);
        }
        points = [];
        continue;
      }
      if (functionDef.domain.plotMode === "polar") {
        points.push({
          x: y * Math.cos(x),
          y: y * Math.sin(x),
        });
      } else {
        points.push({ x, y });
      }
    }
    if (points.length >= 2) {
      segments.push(points);
    }
    return segments;
  }


  function sampleParametricCurve(binding: RuntimeLineBindingJson, parameters: Map<string, number>) {
    const points = [];
    const last = Math.max(1, binding.sampleCount - 1);
    for (let index = 0; index < binding.sampleCount; index += 1) {
      const t = index / last;
      const value = binding.xMin + (binding.xMax - binding.xMin) * t;
      const x = evaluateExpr(binding.xExpr, value, parameters);
      const y = evaluateExpr(binding.yExpr, value, parameters);
      if (x === null || y === null) continue;
      points.push({ x, y });
    }
    return points;
  }


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
    const denominatorLength = Math.hypot(denominator.x - origin.x, denominator.y - origin.y);
    if (denominatorLength <= 1e-9) return null;
    const ratio = Math.hypot(numerator.x - origin.x, numerator.y - origin.y) / denominatorLength;
    return binding.clampToUnit === true ? Math.min(ratio, 1) : ratio;
  }


  function pointDistanceValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const left = scene.points[binding.leftIndex];
    const right = scene.points[binding.rightIndex];
    if (!left || !right) return null;
    return Math.hypot(right.x - left.x, right.y - left.y) * (binding.valueScale ?? 1);
  }


  function pointAngleValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const start = scene.points[binding.startIndex];
    const vertex = scene.points[binding.vertexIndex];
    const end = scene.points[binding.endIndex];
    if (!start || !vertex || !end) return null;
    const first = { x: start.x - vertex.x, y: start.y - vertex.y };
    const second = { x: end.x - vertex.x, y: end.y - vertex.y };
    const firstLen = Math.hypot(first.x, first.y);
    const secondLen = Math.hypot(second.x, second.y);
    if (firstLen <= 1e-9 || secondLen <= 1e-9) return null;
    const cross = (first.x / firstLen) * (second.y / secondLen)
      - (first.y / firstLen) * (second.x / secondLen);
    const dot = (first.x / firstLen) * (second.x / secondLen)
      + (first.y / firstLen) * (second.y / secondLen);
    return Math.abs(Math.atan2(cross, dot)) * 180 / Math.PI;
  }


  function polygonAreaValue(scene: ViewerSceneData, binding: RuntimeLabelBindingJson) {
    const points = binding.pointIndices.map((index: number) => scene.points[index]);
    if (points.length < 3 || points.some((point) => !point)) return null;
    let twiceArea = 0;
    for (let index = 0; index < points.length; index += 1) {
      const left = points[index];
      const right = points[(index + 1) % points.length];
      twiceArea += left.x * right.y - right.x * left.y;
    }
    return Math.abs(twiceArea) * 0.5 * (binding.valueScale ?? 1);
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


  function segmentProjectionParameterFromBinding(scene: ViewerSceneData, binding: { pointIndex?: number; startIndex?: number; endIndex?: number }) {
    const point = scene.points[binding.pointIndex];
    const start = scene.points[binding.startIndex];
    const end = scene.points[binding.endIndex];
    return segmentProjectionParameterFromPoints(point, start, end);
  }


  function segmentProjectionParameterFromPoints(point: Point | null | undefined, start: Point | null | undefined, end: Point | null | undefined) {
    if (!point || !start || !end) return null;
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) return null;
    const t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSq;
    return t;
  }


  function polygonBoundaryParameterFromPoint(scene: ViewerSceneData, pointIndex: number) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (
      !constraint
      || (constraint.kind !== "polygon-boundary" && constraint.kind !== "translated-polygon-boundary")
      || constraint.vertexIndices.length < 2
    ) {
      return null;
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


  function polylineParameterFromPoint(scene: ViewerSceneData, pointIndex: number) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (constraint?.kind !== "polyline" || !Array.isArray(constraint.points) || constraint.points.length < 2) {
      return null;
    }
    const segmentIndex = Number.isFinite(constraint.segmentIndex) ? constraint.segmentIndex : 0;
    const t = Number.isFinite(constraint.t) ? Math.max(0, Math.min(1, constraint.t)) : 0;
    return (segmentIndex + t) / (constraint.points.length - 1);
  }


  function pointOnPolylineByIndex(points: Point[], normalized: number) {
    if (!Array.isArray(points) || points.length < 2 || !Number.isFinite(normalized)) {
      return null;
    }
    const wrapped = ((normalized % 1) + 1) % 1;
    const scaled = wrapped * (points.length - 1);
    const segmentIndex = Math.max(0, Math.min(points.length - 2, Math.floor(scaled)));
    const t = scaled - segmentIndex;
    const start = points[segmentIndex];
    const end = points[segmentIndex + 1];
    if (!start || !end) return null;
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }


  function pointOnPolygonBoundary(vertices: Point[], parameter: number) {
    if (!vertices || vertices.length < 2) {
      return null;
    }
    const wrapped = ((parameter % 1) + 1) % 1;
    const lengths = [];
    let perimeter = 0;
    for (let index = 0; index < vertices.length; index += 1) {
      const start = vertices[index];
      const end = vertices[(index + 1) % vertices.length];
      const length = Math.hypot(end.x - start.x, end.y - start.y);
      lengths.push(length);
      perimeter += length;
    }
    if (perimeter <= 1e-9) {
      return null;
    }
    const target = wrapped * perimeter;
    let traveled = 0;
    for (let edgeIndex = 0; edgeIndex < lengths.length; edgeIndex += 1) {
      const length = lengths[edgeIndex];
      if (traveled + length >= target || edgeIndex === lengths.length - 1) {
        const start = vertices[edgeIndex];
        const end = vertices[(edgeIndex + 1) % vertices.length];
        const localT = length <= 1e-9 ? 0 : Math.max(0, Math.min(1, (target - traveled) / length));
        return {
          x: start.x + (end.x - start.x) * localT,
          y: start.y + (end.y - start.y) * localT,
        };
      }
      traveled += length;
    }
    return null;
  }


  const POINT_CONSTRAINT_PARAMETER_READERS = {
    segment: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    line: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    "line-constraint": (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    ray: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    "ray-constraint": (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    polyline: (scene: ViewerSceneData, pointIndex: number) => scene.points[pointIndex]?.constraint?.t ?? null,
    "polygon-boundary": polygonBoundaryParameterFromPoint,
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
    polyline(point: RuntimeScenePointJson, _scene: ViewerSceneData, value: number) {
      point.constraint.t = wrapUnitInterval(value);
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
    if (binding.kind === "segment-projection-parameter") {
      return segmentProjectionParameterFromBinding(scene, binding);
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


  function parameterNameFromPoint(scene: ViewerSceneData, pointIndex: number) {
    for (const label of scene.labels || []) {
      const binding = label?.binding;
      if (!binding || binding.pointIndex !== pointIndex || typeof binding.pointName !== "string") {
        continue;
      }
      if (
        binding.kind === "polyline-parameter"
        || binding.kind === "polygon-boundary-parameter"
        || binding.kind === "circle-parameter"
      ) {
        return binding.pointName;
      }
    }
    return null;
  }


  function clampNormalizedValue(value: number | null | undefined) {
    return typeof value === "number" && Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : null;
  }


  function hsbToRgba(hue: number, saturation: number, brightness: number, alpha: number): [number, number, number, number] {
    const wrappedHue = wrapUnitInterval(hue);
    const s = Math.max(0, Math.min(1, saturation));
    const v = Math.max(0, Math.min(1, brightness));
    if (s <= 1e-9) {
      const channel = Math.round(v * 255);
      return [channel, channel, channel, alpha];
    }
    const scaled = wrappedHue * 6;
    const sector = Math.floor(scaled) % 6;
    const fraction = scaled - Math.floor(scaled);
    const p = v * (1 - s);
    const q = v * (1 - s * fraction);
    const t = v * (1 - s * (1 - fraction));
    const [r, g, b] = (() => {
      switch (sector) {
        case 0: return [v, t, p];
        case 1: return [q, v, p];
        case 2: return [p, v, t];
        case 3: return [p, q, v];
        case 4: return [t, p, v];
        default: return [v, p, q];
      }
    })();
    return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255), alpha];
  }


  function rgbaToHsb(color: [number, number, number, number]) {
    const red = color[0] / 255;
    const green = color[1] / 255;
    const blue = color[2] / 255;
    const max = Math.max(red, green, blue);
    const min = Math.min(red, green, blue);
    const delta = max - min;
    let hue = 0;
    if (delta > 1e-9) {
      if (max === red) hue = ((green - blue) / delta) / 6;
      else if (max === green) hue = (2 + (blue - red) / delta) / 6;
      else hue = (4 + (red - green) / delta) / 6;
    }
    return {
      hue: wrapUnitInterval(hue),
      saturation: max <= 1e-9 ? 0 : delta / max,
      brightness: max,
    };
  }


  function refreshPolygonColorBinding(scene: ViewerSceneData, polygon: RuntimePolygonJson) {
    const binding = polygon.colorBinding;
    if (!binding || binding.kind !== "spectrum") return;
    const value = parameterValueFromPoint(scene, binding.pointIndex);
    if (!isFiniteNumber(value) || !isFiniteNumber(binding.period) || binding.period <= 1e-9) return;
    const base = rgbaToHsb(binding.baseColor);
    const color = hsbToRgba(
      base.hue + (value - binding.baseValue) / binding.period,
      base.saturation,
      base.brightness,
      binding.baseColor[3],
    );
    polygon.color = color;
    polygon.outlineColor = darken(color, 80);
  }


  function refreshCircleFillColorBinding(scene: ViewerSceneData, circle: RuntimeCircleJson) {
    const binding = circle.fillColorBinding;
    if (!binding) return;
    if (binding.kind === "rgb") {
      const red = clampNormalizedValue(parameterValueFromPoint(scene, binding.redPointIndex));
      const green = clampNormalizedValue(parameterValueFromPoint(scene, binding.greenPointIndex));
      const blue = clampNormalizedValue(parameterValueFromPoint(scene, binding.bluePointIndex));
      if (red === null || green === null || blue === null) return;
      circle.fillColor = [
        Math.round(red * 255),
        Math.round(green * 255),
        Math.round(blue * 255),
        binding.alpha,
      ];
      return;
    }
    if (binding.kind === "hsb") {
      const hue = clampNormalizedValue(parameterValueFromPoint(scene, binding.huePointIndex));
      const saturation = clampNormalizedValue(parameterValueFromPoint(scene, binding.saturationPointIndex));
      const brightness = clampNormalizedValue(parameterValueFromPoint(scene, binding.brightnessPointIndex));
      if (hue === null || saturation === null || brightness === null) return;
      circle.fillColor = hsbToRgba(hue, saturation, brightness, binding.alpha);
    }
  }


  function applyNormalizedParameterToPoint(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number | null | undefined) {
    if (typeof value !== "number") return;
    if (!point.constraint) return;
    const applyParameter = POINT_CONSTRAINT_PARAMETER_APPLIERS[point.constraint.kind];
    if (applyParameter) {
      applyParameter(point, scene, value);
    }
  }


  function applyTraceValueToPoint(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number | null | undefined, xMin: number, xMax: number) {
    if (typeof value !== "number") return;
    if (!point?.constraint) return;
    if (point.constraint.kind === "circle" || point.constraint.kind === "circular-constraint") {
      point.constraint.unitX = Math.cos(value);
      point.constraint.unitY = -Math.sin(value);
      return;
    }
    if (
      point.constraint.kind === "line"
      || point.constraint.kind === "line-constraint"
      || point.constraint.kind === "ray"
      || point.constraint.kind === "ray-constraint"
    ) {
      applyNormalizedParameterToPoint(point, scene, value);
      return;
    }
    const normalized = Math.abs(xMax - xMin) <= 1e-9
      ? 0
      : Math.max(0, Math.min(1, (value - xMin) / (xMax - xMin)));
    applyNormalizedParameterToPoint(point, scene, normalized);
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
    const add = ( name: string) => {
      if (typeof name === "string" && name.length > 0) {
        names.add(name);
      }
    };
    (scene?.pointIterations || []).forEach((family) => {
      if ("parameterName" in family) {
        add(family.parameterName);
      }
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
    (scene?.polygonIterations || []).forEach((family) => add(family.parameterName));
    (scene?.labelIterations || []).forEach((family) => add(family.depthParameterName));
    (scene?.iterationTables || []).forEach((table) => add(table.depthParameterName));
    return names;
  }


  function isDiscreteIterationParameterName(scene: ViewerSceneData | SceneData | null | undefined, name: string) {
    return collectDiscreteIterationParameterNames(scene).has(name);
  }


  function affineMapFromTriangles(sourceTriangle: Point[], targetTriangle: Point[]) {
    const sourceOrigin = sourceTriangle[0];
    const su = {
      x: sourceTriangle[1].x - sourceOrigin.x,
      y: sourceTriangle[1].y - sourceOrigin.y,
    };
    const sv = {
      x: sourceTriangle[2].x - sourceOrigin.x,
      y: sourceTriangle[2].y - sourceOrigin.y,
    };
    const det = su.x * sv.y - su.y * sv.x;
    if (Math.abs(det) <= 1e-9) {
      return null;
    }
    const targetOrigin = targetTriangle[0];
    const tu = {
      x: targetTriangle[1].x - targetOrigin.x,
      y: targetTriangle[1].y - targetOrigin.y,
    };
    const tv = {
      x: targetTriangle[2].x - targetOrigin.x,
      y: targetTriangle[2].y - targetOrigin.y,
    };
    return ( point) => {
      const relative = { x: point.x - sourceOrigin.x, y: point.y - sourceOrigin.y };
      const u = (relative.x * sv.y - relative.y * sv.x) / det;
      const v = (su.x * relative.y - su.y * relative.x) / det;
      return {
        x: targetOrigin.x + tu.x * u + tv.x * v,
        y: targetOrigin.y + tu.y * u + tv.y * v,
      };
    };
  }


  function segmentPointCoefficients(sourceStart: Point, sourceEnd: Point, point: Point) {
    const dx = sourceEnd.x - sourceStart.x;
    const dy = sourceEnd.y - sourceStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) {
      return null;
    }
    const relativeX = point.x - sourceStart.x;
    const relativeY = point.y - sourceStart.y;
    return {
      alpha: (relativeX * dx + relativeY * dy) / lenSq,
      beta: (relativeX * -dy + relativeY * dx) / lenSq,
    };
  }


  function applySegmentCoefficients(segmentStart: Point, segmentEnd: Point, coeffs: { alpha: number; beta: number }) {
    const dx = segmentEnd.x - segmentStart.x;
    const dy = segmentEnd.y - segmentStart.y;
    return {
      x: segmentStart.x + coeffs.alpha * dx - coeffs.beta * dy,
      y: segmentStart.y + coeffs.alpha * dy + coeffs.beta * dx,
    };
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
    return label.binding.exprLabel.includes("°") || exprContainsPiAngle(label.binding.expr)
      ? `${value.toFixed(2)}°`
      : env.formatNumber(value);
  }


  function darken(color: [number, number, number, number], amount: number) {
    return [
      Math.max(0, color[0] - amount),
      Math.max(0, color[1] - amount),
      Math.max(0, color[2] - amount),
      color[3],
    ];
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
  function updateCoordinateSourcePoint(point: RuntimeScenePointJson, source: Point | null, parameters: Map<string, number>) {
    if (!source) return;
    const parameterValue = parameters.get(point.binding.name);
    if (!isFiniteNumber(parameterValue)) return;
    const exprParameters = new Map<string, number>(parameters);
    exprParameters.set(point.binding.name, parameterValue);
    const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
    if (offset === null) return;
    if (point.binding.axis === "horizontal") {
      point.x = source.x + offset;
      point.y = source.y;
      return;
    }
    point.x = source.x;
    point.y = source.y + offset;
  }


  function updateCoordinateSource2dPoint(point: RuntimeScenePointJson, source: Point | null, parameters: Map<string, number>) {
    if (!source) return;
    const xParameterValue = parameters.get(point.binding.xName);
    const yParameterValue = parameters.get(point.binding.yName);
    if (!isFiniteNumber(xParameterValue) || !isFiniteNumber(yParameterValue)) return;
    const exprParameters = new Map<string, number>(parameters);
    exprParameters.set(point.binding.xName, xParameterValue);
    exprParameters.set(point.binding.yName, yParameterValue);
    const dx = evaluateExpr(point.binding.xExpr, 0, exprParameters);
    const dy = evaluateExpr(point.binding.yExpr, 0, exprParameters);
    if (dx !== null && dy !== null) {
      point.x = source.x + dx;
      point.y = source.y + dy;
    }
  }


  function updateConstraintParameterizedPoint(point: RuntimeScenePointJson, scene: ViewerSceneData, value: number) {
    if (!Number.isFinite(value)) return;
    applyNormalizedParameterToPoint(point, scene, value);
  }


  function updateCustomTransformPoint(point: RuntimeScenePointJson, parameters: Map<string, number>, resolvePointAt: (pointIndex: number) => Point | null, parameterSourceScene: ViewerSceneData) {
    const value = parameterValueFromPoint(parameterSourceScene, point.binding.sourceIndex);
    if (!isFiniteNumber(value)) return;
    const exprParameters = new Map<string, number>(parameters);
    const names = new Set<string>();
    collectExprParameterNames(point.binding.distanceExpr, names);
    collectExprParameterNames(point.binding.angleExpr, names);
    names.forEach((name: string) => exprParameters.set(name, value));
    const distanceValue = evaluateExpr(point.binding.distanceExpr, value, exprParameters);
    const angleValue = evaluateExpr(point.binding.angleExpr, value, exprParameters);
    const origin = resolvePointAt(point.binding.originIndex);
    const axisEnd = resolvePointAt(point.binding.axisEndIndex);
    if (distanceValue === null || angleValue === null || !origin || !axisEnd) return;
    const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
    const radians = (baseAngle + angleValue * point.binding.angleDegreesScale) * Math.PI / 180;
    const distance = distanceValue * point.binding.distanceRawScale;
    point.x = origin.x + distance * Math.cos(radians);
    point.y = origin.y - distance * Math.sin(radians);
  }


  function updateScaleByRatioPoint(point: RuntimeScenePointJson, resolvePointAt: (pointIndex: number) => Point | null) {
    const source = resolvePointAt(point.binding.sourceIndex);
    const center = resolvePointAt(point.binding.centerIndex);
    const ratioOrigin = resolvePointAt(point.binding.ratioOriginIndex);
    const ratioDenominator = resolvePointAt(point.binding.ratioDenominatorIndex);
    const ratioNumerator = resolvePointAt(point.binding.ratioNumeratorIndex);
    if (!source || !center || !ratioOrigin || !ratioDenominator || !ratioNumerator) return;
    const scaled = scaleByThreePointRatio(
      source,
      center,
      ratioOrigin,
      ratioDenominator,
      ratioNumerator,
      point.binding.signed !== false,
      point.binding.clampToUnit === true,
    );
    if (!scaled) return;
    point.x = scaled.x;
    point.y = scaled.y;
  }


  function circumcenter(start: Point, mid: Point, end: Point) {
    const determinant = 2 * (
      start.x * (mid.y - end.y)
      + mid.x * (end.y - start.y)
      + end.x * (start.y - mid.y)
    );
    if (Math.abs(determinant) <= 1e-9) return null;
    const startSq = start.x * start.x + start.y * start.y;
    const midSq = mid.x * mid.x + mid.y * mid.y;
    const endSq = end.x * end.x + end.y * end.y;
    return {
      x: (
        startSq * (mid.y - end.y)
        + midSq * (end.y - start.y)
        + endSq * (start.y - mid.y)
      ) / determinant,
      y: (
        startSq * (end.x - mid.x)
        + midSq * (start.x - end.x)
        + endSq * (mid.x - start.x)
      ) / determinant,
    };
  }


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
    return null;
  }


  const DERIVED_POINT_BINDING_REFRESHERS = {
    "derived-parameter"(_env: ViewerEnv, scene: ViewerSceneData, point: RuntimeScenePointJson) {
      const value = typeof point.binding.parameterStartIndex === "number"
        && typeof point.binding.parameterEndIndex === "number"
        ? segmentProjectionParameterFromBinding(scene, {
            pointIndex: point.binding.sourceIndex,
            startIndex: point.binding.parameterStartIndex,
            endIndex: point.binding.parameterEndIndex,
          })
        : parameterValueFromPoint(scene, point.binding.sourceIndex);
      if (value !== null) {
        applyNormalizedParameterToPoint(point, scene, value);
      }
    },
    "constraint-parameter-expr"(_env: ViewerEnv, scene: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      const value = evaluateExpr(point.binding.expr, 0, parameters);
      if (isFiniteNumber(value)) {
        updateConstraintParameterizedPoint(point, scene, value);
      }
    },
    "constraint-parameter-from-point-expr"(_env: ViewerEnv, scene: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      const sourceValue = parameterValueFromPoint(scene, point.binding.sourceIndex);
      if (!isFiniteNumber(sourceValue)) return;
      const exprParameters = new Map<string, number>(parameters);
      if (point.binding.parameterName) {
        exprParameters.set(point.binding.parameterName, sourceValue);
      }
      const value = evaluateExpr(point.binding.expr, 0, exprParameters);
      if (value !== null) {
        updateConstraintParameterizedPoint(
          point,
          scene,
          point.binding.absoluteValue === true ? value : sourceValue + value,
        );
      }
    },
    "coordinate-source"(env: ViewerEnv, _scene: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      updateCoordinateSourcePoint(point, env.resolveScenePoint(point.binding.sourceIndex), parameters);
    },
    "coordinate-source-2d"(env: ViewerEnv, _scene: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      updateCoordinateSource2dPoint(point, env.resolveScenePoint(point.binding.sourceIndex), parameters);
    },
    derived(env: ViewerEnv, scene: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      const source = resolveScenePointInScene(env, scene, point.binding.sourceIndex);
      if (!source) return;
      const transform = point.binding.transform;
      if (transform.kind === "translate") {
        const vectorStart = resolveScenePointInScene(env, scene, transform.vectorStartIndex);
        const vectorEnd = resolveScenePointInScene(env, scene, transform.vectorEndIndex);
        if (!vectorStart || !vectorEnd) return;
        point.x = source.x + (vectorEnd.x - vectorStart.x);
        point.y = source.y + (vectorEnd.y - vectorStart.y);
        return;
      }
      if (transform.kind === "reflect") {
        const lineStart = resolveScenePointInScene(env, scene, transform.lineStartIndex);
        const lineEnd = resolveScenePointInScene(env, scene, transform.lineEndIndex);
        if (!lineStart || !lineEnd) return;
        const reflected = reflectAcrossLine(source, lineStart, lineEnd);
        if (!reflected) return;
        point.x = reflected.x;
        point.y = reflected.y;
        return;
      }
      if (transform.kind === "reflect-constraint") {
        const line = resolveLineConstraintPoints(
          ( index: number) => resolveScenePointInScene(env, scene, index),
          env.getViewBounds ? env.getViewBounds() : env.sourceScene.bounds,
          transform.line,
        );
        if (!line) return;
        const reflected = reflectAcrossLine(source, line[0], line[1]);
        if (!reflected) return;
        point.x = reflected.x;
        point.y = reflected.y;
        return;
      }
      if (transform.kind === "rotate") {
        const center = resolveScenePointInScene(env, scene, transform.centerIndex);
        if (!center) return;
        const angleDegrees = resolveRotateTransformAngleDegrees(
          transform,
          parameters,
          (index: number) => resolveScenePointInScene(env, scene, index),
        );
        if (!isFiniteNumber(angleDegrees)) return;
        const rotated = rotateAround(source, center, angleDegrees * Math.PI / 180);
        point.x = rotated.x;
        point.y = rotated.y;
        return;
      }
      if (transform.kind === "scale") {
        const center = resolveScenePointInScene(env, scene, transform.centerIndex);
        if (!center) return;
        const factor = resolveScaleTransformFactor(
          transform,
          parameters,
          (index: number) => resolveScenePointInScene(env, scene, index),
        );
        if (!isFiniteNumber(factor)) return;
        const scaled = scaleAround(source, center, factor);
        point.x = scaled.x;
        point.y = scaled.y;
      }
    },
    "scale-by-ratio"(env: ViewerEnv, _scene: ViewerSceneData, point: RuntimeScenePointJson) {
      updateScaleByRatioPoint(point, ( index: number) => env.resolveScenePoint(index));
    },
    circumcenter(env: ViewerEnv, scene: ViewerSceneData, point: RuntimeScenePointJson) {
      const start = resolveScenePointInScene(env, scene, point.binding.startIndex);
      const mid = resolveScenePointInScene(env, scene, point.binding.midIndex);
      const end = resolveScenePointInScene(env, scene, point.binding.endIndex);
      if (!start || !mid || !end) return;
      const center = circumcenter(start, mid, end);
      if (!center) return;
      point.x = center.x;
      point.y = center.y;
    },
    "custom-transform"(_env: ViewerEnv, scene: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      updateCustomTransformPoint(point, parameters, ( index: number) => scene.points[index], scene);
    },
  };


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
      let currentValue = parameters.get(label.binding.parameterName);
      if (!isFiniteNumber(currentValue)) return;
      const depth = pointIterationDepth({
        depth: label.binding.depth,
        parameterName: label.binding.depthParameterName,
      }, parameters);

      let value = null;
      for (let step = 0; step <= depth; step += 1) {
        const nextValue = evaluateRecursiveExpression(
          label.binding.expr,
          label.binding.parameterName,
          currentValue,
          parameters,
        );
        if (!isFiniteNumber(nextValue)) return;
        value = nextValue;
        currentValue = value;
      }
      if (value !== null) {
        label.text = formatSequenceValue(value);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
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
    "segment-projection-parameter"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson) {
      const value = segmentProjectionParameterFromBinding(scene, label.binding);
      if (value !== null) {
        label.text = usesVerboseParameterLabel(label)
          ? `${label.binding.pointName}在${label.binding.segmentName}上的t值 = ${env.formatNumber(value)}`
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
      const first = { x: start.x - vertex.x, y: start.y - vertex.y };
      const second = { x: end.x - vertex.x, y: end.y - vertex.y };
      const firstLen = Math.hypot(first.x, first.y);
      const secondLen = Math.hypot(second.x, second.y);
      if (firstLen <= 1e-9 || secondLen <= 1e-9) return;
      const cross = (first.x / firstLen) * (second.y / secondLen)
        - (first.y / firstLen) * (second.x / secondLen);
      const dot = (first.x / firstLen) * (second.x / secondLen)
        + (first.y / firstLen) * (second.y / secondLen);
      const value = Math.abs(Math.atan2(cross, dot)) * 180 / Math.PI;
      if (Number.isFinite(value)) {
        label.text = value.toFixed(label.binding.decimals);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "custom-transform-value"(env: ViewerEnv, scene: ViewerSceneData, label: RuntimeLabelJson, parameters: Map<string, number>) {
      const value = parameterValueFromPoint(scene, label.binding.pointIndex);
      if (!isFiniteNumber(value)) return;
      const exprParameters = new Map<string, number>(parameters);
      const names = new Set<string>();
      collectExprParameterNames(label.binding.expr, names);
      names.forEach((name: string) => exprParameters.set(name, value));
      const evaluated = evaluateExpr(label.binding.expr, value, exprParameters);
      if (evaluated !== null) {
        label.text = `${label.binding.exprLabel} = ${env.formatNumber(evaluated * label.binding.valueScale)}${label.binding.valueSuffix}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
  };


  const SYNC_DYNAMIC_POINT_BINDING_UPDATERS = {
    coordinate(_env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      const value = parameters.get(point.binding.name);
      if (!isFiniteNumber(value)) return;
      point.x = value;
      const y = evaluateExpr(point.binding.expr, 0, parameters);
      if (y !== null) {
        point.y = y;
      }
    },
    "coordinate-source"(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      updateCoordinateSourcePoint(point, draft.points[point.binding.sourceIndex], parameters);
    },
    "coordinate-source-2d"(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      updateCoordinateSource2dPoint(point, draft.points[point.binding.sourceIndex], parameters);
    },
    "custom-transform"(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      updateCustomTransformPoint(point, parameters, (index: number) => draft.points[index], draft);
    },
    "scale-by-ratio"(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson) {
      updateScaleByRatioPoint(point, (index: number) => draft.points[index]);
    },
    circumcenter(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson) {
      const start = draft.points[point.binding.startIndex];
      const mid = draft.points[point.binding.midIndex];
      const end = draft.points[point.binding.endIndex];
      if (!start || !mid || !end) return;
      const center = circumcenter(start, mid, end);
      if (!center) return;
      point.x = center.x;
      point.y = center.y;
    },
    "constraint-parameter-expr"(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      const value = evaluateExpr(point.binding.expr, 0, parameters);
      if (isFiniteNumber(value)) {
        updateConstraintParameterizedPoint(point, draft, value);
      }
    },
    "constraint-parameter-from-point-expr"(_env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, parameters: Map<string, number>) {
      const sourceValue = parameterValueFromPoint(draft, point.binding.sourceIndex);
      if (!isFiniteNumber(sourceValue)) return;
      const exprParameters = new Map<string, number>(parameters);
      if (point.binding.parameterName) {
        exprParameters.set(point.binding.parameterName, sourceValue);
      }
      const value = evaluateExpr(point.binding.expr, 0, exprParameters);
      if (value !== null) {
        updateConstraintParameterizedPoint(
          point,
          draft,
          point.binding.absoluteValue === true ? value : sourceValue + value,
        );
      }
    },
  };


  function resolveHostLinePoints(scene: ViewerSceneData, binding: HostLineBinding) {
    const hostBinding = binding;
    if (
      typeof hostBinding?.lineStartIndex === "number"
      && typeof hostBinding?.lineEndIndex === "number"
    ) {
      const start = scene.points[hostBinding.lineStartIndex];
      const end = scene.points[hostBinding.lineEndIndex];
      return start && end ? [start, end] : null;
    }
    if (typeof hostBinding?.lineIndex === "number") {
      const hostLine = scene.lines[hostBinding.lineIndex];
      return hostLine?.points?.length >= 2 ? hostLine.points : null;
    }
    return null;
  }


  function sampleCustomTransformTraceLine(scene: ViewerSceneData, line: RuntimeLineJson, parameters: Map<string, number>) {
    const point = scene.points[line.binding.pointIndex];
    const binding = point?.binding;
    if (binding?.kind !== "custom-transform" && Number.isInteger(line.binding.driverIndex)) {
      return samplePointTraceLine(scene, line, parameters);
    }
    if (binding?.kind !== "custom-transform") return null;
    const origin = scene.points[binding.originIndex];
    const axisEnd = scene.points[binding.axisEndIndex];
    const traceMax = parameterValueFromPoint(scene, binding.sourceIndex);
    if (!origin || !axisEnd || !isFiniteNumber(traceMax)) return null;
    const sampled = [];
    const last = Math.max(1, line.binding.sampleCount - 1);
    const maxValue = Math.max(line.binding.xMin, Math.min(line.binding.xMax, traceMax));
    for (let index = 0; index < line.binding.sampleCount; index += 1) {
      const value = line.binding.xMin + (maxValue - line.binding.xMin) * (index / last);
      const exprParameters = new Map<string, number>(parameters);
      const names = new Set<string>();
      collectExprParameterNames(binding.distanceExpr, names);
      collectExprParameterNames(binding.angleExpr, names);
      names.forEach((name: string) => exprParameters.set(name, value));
      const distanceValue = evaluateExpr(binding.distanceExpr, value, exprParameters);
      const angleValue = evaluateExpr(binding.angleExpr, value, exprParameters);
      if (distanceValue === null || angleValue === null) continue;
      const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
      const radians = (baseAngle + angleValue * binding.angleDegreesScale) * Math.PI / 180;
      const distance = distanceValue * binding.distanceRawScale;
      sampled.push({
        x: origin.x + distance * Math.cos(radians),
        y: origin.y - distance * Math.sin(radians),
      });
    }
    return sampled.length >= 2 ? sampled : null;
  }


  function cloneTracePoint(point: Point) {
    if (typeof structuredClone === "function") {
      return structuredClone(point);
    }
    return JSON.parse(JSON.stringify(point));
  }


  function samplePointTraceLine(scene: ViewerSceneData, line: RuntimeLineJson, parameters: Map<string, number>) {
    const driver = scene.points[line.binding.driverIndex];
    if (!driver?.constraint) return null;
    const tracedPoint = scene.points[line.binding.pointIndex];
    const sourceBinding = tracedPoint?.binding;
    const sourcePoint = sourceBinding?.kind === "coordinate-source-2d"
      ? scene.points[sourceBinding.sourceIndex]
      : null;
    if (sourceBinding?.kind === "coordinate-source-2d" && sourcePoint) {
      const baseParameters = deriveLabelParameters(scene, new Map<string, number>(parameters));
      const driverParameterName = parameterNameFromPoint(scene, line.binding.driverIndex);
      const xNames = new Set<string>();
      const yNames = new Set<string>();
      collectExprParameterNames(sourceBinding.xExpr, xNames);
      collectExprParameterNames(sourceBinding.yExpr, yNames);
      const sampled = [];
      const sampleCount = Math.max(2, line.binding.sampleCount || 2);
      const last = Math.max(1, sampleCount - 1);
      for (let index = 0; index < sampleCount; index += 1) {
        const value = line.binding.useMidpoints
          ? line.binding.xMin
            + (line.binding.xMax - line.binding.xMin) * ((index + 0.5) / sampleCount)
          : line.binding.xMin + (line.binding.xMax - line.binding.xMin) * (index / last);
        const exprParameters = new Map(baseParameters);
        if (driverParameterName) {
          exprParameters.set(driverParameterName, value);
        }
        xNames.forEach((name: string) => {
          if (!exprParameters.has(name)) {
            exprParameters.set(name, value);
          }
        });
        yNames.forEach((name: string) => {
          if (!exprParameters.has(name)) {
            exprParameters.set(name, value);
          }
        });
        const dx = evaluateExpr(sourceBinding.xExpr, 0, exprParameters);
        const dy = evaluateExpr(sourceBinding.yExpr, 0, exprParameters);
        if (dx === null || dy === null) {
          continue;
        }
        sampled.push({
          x: sourcePoint.x + dx,
          y: sourcePoint.y + dy,
        });
      }
      return sampled.length >= 2 ? sampled : null;
    }
    const sampleScene = {
      ...scene,
      lines: scene.lines,
      circles: scene.circles,

      points: [],
    };

    let baseParameters = new Map<string, number>(parameters);
    let driverValue = Number.NaN;

    let resolvedCache = new Map();


    const resolveTracePoint = (points, index: number, visiting= new Set<number>()) => {
      if (resolvedCache.has(index)) {
        return resolvedCache.get(index) ?? null;
      }
      if (visiting.has(index)) return null;
      const point = points[index];
      if (!point) return null;
      visiting.add(index);

      const applyDriverParameterGuesses = (exprParameters, ...exprs: FunctionExprJson[]) => {
        if (!Number.isFinite(driverValue)) return exprParameters;
        const names = new Set<string>();
        exprs.forEach((expr) => collectExprParameterNames(expr, names));
        names.forEach((name: string) => {
          if (!exprParameters.has(name)) {
            exprParameters.set(name, driverValue);
          }
        });
        return exprParameters;
      };

      let resolved = null;
      if (point.binding?.kind === "derived") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const transform = point.binding.transform;
        if (transform.kind === "translate") {
          const vectorStart = resolveTracePoint(points, transform.vectorStartIndex, visiting);
          const vectorEnd = resolveTracePoint(points, transform.vectorEndIndex, visiting);
          if (source && vectorStart && vectorEnd) {
            resolved = {
              x: source.x + (vectorEnd.x - vectorStart.x),
              y: source.y + (vectorEnd.y - vectorStart.y),
            };
          }
        } else if (transform.kind === "reflect") {
          const lineStart = resolveTracePoint(points, transform.lineStartIndex, visiting);
          const lineEnd = resolveTracePoint(points, transform.lineEndIndex, visiting);
          if (source && lineStart && lineEnd) {
            resolved = reflectAcrossLine(source, lineStart, lineEnd);
          }
        } else if (transform.kind === "reflect-constraint") {
          const line = resolveLineConstraintPoints(
            (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
            scene.bounds,
            transform.line,
          );
          if (source && line) {
            resolved = reflectAcrossLine(source, line[0], line[1]);
          }
        } else if (transform.kind === "rotate") {
          const center = resolveTracePoint(points, transform.centerIndex, visiting);
          const angleDegrees = resolveRotateTransformAngleDegrees(
            transform,
            baseParameters,
            (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
          );
          if (source && center && isFiniteNumber(angleDegrees)) {
            resolved = rotateAround(source, center, angleDegrees * Math.PI / 180);
          }
        } else if (transform.kind === "scale") {
          const center = resolveTracePoint(points, transform.centerIndex, visiting);
          const factor = resolveScaleTransformFactor(
            transform,
            baseParameters,
            (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
          );
          if (source && center && isFiniteNumber(factor)) {
            resolved = scaleAround(source, center, factor);
          }
        }
      } else if (point.binding?.kind === "scale-by-ratio") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const center = resolveTracePoint(points, point.binding.centerIndex, visiting);
        const ratioOrigin = resolveTracePoint(points, point.binding.ratioOriginIndex, visiting);
        const ratioDenominator = resolveTracePoint(points, point.binding.ratioDenominatorIndex, visiting);
        const ratioNumerator = resolveTracePoint(points, point.binding.ratioNumeratorIndex, visiting);
        if (source && center && ratioOrigin && ratioDenominator && ratioNumerator) {
          resolved = scaleByThreePointRatio(
            source,
            center,
            ratioOrigin,
            ratioDenominator,
            ratioNumerator,
            point.binding.signed !== false,
            point.binding.clampToUnit === true,
          );
        }
      } else if (point.binding?.kind === "midpoint") {
        const start = resolveTracePoint(points, point.binding.startIndex, visiting);
        const end = resolveTracePoint(points, point.binding.endIndex, visiting);
        if (start && end) {
          resolved = lerpPoint(start, end, 0.5);
        }
      } else if (point.binding?.kind === "derived-parameter") {
        let value = null;
        if (
          typeof point.binding.parameterStartIndex === "number"
          && typeof point.binding.parameterEndIndex === "number"
        ) {
          const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
          const start = resolveTracePoint(points, point.binding.parameterStartIndex, visiting);
          const end = resolveTracePoint(points, point.binding.parameterEndIndex, visiting);
          value = source && start && end
            ? segmentProjectionParameterFromPoints(source, start, end)
            : null;
        } else {
          value = parameterValueFromPoint(sampleScene, point.binding.sourceIndex);
        }
        if (isFiniteNumber(value)) {
          const derived = cloneTracePoint(point);
          updateConstraintParameterizedPoint(derived, sampleScene, value);
          sampleScene.points[index] = derived;
          resolved = modules.scene.resolveConstrainedPoint(
            {
              sourceScene: scene,
              currentScene: () => sampleScene,
              resolveScenePoint: (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
            },
            derived.constraint,
            (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
            derived,
          );
        }
      } else if (point.binding?.kind === "circumcenter") {
        const start = resolveTracePoint(points, point.binding.startIndex, visiting);
        const mid = resolveTracePoint(points, point.binding.midIndex, visiting);
        const end = resolveTracePoint(points, point.binding.endIndex, visiting);
        if (start && mid && end) {
          resolved = circumcenter(start, mid, end);
        }
      } else if (point.binding?.kind === "coordinate") {
        const exprParameters = applyDriverParameterGuesses(new Map(baseParameters), point.binding.expr);
        if (typeof point.binding.name === "string" && !exprParameters.has(point.binding.name) && Number.isFinite(driverValue)) {
          exprParameters.set(point.binding.name, driverValue);
        }
        const x = exprParameters.get(point.binding.name);
        const y = evaluateExpr(point.binding.expr, 0, exprParameters);
        if (isFiniteNumber(x) && y !== null) {
          resolved = { x, y };
        }
      } else if (point.binding?.kind === "coordinate-source") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const exprParameters = applyDriverParameterGuesses(new Map(baseParameters), point.binding.expr);
        if (typeof point.binding.name === "string" && !exprParameters.has(point.binding.name) && Number.isFinite(driverValue)) {
          exprParameters.set(point.binding.name, driverValue);
        }
        const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
        if (source && offset !== null) {
          resolved = point.binding.axis === "horizontal"
            ? { x: source.x + offset, y: source.y }
            : { x: source.x, y: source.y + offset };
        }
      } else if (point.binding?.kind === "coordinate-source-2d") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const exprParameters = applyDriverParameterGuesses(
          new Map(baseParameters),
          point.binding.xExpr,
          point.binding.yExpr,
        );
        const dx = evaluateExpr(point.binding.xExpr, 0, exprParameters);
        const dy = evaluateExpr(point.binding.yExpr, 0, exprParameters);
        if (source && dx !== null && dy !== null) {
          resolved = { x: source.x + dx, y: source.y + dy };
        }
      } else if (point.binding?.kind === "constraint-parameter-expr") {
        const value = evaluateExpr(point.binding.expr, 0, baseParameters);
        if (value !== null) {
          const derived = cloneTracePoint(point);
          updateConstraintParameterizedPoint(derived, sampleScene, value);
          sampleScene.points[index] = derived;
          resolved = modules.scene.resolveConstrainedPoint(
            {
              sourceScene: scene,
              currentScene: () => sampleScene,
              resolveScenePoint: (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
            },
            derived.constraint,
            (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
            derived,
          );
        }
      } else if (point.binding?.kind === "constraint-parameter-from-point-expr") {
        const sourceValue = parameterValueFromPoint(sampleScene, point.binding.sourceIndex);
        if (isFiniteNumber(sourceValue)) {
          const exprParameters = new Map(baseParameters);
          if (point.binding.parameterName) {
            exprParameters.set(point.binding.parameterName, sourceValue);
          }
          const exprValue = evaluateExpr(point.binding.expr, 0, exprParameters);
          if (exprValue !== null) {
            const derived = cloneTracePoint(point);
            updateConstraintParameterizedPoint(
              derived,
              sampleScene,
              point.binding.absoluteValue === true ? exprValue : sourceValue + exprValue,
            );
            sampleScene.points[index] = derived;
            resolved = modules.scene.resolveConstrainedPoint(
              {
                sourceScene: scene,
                currentScene: () => sampleScene,
                resolveScenePoint: (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
              },
              derived.constraint,
              (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
              derived,
            );
          }
        }
      } else if (point.binding?.kind === "custom-transform") {
        const derived = { ...point };
        updateCustomTransformPoint(derived, baseParameters, (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting), sampleScene);
        if (Number.isFinite(derived.x) && Number.isFinite(derived.y)) {
          resolved = { x: derived.x, y: derived.y };
        }
      }

      if (!resolved && point.constraint) {
        sampleScene.points = points;
        resolved = modules.scene.resolveConstrainedPoint(
          {
            sourceScene: scene,
            currentScene: () => sampleScene,
            resolveScenePoint: (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
          },
          point.constraint,
          (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
          point,
        );
      }

      visiting.delete(index);
      const finalPoint = resolved || (point.constraint ? null : point);
      resolvedCache.set(index, finalPoint);
      return finalPoint;
    };

    const sampled = [];
    const rawTraceMax = line.binding.kind === "custom-transform-trace"
      ? parameterValueFromPoint(scene, line.binding.driverIndex)
      : null;
    const sampleXMax = isFiniteNumber(rawTraceMax)
      ? Math.max(line.binding.xMin, Math.min(line.binding.xMax, rawTraceMax))
      : line.binding.xMax;
    const last = Math.max(1, line.binding.sampleCount - 1);
    for (let index = 0; index < line.binding.sampleCount; index += 1) {
      const value = line.binding.useMidpoints
        ? line.binding.xMin
          + (sampleXMax - line.binding.xMin) * ((index + 0.5) / Math.max(1, line.binding.sampleCount))
        : line.binding.xMin + (sampleXMax - line.binding.xMin) * (index / last);
      const points = scene.points.map(cloneTracePoint);
      sampleScene.points = points;
      applyTraceValueToPoint(
        points[line.binding.driverIndex],
        sampleScene,
        value,
        line.binding.xMin,
        line.binding.xMax,
      );
      const driverPoint = points[line.binding.driverIndex];
      const resolvedDriver = driverPoint?.constraint
        ? modules.scene.resolveConstrainedPoint(
          {
            sourceScene: scene,
            currentScene: () => sampleScene,
            resolveScenePoint: (pointIndex: number) => points[pointIndex],
          },
          driverPoint.constraint,
          (pointIndex: number) => points[pointIndex],
          driverPoint,
        )
        : null;
      if (resolvedDriver) {
        driverPoint.x = resolvedDriver.x;
        driverPoint.y = resolvedDriver.y;
      }
      baseParameters = deriveLabelParameters(sampleScene, new Map<string, number>(parameters));
      driverValue = parameterValueFromPoint(sampleScene, line.binding.driverIndex) ?? Number.NaN;
      resolvedCache = new Map();
      const point = resolveTracePoint(points, line.binding.pointIndex);
      if (point) {
        sampled.push({ x: point.x, y: point.y });
      }
    }
    return sampled.length >= 2 ? sampled : null;
  }

  type DerivedTransform =
    | { kind: "translate"; dx: number; dy: number }
    | { kind: "rotate"; center: Point; radians: number }
    | { kind: "scale"; center: Point; factor: number }
    | { kind: "reflect"; lineStart: Point; lineEnd: Point };


  function resolveDerivedTransform(transform: TransformJson | null | undefined, scene: ViewerSceneData, parameters: Map<string, number>): DerivedTransform | null {
    if (!transform) return null;
    if (transform.kind === "translate") {
      const vectorStart = scene.points[transform.vectorStartIndex];
      const vectorEnd = scene.points[transform.vectorEndIndex];
      if (!vectorStart || !vectorEnd) return null;
      return {
        kind: "translate",
        dx: vectorEnd.x - vectorStart.x,
        dy: vectorEnd.y - vectorStart.y,
      };
    }
    if (transform.kind === "translate-delta") {
      return { kind: "translate", dx: transform.dx, dy: transform.dy };
    }
    if (transform.kind === "rotate") {
      const center = scene.points[transform.centerIndex];
      if (!center) return null;
      const angleDegrees = resolveRotateTransformAngleDegrees(
        transform,
        parameters,
        (index: number) => scene.points[index] || null,
      );
      if (!isFiniteNumber(angleDegrees)) return null;
      return { kind: "rotate", center, radians: angleDegrees * Math.PI / 180 };
    }
    if (transform.kind === "scale") {
      const center = scene.points[transform.centerIndex];
      if (!center) return null;
      const factor = resolveScaleTransformFactor(
        transform,
        parameters,
        (pointIndex: number) => scene.points[pointIndex] || null,
      );
      if (!isFiniteNumber(factor)) return null;
      return { kind: "scale", center, factor };
    }
    if (transform.kind === "reflect") {
      const [lineStart, lineEnd] = reflectionAxisPoints(scene, transform);
      if (!lineStart || !lineEnd) return null;
      return { kind: "reflect", lineStart, lineEnd };
    }
    return null;
  }


  function applyDerivedTransform(point: Point, transform: DerivedTransform) {
    if (transform.kind === "translate") {
      return { x: point.x + transform.dx, y: point.y + transform.dy };
    }
    if (transform.kind === "rotate") {
      return rotateAround(point, transform.center, transform.radians);
    }
    if (transform.kind === "scale") {
      return scaleAround(point, transform.center, transform.factor);
    }
    return reflectAcrossLine(point, transform.lineStart, transform.lineEnd);
  }


  function refreshDerivedLine(env: { scene: ViewerSceneData, parameters: Map<string, number> }, line: RuntimeLineJson) {
    const source = env.scene.lines[line.binding.sourceIndex];
    const transform = resolveDerivedTransform(line.binding.transform, env.scene, env.parameters);
    if (!source || !transform) return;
    const nextPoints = source.points
      .map(( point) => applyDerivedTransform(point, transform));
    if (nextPoints.some(( point) => !point)) return;
    line.points =  (nextPoints);
  }


  function refreshColorizedSpectrumLine(context: { scene: ViewerSceneData, bounds: BoundsJson, parameters: Map<string, number> }, line: RuntimeLineJson) {
    const binding = line.binding;
    const hostLine = context.scene.lines[binding.lineIndex];
    const traceLine = context.scene.lines[binding.traceLineIndex];
    const baseParameter = polylineParameterFromPoint(context.scene, binding.pointIndex);
    if (!traceLine?.points || traceLine.points.length < 2 || !isFiniteNumber(baseParameter)) {
      return;
    }
    const rawDepth = binding.depthParameterName
      ? context.parameters.get(binding.depthParameterName)
      : binding.depth;
    const depth = discreteIterationDepth(isFiniteNumber(rawDepth) ? rawDepth : binding.depth);
    line.visible = binding.stepIndex < depth;
    if (depth <= 0 || binding.stepIndex >= depth) {
      return;
    }
    line.color = hsbToRgba((binding.stepIndex || 0) / depth, 1, 1, 255);
    const sample = pointOnPolylineByIndex(
      traceLine.points,
      baseParameter + (binding.stepIndex || 0) / depth,
    );
    if (!sample) return;

    const hostPoints = hostLine?.points;
    if (!hostPoints || hostPoints.length < 2) return;
    const traceEndpointIndex = binding.traceEndpointIndex === 1 ? 1 : 0;
    const hostStart = hostPoints[traceEndpointIndex];
    let hostEnd = hostPoints[1 - traceEndpointIndex];
    let rayStart = hostStart;
    let rayEnd = hostEnd;
    if (
      isFiniteNumber(binding.reflectionSourceIndex)
      && isFiniteNumber(binding.reflectionAxisLineIndex)
    ) {
      const source = context.scene.points[binding.reflectionSourceIndex];
      const sampledAxis = sampledReflectionAxis(context.scene, binding, sample);
      const axisLine = sampledAxis ? null : context.scene.lines[binding.reflectionAxisLineIndex];
      const axisStart = sampledAxis?.[0] ?? axisLine?.points?.[0];
      const axisEnd = sampledAxis?.[1] ?? axisLine?.points?.[axisLine.points.length - 1];
      if (source && axisStart && axisEnd) {
        const reflected = reflectAcrossLine(source, axisStart, axisEnd);
        if (reflected) {
          if (sampledAxis && binding.ray) {
            rayStart = reflected;
            rayEnd = sample;
          } else {
            rayStart = sample;
            rayEnd = reflected;
          }
          hostEnd = reflected;
        }
      }
    }
    if (!hostStart || !hostEnd || !rayStart || !rayEnd) return;

    if (binding.ray) {
      const dx = rayEnd.x - rayStart.x;
      const dy = rayEnd.y - rayStart.y;
      if (Math.hypot(dx, dy) <= 1e-9) return;
      const clipped = clipRayToBounds(sample, { x: sample.x + dx, y: sample.y + dy }, context.bounds);
      if (clipped) {
        line.points = clipped;
      }
      return;
    }

    line.points = [sample, { x: hostEnd.x, y: hostEnd.y }];
  }


  function sampledReflectionAxis(scene: ViewerSceneData, binding: RuntimeLineBindingJson, sample: Point) {
    if (
      !isFiniteNumber(binding.reflectionFocusIndex)
      || !isFiniteNumber(binding.reflectionDirectrixLineIndex)
    ) {
      return null;
    }
    const focus = scene.points[binding.reflectionFocusIndex];
    const directrixLine = scene.lines[binding.reflectionDirectrixLineIndex];
    const directrixStart = directrixLine?.points?.[0];
    const directrixEnd = directrixLine?.points?.[directrixLine.points.length - 1];
    if (!focus || !directrixStart || !directrixEnd) return null;
    const projection = projectPointToLine(sample, directrixStart, directrixEnd);
    if (!projection) return null;
    const normalX = focus.x - projection.x;
    const normalY = focus.y - projection.y;
    if (Math.hypot(normalX, normalY) <= 1e-9) return null;
    return [
      sample,
      { x: sample.x - normalY, y: sample.y + normalX },
    ];
  }


  function projectPointToLine(point: Point, lineStart: Point, lineEnd: Point) {
    const dx = lineEnd.x - lineStart.x;
    const dy = lineEnd.y - lineStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) return null;
    const t = ((point.x - lineStart.x) * dx + (point.y - lineStart.y) * dy) / lenSq;
    return {
      x: lineStart.x + t * dx,
      y: lineStart.y + t * dy,
    };
  }


  function refreshDerivedPolygon(env: { scene: ViewerSceneData, parameters: Map<string, number>, resolveHandle: (handle: PointHandle) => Point | null }, polygon: RuntimePolygonJson) {
    const source = env.scene.polygons[polygon.binding.sourceIndex];
    const transform = resolveDerivedTransform(polygon.binding.transform, env.scene, env.parameters);
    if (!source || !transform) return;
    const nextPoints = source.points
      .map(( handle) => env.resolveHandle(handle))
      .filter(Boolean)
      .map(( point) => applyDerivedTransform(point, transform));
    if (nextPoints.some(( point) => !point)) return;
    polygon.points =  (nextPoints);
  }


  function refreshDerivedCircle(env: { scene: ViewerSceneData, parameters: Map<string, number>, resolveHandle: (handle: PointHandle) => Point | null }, circle: RuntimeCircleJson) {
    const source = env.scene.circles[circle.binding.sourceIndex];
    const transform = resolveDerivedTransform(circle.binding.transform, env.scene, env.parameters);
    if (!source || !transform) return;
    const sourceCenter = env.resolveHandle(source.center);
    const sourceRadius = env.resolveHandle(source.radiusPoint);
    if (!sourceCenter || !sourceRadius) return;
    const nextCenter = applyDerivedTransform(sourceCenter, transform);
    const nextRadius = applyDerivedTransform(sourceRadius, transform);
    if (!nextCenter || !nextRadius) return;
    circle.center = nextCenter;
    circle.radiusPoint = nextRadius;
  }


  const LINE_BINDING_REFRESHERS = {
    segment({ scene }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      if (start && end) {
        line.points = [{ x: start.x, y: start.y }, { x: end.x, y: end.y }];
      }
    },
    "angle-marker"({ scene }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const start = scene.points[line.binding.startIndex];
      const vertex = scene.points[line.binding.vertexIndex];
      const end = scene.points[line.binding.endIndex];
      const points = start && vertex && end
        ? resolveAngleMarkerPoints(start, vertex, end, line.binding.markerClass)
        : null;
      if (points) {
        line.points = points;
      }
    },
    "angle-bisector-ray"({ scene, bounds }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const start = scene.points[line.binding.startIndex];
      const vertex = scene.points[line.binding.vertexIndex];
      const end = scene.points[line.binding.endIndex];
      if (start && vertex && end) {
        const direction = angleBisectorDirection(start, vertex, end);
        const clipped = direction
          ? clipRayToBounds(
              vertex,
              { x: vertex.x + direction.x, y: vertex.y + direction.y },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    "perpendicular-line"({ scene, bounds }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const through = scene.points[line.binding.throughIndex];
      const hostLine = resolveHostLinePoints(scene, line.binding);
      const lineStart = hostLine?.[0];
      const lineEnd = hostLine?.[1];
      if (through && lineStart && lineEnd) {
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        const clipped = len > 1e-9
          ? clipLineToBounds(
              through,
              { x: through.x - dy / len, y: through.y + dx / len },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    "parallel-line"({ scene, bounds }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const through = scene.points[line.binding.throughIndex];
      const hostLine = resolveHostLinePoints(scene, line.binding);
      const lineStart = hostLine?.[0];
      const lineEnd = hostLine?.[1];
      if (through && lineStart && lineEnd) {
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        const clipped = len > 1e-9
          ? clipLineToBounds(
              through,
              { x: through.x + dx / len, y: through.y + dy / len },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    line({ scene, bounds }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      const clipped = start && end ? clipLineToBounds(start, end, bounds) : null;
      if (clipped) line.points = clipped;
    },
    ray({ scene, bounds }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      const clipped = start && end ? clipRayToBounds(start, end, bounds) : null;
      if (clipped) line.points = clipped;
    },
    "arc-boundary"({ env }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const sampled = modules.scene.sampleArcBoundaryPoints(env, line.binding);
      if (sampled) {
        line.points = sampled;
      }
    },
    derived: refreshDerivedLine,
    "custom-transform-trace"({ scene, parameters }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const sampled = sampleCustomTransformTraceLine(scene, line, parameters);
      if (sampled) {
        line.points = sampled;
      }
    },
    "coordinate-trace"({ env }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const sampled = modules.scene.sampleCoordinateTracePoints(env, line.binding);
      if (sampled && sampled.length >= 2) {
        line.points = sampled;
      }
    },
    "point-trace"({ scene, parameters }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const sampled = samplePointTraceLine(scene, line, parameters);
      if (sampled) {
        line.points = sampled;
      }
    },
    "colorized-spectrum": refreshColorizedSpectrumLine,
    "parametric-curve"({ parameters }: LineBindingRefreshContext, line: RuntimeLineJson) {
      const sampled = sampleParametricCurve(line.binding, parameters);
      if (sampled.length >= 2) {
        line.points = sampled;
      }
    },
  };


  function resolveScenePointInScene(env: ViewerEnv, scene: ViewerSceneData, index: number, visiting: Set<number> = new Set<number>()) {
    const point = scene.points[index];
    if (!point) return null;
    if (!point.constraint) return point;
    if (visiting.has(index)) return null;
    visiting.add(index);
    const resolved = modules.scene.resolveConstrainedPoint(
      env,
      point.constraint,
      (pointIndex: number) => resolveScenePointInScene(env, scene, pointIndex, visiting),
      point,
    );
    visiting.delete(index);
    return resolved;
  }


  const CIRCLE_BINDING_REFRESHERS = {
    "point-radius-circle"({ env }: CircleBindingRefreshContext, circle: RuntimeCircleJson) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const radiusPoint = env.resolveScenePoint(circle.binding.radiusIndex);
      if (!center || !radiusPoint) return;
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: radiusPoint.x, y: radiusPoint.y };
    },
    "segment-radius-circle"({ env }: CircleBindingRefreshContext, circle: RuntimeCircleJson) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const lineStart = env.resolveScenePoint(circle.binding.lineStartIndex);
      const lineEnd = env.resolveScenePoint(circle.binding.lineEndIndex);
      if (!center || !lineStart || !lineEnd) return;
      const radius = Math.hypot(lineEnd.x - lineStart.x, lineEnd.y - lineStart.y);
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: center.x + radius, y: center.y };
    },
    "parameter-radius-circle"({ env, parameters }: CircleBindingRefreshContext, circle: RuntimeCircleJson) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const value = parameters.get(circle.binding.parameterName);
      if (!center || !isFiniteNumber(value)) return;
      const radius = Math.abs(value) * circle.binding.rawPerUnit;
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: center.x + radius, y: center.y };
    },
    derived: refreshDerivedCircle,
  };


  const POLYGON_BINDING_REFRESHERS = {
    "point-polygon"({ scene }: PolygonBindingRefreshContext, polygon: RuntimePolygonJson) {
      const points = polygon.binding.vertexIndices
        .map(( index: number) => scene.points[index])
        .filter(Boolean);
      if (points.length === polygon.binding.vertexIndices.length) {
        polygon.points = points.map(( point) => ({ x: point.x, y: point.y }));
      }
    },
    "arc-boundary-polygon"({ env }: PolygonBindingRefreshContext, polygon: RuntimePolygonJson) {
      const sampled = modules.scene.sampleArcBoundaryPoints(env, polygon.binding);
      if (sampled) {
        polygon.points = sampled;
      }
    },
    derived: refreshDerivedPolygon,
  };


  function refreshDerivedPoints(env: ViewerEnv, scene: ViewerSceneData) {
    const bounds = env.getViewBounds ? env.getViewBounds() : (scene.bounds || env.sourceScene.bounds);
    let parameters = parameterMapForScene(env, scene);
    refreshConstrainedPointPositions(env, scene);
    parameters = parameterMapForScene(env, scene);
    const resolveHandle = ( handle) => {
      if (hasPointIndexHandle(handle)) {
        return env.resolveScenePoint(handle.pointIndex);
      }
      if (hasLineIndexHandle(handle)) {
        const line = scene.lines[handle.lineIndex];
        if (!line?.points || line.points.length < 2) return null;
        const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
        const t = typeof handle.t === "number" ? handle.t : 0.5;
        const p0 = line.points[segmentIndex];
        const p1 = line.points[segmentIndex + 1];
        return {
          x: p0.x + (p1.x - p0.x) * t,
          y: p0.y + (p1.y - p0.y) * t,
        };
      }
      return  (handle);
    };

    scene.points.forEach(( point) => {
      const refreshBinding = point.binding ? DERIVED_POINT_BINDING_REFRESHERS[point.binding.kind] : null;
      if (refreshBinding) {
        refreshBinding(env, scene, point, parameters);
      }
    });

    refreshConstrainedPointPositions(env, scene);
    parameters = parameterMapForScene(env, scene);


    const preservedLines = [];
    const lineContext = { env, scene, bounds, parameters };
    scene.lines.forEach(( line) => {
      const bindingKind = line.binding?.kind;
      if (!bindingKind) {
        preservedLines.push(line);
        return;
      }
      const refreshLine = LINE_BINDING_REFRESHERS[bindingKind];
      if (refreshLine) {
        refreshLine(lineContext, line);
      }
      preservedLines.push(line);
    });
    scene.lines = preservedLines;
    refreshTraceConstrainedPointPositions(env, scene);

    const shapeContext = { env, scene, parameters, resolveHandle };
    scene.circles.forEach(( circle) => {
      const refreshCircle = circle.binding ? CIRCLE_BINDING_REFRESHERS[circle.binding.kind] : null;
      if (refreshCircle) {
        refreshCircle(shapeContext, circle);
      }
      refreshCircleFillColorBinding(scene, circle);
    });

    const sourceCircleIterations = env.sourceScene.circleIterations || [];
    if (sourceCircleIterations.length > 0) {
      const generatedCount = sourceCircleIterations.reduce((sum, family) => sum + family.depth, 0);
      const baseCount = Math.max(0, env.sourceScene.circles.length - generatedCount);
      scene.circles = scene.circles.slice(0, baseCount);
      sourceCircleIterations.forEach(( family) => {
        const source = scene.circles[family.sourceCircleIndex];
        if (!source) {
          return;
        }
        const vertices = family.vertexIndices
          .map(( index: number) => scene.points[index])
          .filter(Boolean);
        if (vertices.length !== family.vertexIndices.length) {
          return;
        }
        const liveSeedParameter =
          polygonBoundaryParameterFromPoint(scene, family.sourceCenterIndex);
        const liveNextParameter =
          polygonBoundaryParameterFromPoint(scene, family.sourceNextCenterIndex);
        const seedParameter = isFiniteNumber(liveSeedParameter)
          ? liveSeedParameter
          : family.seedParameter;
        const stepParameter = isFiniteNumber(liveSeedParameter) && isFiniteNumber(liveNextParameter)
          ? ((liveNextParameter - liveSeedParameter) % 1 + 1) % 1
          : family.stepParameter;
        if (!isFiniteNumber(seedParameter) || !isFiniteNumber(stepParameter)) {
          return;
        }
        const depth = pointIterationDepth({
          depth: family.depth,
          parameterName: family.depthParameterName,
        }, parameters);
        const dx = source.radiusPoint.x - source.center.x;
        const dy = source.radiusPoint.y - source.center.y;
        for (let step = 1; step <= depth; step += 1) {
          const center = pointOnPolygonBoundary(
            vertices,
            seedParameter + stepParameter * step,
          );
          if (!center) {
            continue;
          }
          scene.circles.push({
            center,
            radiusPoint: {
              x: center.x + dx,
              y: center.y + dy,
            },
            color: source.color,
            fillColor: source.fillColor,
            fillVisible: source.fillVisible !== false,
            fillColorBinding: null,
            dashed: source.dashed,
            visible: family.visible !== false,
            binding: null,
          });
        }
      });
    }

    scene.polygons.forEach(( polygon) => {
      const refreshPolygon = polygon.binding ? POLYGON_BINDING_REFRESHERS[polygon.binding.kind] : null;
      if (refreshPolygon) {
        refreshPolygon(shapeContext, polygon);
      }
      refreshPolygonColorBinding(scene, polygon);
    });
  }


  function refreshConstrainedPointPositions(env: ViewerEnv, scene: ViewerSceneData) {
    scene.points.forEach(( point,  pointIndex: number) => {
      if (!point.constraint) {
        return;
      }
      const resolved = resolveScenePointInScene(env, scene, pointIndex);
      if (!resolved) {
        return;
      }
      point.x = resolved.x;
      point.y = resolved.y;
    });
  }


  function refreshTraceConstrainedPointPositions(env: ViewerEnv, scene: ViewerSceneData) {
    scene.points.forEach(( point,  pointIndex: number) => {
      if (point.constraint?.kind !== "polyline" || typeof point.constraint.functionKey !== "number") {
        return;
      }
      const resolved = resolveScenePointInScene(env, scene, pointIndex);
      if (!resolved) {
        return;
      }
      point.x = resolved.x;
      point.y = resolved.y;
    });
  }


  function reflectionAxisPoints(scene: ViewerSceneData, binding: { lineStartIndex?: number | null, lineEndIndex?: number | null, lineIndex?: number | null }) {
    const lineIndex = binding.lineIndex;
    if (typeof lineIndex === "number" && Number.isInteger(lineIndex)) {
      const axis = scene.lines[lineIndex];
      if (axis?.points?.length >= 2) {
        return [axis.points[0], axis.points[axis.points.length - 1]];
      }
    }
    const lineStartIndex = binding.lineStartIndex;
    const lineEndIndex = binding.lineEndIndex;
    const lineStart = typeof lineStartIndex === "number" && Number.isInteger(lineStartIndex)
      ? scene.points[lineStartIndex]
      : null;
    const lineEnd = typeof lineEndIndex === "number" && Number.isInteger(lineEndIndex)
      ? scene.points[lineEndIndex]
      : null;
    return [lineStart || null, lineEnd || null];
  }


  function refreshDynamicLabels(env: ViewerEnv, scene: ViewerSceneData) {
    const parameters = parameterMapForScene(env, scene);
    scene.labels.forEach(( label) => {
      if (!label.binding) return;
      const refreshLabel = DYNAMIC_LABEL_REFRESHERS[label.binding.kind];
      if (refreshLabel) {
        refreshLabel(env, scene, label, parameters);
      }
    });
  }


  function applyBaseDynamicUpdates(env: ViewerEnv, draft: ViewerSceneData, parameters: Map<string, number>) {
    env.currentDynamics().parameters.forEach(( parameter) => {
      if (typeof parameter.labelIndex === "number" && draft.labels[parameter.labelIndex]) {
        draft.labels[parameter.labelIndex].text =
          `${parameter.name} = ${env.formatNumber(parameter.value)}${parameterValueSuffix(parameter)}`;
      }
    });
    draft.points.forEach(( point) => {
      if (point.binding?.kind !== "parameter" || !point.constraint) {
        const updatePoint = point.binding ? SYNC_DYNAMIC_POINT_BINDING_UPDATERS[point.binding.kind] : null;
        if (updatePoint) {
          updatePoint(env, draft, point, parameters);
        }
        return;
      }
      const value = parameters.get(point.binding.name);
      if (!Number.isFinite(value)) return;
      applyNormalizedParameterToPoint(point, draft, value);
    });
    env.currentDynamics().functions.forEach(( functionDef) => {
      if (typeof functionDef.labelIndex === "number" && draft.labels[functionDef.labelIndex]) {
        const variableLabel = functionDef.domain.plotMode === "polar" ? "θ" : "x";
        const head = functionDef.domain.plotMode === "polar"
          ? (functionDef.derivative ? `r'(${variableLabel})` : "r")
          : (functionDef.derivative
            ? `${functionDef.name}'(${variableLabel})`
            : `${functionDef.name}(${variableLabel})`);
        draft.labels[functionDef.labelIndex].text = `${head} = ${formatExpr(functionDef.expr, env.formatAxisNumber, variableLabel)}`;
      }
      const sampledSegments = sampleDynamicFunction(functionDef, parameters);
      const sampled = sampledSegments.flat();
      if (typeof functionDef.lineIndex === "number" && draft.lines[functionDef.lineIndex]) {
        draft.lines[functionDef.lineIndex].points = sampled.map((point) => ({ ...point }));
        draft.lines[functionDef.lineIndex].segments = sampledSegments
          .map((segment) => segment.map((point) => ({ ...point })));
      }
      functionDef.constrainedPointIndices.forEach(( pointIndex: number) => {
        const constraint = draft.points[pointIndex]?.constraint;
        if (constraint && constraint.kind === "polyline") {
          constraint.points = sampled.map((point) => ({ ...point }));
          constraint.segmentIndex = Math.min(constraint.segmentIndex, Math.max(0, sampled.length - 2));
        }
      });
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
    rebuildIterationPoints,
    rebuildIteratedLines,
    rebuildIteratedPolygons,
    rebuildIteratedLabels,
    rebuildIterationTables,
  } = modules.dynamicsIterations.createDynamicsIterations({
    affineMapFromTriangles,
    applyNormalizedParameterToPoint,
    applySegmentCoefficients,
    buildPlainTextRichMarkup,
    cloneTracePoint,
    darken,
    deriveExpressionLabelParameters,
    deriveLabelParameters,
    discreteIterationDepth,
    DERIVED_POINT_BINDING_REFRESHERS,
    evaluateExpr,
    evaluateRecursiveExpression,
    formatSequenceValue,
    hasLineIndexHandle,
    hasPointIndexHandle,
    isFiniteNumber,
    pointIterationDepth,
    pointAngleValue,
    refreshDerivedPoints,
    rotateAround,
    samplePointTraceLine,
    segmentPointCoefficients,
    SYNC_DYNAMIC_POINT_BINDING_UPDATERS,
  });


  function refreshIterationGeometry(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    rebuildIterationPoints(env, scene, parameters);
    rebuildIteratedLines(env, scene, parameters);
    rebuildIteratedPolygons(env, scene, parameters);
    rebuildIteratedLabels(env, scene, parameters);
    rebuildIterationTables(env, scene, parameters);
    // Point iteration rebuilds replace the exported iteration tail. Re-resolve
    // the preserved base graph afterwards so bindings that depend on a moved
    // source point are not left with their pre-rebuild coordinates.
    refreshDerivedPoints(env, scene);
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
    formatExpr,
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
