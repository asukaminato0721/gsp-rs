(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function createDynamicsGeometry(dependencies: RuntimeDynamicsGeometryDependencies) {
    const {
      applyTraceValueToPoint,
      markedAngleTranslationPoint,
      circumcenter,
      clipRayToBounds,
      deriveLabelParameters,
      discreteIterationDepth,
      evaluateExpr,
      hsbToRgba,
      isFiniteNumber,
      lerpPoint,
      lineProjectionParameterFromPoints,
      parameterValueFromPoint,
      pointOnPolylineByIndex,
      polylineParameterFromPoint,
      reflectAcrossLine,
      resolveLineConstraintPoints,
      resolveRotateTransformAngleDegrees,
      resolveScaleTransformFactor,
      rotateAround,
      scaleAround,
      scaleByThreePointRatio,
      updateConstraintParameterizedPoint,
      updateCustomTransformPoint,
      updatePolarTransformPoint,
    } = dependencies;

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
    const sampled = window.GspRuntimeCore.sampleCustomTransformTrace(
      binding.distanceExpr,
      binding.angleExpr,
      parameters,
      origin,
      axisEnd,
      line.binding.xMin,
      line.binding.xMax,
      traceMax,
      line.binding.sampleCount,
      binding.distanceRawScale,
      binding.angleDegreesScale,
    );
    return sampled.length >= 2 ? sampled : null;
  }
  
  
  function cloneTracePoint(point: Point) {
    if (typeof structuredClone === "function") {
      return structuredClone(point);
    }
    return JSON.parse(JSON.stringify(point));
  }
  
  
  function samplePointTraceTargets(
    scene: ViewerSceneData,
    line: RuntimeLineJson,
    parameters: Map<string, number>,
    targetPointIndices: number[],
  ) {
    const driver = scene.points[line.binding.driverIndex];
    if (!driver?.constraint) return null;
    const tracedPoint = targetPointIndices.length === 1
      ? scene.points[targetPointIndices[0]]
      : null;
    const sourceBinding = tracedPoint?.binding;
    const sourcePoint = sourceBinding?.kind === "coordinate-source-2d"
      ? scene.points[sourceBinding.sourceIndex]
      : null;
    if (sourceBinding?.kind === "coordinate-source-2d" && sourcePoint) {
      const baseParameters = deriveLabelParameters(scene, new Map<string, number>(parameters));
      const sampled = window.GspRuntimeCore.sampleCoordinateTrace(
        sourceBinding.xExpr,
        sourceBinding.yExpr,
        baseParameters,
        sourceBinding.xName,
        sourceBinding.yName,
        sourcePoint,
        line.binding.xMin,
        line.binding.xMax,
        Math.max(2, line.binding.sampleCount || 2),
        line.binding.useMidpoints === true,
        "two-dimensional",
      );
      return sampled.length >= 2 ? [sampled] : null;
    }
    const sampleScene = {
      ...scene,
      lines: scene.lines,
      circles: scene.circles,
  
      points: [],
    };
  
    let baseParameters = new Map<string, number>(parameters);
    let driverValue = Number.NaN;
    const pointOrder = modules.dynamicsDependencies.createPointDependencyOrder(scene);
    let resolvedBatch: Array<Point | null> = [];
    let resolvedCache = new Map();
  
  
    const resolveTracePoint = (points, index: number, visiting= new Set<number>()) => {
      if (resolvedCache.has(index)) {
        return resolvedCache.get(index) ?? null;
      }
      if (visiting.has(index)) return null;
      const point = points[index];
      if (!point) return null;
      const batchPoint = resolvedBatch[index];
      if (batchPoint) return batchPoint;
      visiting.add(index);
  
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
      } else if (point.binding?.kind === "marked-angle-translation") {
        resolved = markedAngleTranslationPoint(
          point.binding,
          baseParameters,
          (pointIndex: number) => resolveTracePoint(points, pointIndex, visiting),
        );
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
            ? lineProjectionParameterFromPoints(source, start, end)
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
        const exprParameters = new Map(baseParameters);
        if (typeof point.binding.name === "string" && Number.isFinite(driverValue)) {
          exprParameters.set(point.binding.name, driverValue);
        }
        const x = exprParameters.get(point.binding.name);
        const y = evaluateExpr(point.binding.expr, 0, exprParameters);
        if (isFiniteNumber(x) && y !== null) {
          resolved = { x, y };
        }
      } else if (point.binding?.kind === "coordinate-source") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const exprParameters = new Map(baseParameters);
        if (typeof point.binding.name === "string" && Number.isFinite(driverValue)) {
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
        const exprParameters = new Map(baseParameters);
        if (Number.isFinite(driverValue)) {
          exprParameters.set(point.binding.xName, driverValue);
          exprParameters.set(point.binding.yName, driverValue);
        }
        const dx = evaluateExpr(point.binding.xExpr, 0, exprParameters);
        const dy = evaluateExpr(point.binding.yExpr, 0, exprParameters);
        if (source && dx !== null && dy !== null) {
          resolved = { x: source.x + dx, y: source.y + dy };
        }
      } else if (point.binding?.kind === "polar-transform") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const derived = { ...point };
        updatePolarTransformPoint(derived, source, baseParameters, scene.yUp === true);
        if (Number.isFinite(derived.x) && Number.isFinite(derived.y)) {
          resolved = { x: derived.x, y: derived.y };
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
      } else if (point.binding?.kind === "constraint-parameter-point-distance-ratio") {
        const origin = resolveTracePoint(points, point.binding.originIndex, visiting);
        const denominator = resolveTracePoint(points, point.binding.denominatorIndex, visiting);
        const numerator = resolveTracePoint(points, point.binding.numeratorIndex, visiting);
        const value = origin && denominator && numerator
          ? window.GspRuntimeCore.pointDistanceRatio(
              origin,
              denominator,
              numerator,
              point.binding.clampToUnit === true,
            )
          : null;
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
        const sourceValue = typeof point.binding.sourceParameterStartIndex === "number"
          && typeof point.binding.sourceParameterEndIndex === "number"
          ? lineProjectionParameterFromPoints(
              sampleScene.points[point.binding.sourceIndex],
              sampleScene.points[point.binding.sourceParameterStartIndex],
              sampleScene.points[point.binding.sourceParameterEndIndex],
              "segment",
            )
          : parameterValueFromPoint(sampleScene, point.binding.sourceIndex);
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
  
    const sampledByTarget = targetPointIndices.map(() => [] as Point[]);
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
      // Trace evaluation replaces derived entries in this per-sample array;
      // only the driver itself is mutated in place. Avoid deep-cloning every
      // point (including large polyline constraints) for every trace sample.
      const points = scene.points.slice();
      points[line.binding.driverIndex] = cloneTracePoint(
        scene.points[line.binding.driverIndex],
      );
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
      resolvedBatch = window.GspRuntimeCore.resolvePointConstraints(
        points,
        pointOrder,
        scene.yUp,
        baseParameters,
      );
      resolvedCache = new Map();
      const resolvedTargets = targetPointIndices.map((pointIndex) =>
        resolveTracePoint(points, pointIndex)
      );
      if (resolvedTargets.every(Boolean)) {
        resolvedTargets.forEach((point, targetIndex) => {
          sampledByTarget[targetIndex].push({ x: point.x, y: point.y });
        });
      }
    }
    return sampledByTarget.every((sampled) => sampled.length >= 2)
      ? sampledByTarget
      : null;
  }
  
  
  function samplePointTraceLine(scene: ViewerSceneData, line: RuntimeLineJson, parameters: Map<string, number>) {
    return samplePointTraceTargets(
      scene,
      line,
      parameters,
      [line.binding.pointIndex],
    )?.[0] ?? null;
  }
  
  function refreshDerivedLine(env: LineBindingRefreshContext, line: RuntimeLineJson) {
    const source = env.scene.lines[line.binding.sourceIndex];
    if (!source) return;
    const sourcePoints = source.points
      .map(env.env.resolvePoint)
      .filter((point): point is Point => point !== null);
    const nextPoints = window.GspRuntimeCore.transformPoints(
      sourcePoints,
      line.binding.transform,
      env.scene,
      env.parameters,
    );
    if (nextPoints) line.points = nextPoints;
  }
  
  
  function refreshColorizedSpectrumLine(context: LineBindingRefreshContext, line: RuntimeLineJson) {
    const binding = line.binding;
    const hostLine = context.scene.lines[binding.lineIndex];
    const traceLine = context.scene.lines[binding.traceLineIndex];
    const baseParameter = polylineParameterFromPoint(context.scene, binding.pointIndex);
    const tracePoints = traceLine?.points
      .map(context.env.resolvePoint)
      .filter((point): point is Point => point !== null) ?? [];
    if (tracePoints.length < 2 || !isFiniteNumber(baseParameter)) {
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
      tracePoints,
      baseParameter + (binding.stepIndex || 0) / depth,
    );
    if (!sample) return;
  
    const hostPoints = hostLine?.points
      .map(context.env.resolvePoint)
      .filter((point): point is Point => point !== null);
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
      const sampledAxis = sampledReflectionAxis(
        context.scene,
        binding,
        sample,
        context.env.resolvePoint,
      );
      const axisLine = sampledAxis ? null : context.scene.lines[binding.reflectionAxisLineIndex];
      const axisStartHandle = axisLine?.points?.[0];
      const axisEndHandle = axisLine?.points?.[axisLine.points.length - 1];
      const axisStart = sampledAxis?.[0]
        ?? (axisStartHandle ? context.env.resolvePoint(axisStartHandle) : null);
      const axisEnd = sampledAxis?.[1]
        ?? (axisEndHandle ? context.env.resolvePoint(axisEndHandle) : null);
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
  
  
  function sampledReflectionAxis(
    scene: ViewerSceneData,
    binding: RuntimeLineBindingJson,
    sample: Point,
    resolveHandle: (handle: PointHandle) => Point | null,
  ) {
    if (
      !isFiniteNumber(binding.reflectionFocusIndex)
      || !isFiniteNumber(binding.reflectionDirectrixLineIndex)
    ) {
      return null;
    }
    const focus = scene.points[binding.reflectionFocusIndex];
    const directrixLine = scene.lines[binding.reflectionDirectrixLineIndex];
    const directrixStartHandle = directrixLine?.points?.[0];
    const directrixEndHandle = directrixLine?.points?.[directrixLine.points.length - 1];
    const directrixStart = directrixStartHandle ? resolveHandle(directrixStartHandle) : null;
    const directrixEnd = directrixEndHandle ? resolveHandle(directrixEndHandle) : null;
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
    return window.GspRuntimeCore.projectToLineLike(point, lineStart, lineEnd, "line")?.projected ?? null;
  }
  
  
  function refreshDerivedPolygon(env: { scene: ViewerSceneData, parameters: Map<string, number>, resolveHandle: (handle: PointHandle) => Point | null }, polygon: RuntimePolygonJson) {
    const source = env.scene.polygons[polygon.binding.sourceIndex];
    if (!source) return;
    const sourcePoints = source.points
      .map(( handle) => env.resolveHandle(handle))
      .filter((point): point is Point => point !== null);
    const nextPoints = window.GspRuntimeCore.transformPoints(
      sourcePoints,
      polygon.binding.transform,
      env.scene,
      env.parameters,
    );
    if (nextPoints) polygon.points = nextPoints;
  }
  
  
  function refreshDerivedCircle(env: { scene: ViewerSceneData, parameters: Map<string, number>, resolveHandle: (handle: PointHandle) => Point | null }, circle: RuntimeCircleJson) {
    const source = env.scene.circles[circle.binding.sourceIndex];
    if (!source) return;
    const sourceCenter = env.resolveHandle(source.center);
    const sourceRadius = env.resolveHandle(source.radiusPoint);
    if (!sourceCenter || !sourceRadius) return;
    const transformed = window.GspRuntimeCore.transformPoints(
      [sourceCenter, sourceRadius],
      circle.binding.transform,
      env.scene,
      env.parameters,
    );
    if (!transformed) return;
    [circle.center, circle.radiusPoint] = transformed;
  }
  
  

    return {
      resolveHostLinePoints,
      sampleCustomTransformTraceLine,
      cloneTracePoint,
      samplePointTraceTargets,
      samplePointTraceLine,
      refreshDerivedLine,
      refreshColorizedSpectrumLine,
      refreshDerivedPolygon,
      refreshDerivedCircle,
    };
  }

  modules.dynamicsGeometry = { createDynamicsGeometry };
})();
