(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  );
  const geometry = modules.geometry;
  const {
    normalizeAngleDelta,
    lerpPoint,
    scaleAround: scalePointAround,
    reflectAcrossLine: reflectPointAcrossLine,
    clipParametricLineToBounds,
    angleBisectorDirection,
  } = geometry;
  
  const extraPointConstraintResolvers: Partial<Record<RuntimePointConstraintJson["kind"], (
    env: ViewerSceneResolverEnv | null,
    constraint: RuntimePointConstraintJson,
    resolveFn: (index: number) => Point | null,
    reference?: RuntimeScenePointJson | Point | null,
  ) => Point | null>> = {};
  
  const extraLineBindingResolvers: Partial<Record<RuntimeLineBindingJson["kind"], (
    env: ViewerEnv,
    line: RuntimeLineJson,
  ) => Point[] | null>> = {};

  
  function registerPointConstraintResolver<K extends RuntimePointConstraintJson["kind"]>(
    kind: K,
    resolver: (
      env: ViewerSceneResolverEnv | null,
      constraint: Extract<RuntimePointConstraintJson, { kind: K }>,
      resolveFn: (index: number) => Point | null,
      reference?: RuntimeScenePointJson | Point | null,
    ) => Point | null,
  ) {
    extraPointConstraintResolvers[kind] = resolver as (
      env: ViewerSceneResolverEnv | null,
      constraint: RuntimePointConstraintJson,
      resolveFn: (index: number) => Point | null,
      reference?: RuntimeScenePointJson | Point | null,
    ) => Point | null;
  }

  
  function registerLineBindingResolver<K extends RuntimeLineBindingJson["kind"]>(
    kind: K,
    resolver: (env: ViewerEnv, line: RuntimeLineJson & { binding: Extract<RuntimeLineBindingJson, { kind: K }> }) => Point[] | null,
  ) {
    extraLineBindingResolvers[kind] = resolver as (env: ViewerEnv, line: RuntimeLineJson) => Point[] | null;
  }

  
  function hasPointIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { pointIndex: number }> {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  
  function hasLineIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { lineIndex: number }> {
    return !!handle && typeof handle === "object" && "lineIndex" in handle && typeof handle.lineIndex === "number";
  }

  
  function projectToSegment(point: Point, start: Point, end: Point) {
    return projectToLineLike(point, start, end, "segment");
  }

  
  function projectToLineLike(point: Point, start: Point, end: Point, kind: "segment" | "line" | "ray") {
    return window.GspRuntimeCore.projectToLineLike(point, start, end, kind);
  }

  
  function threePointArcGeometry(start: Point, mid: Point, end: Point) {
    return window.GspRuntimeCore.threePointArcGeometry(start, mid, end);
  }

  
  function circleArcControlPoints(center: Point, start: Point, end: Point, yUp: boolean) {
    const controls = window.GspRuntimeCore.circleArcControlPoints(center, start, end, yUp);
    return controls ? { start: controls[0], mid: controls[1], end: controls[2] } : null;
  }

  
  function pointOnThreePointArc(start: Point, mid: Point, end: Point, t: number) {
    return window.GspRuntimeCore.pointOnThreePointArc(start, mid, end, t, false);
  }

  
  function pointOnThreePointArcComplement(start: Point, mid: Point, end: Point, t: number) {
    return window.GspRuntimeCore.pointOnThreePointArc(start, mid, end, t, true);
  }

  
  function pointOnCircleArc(center: Point, start: Point, end: Point, t: number, yUp: boolean) {
    return window.GspRuntimeCore.pointOnCircleArc(center, start, end, t, yUp);
  }

  
  function projectToThreePointArc(point: Point, start: Point, mid: Point, end: Point) {
    return window.GspRuntimeCore.projectToThreePointArc(point, start, mid, end);
  }

  
  function projectToCircleArc(point: Point, center: Point, start: Point, end: Point, yUp: boolean) {
    return window.GspRuntimeCore.projectToCircleArc(point, center, start, end, yUp);
  }

  
  function reflectionAxisPoints(env: ViewerSceneResolverEnv | null, constraint: { lineStartIndex?: number | null, lineEndIndex?: number | null, lineIndex?: number | null }, resolveFn: (index: number) => Point | null) {
    const scene = typeof env?.currentScene === "function"
      ? env.currentScene()
      : env?.sourceScene || null;
    
    const resolveFromBinding = (binding) => {
      if (!binding) return null;
      if (typeof binding.lineStartIndex === "number" && typeof binding.lineEndIndex === "number") {
        const lineStart = resolveFn(binding.lineStartIndex);
        const lineEnd = resolveFn(binding.lineEndIndex);
        return lineStart && lineEnd ? [lineStart, lineEnd] : null;
      }
      if (typeof binding.lineIndex === "number") {
        const line = scene?.lines?.[binding.lineIndex];
        return line ? resolveFromLine(line) : null;
      }
      return null;
    };
    
    const resolveFromLine = (line) => {
      if (!line) return null;
      if (line.points?.length >= 2 && !line.binding) {
        return [line.points[0], line.points[line.points.length - 1]];
      }
      if (line.binding?.kind === "segment") {
        const start = resolveFn(line.binding.startIndex);
        const end = resolveFn(line.binding.endIndex);
        return start && end ? [start, end] : null;
      }
      if (line.binding?.kind === "perpendicular-line" || line.binding?.kind === "parallel-line") {
        const through = resolveFn(line.binding.throughIndex);
        const hostLine = resolveFromBinding(line.binding);
        if (!through || !hostLine) return null;
        const [lineStart, lineEnd] = hostLine;
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        if (len <= 1e-9) return null;
        return line.binding.kind === "perpendicular-line"
          ? [through, { x: through.x - dy / len, y: through.y + dx / len }]
          : [through, { x: through.x + dx / len, y: through.y + dy / len }];
      }
      if (line.binding?.kind === "angle-bisector-ray") {
        const start = resolveFn(line.binding.startIndex);
        const vertex = resolveFn(line.binding.vertexIndex);
        const end = resolveFn(line.binding.endIndex);
        if (!start || !vertex || !end) return null;
        const direction = angleBisectorDirection(start, vertex, end);
        return direction ? [vertex, { x: vertex.x + direction.x, y: vertex.y + direction.y }] : null;
      }
      if (line.points?.length >= 2) {
        return [line.points[0], line.points[line.points.length - 1]];
      }
      return null;
    };
    if (typeof constraint.lineIndex === "number") {
      const line = scene?.lines?.[constraint.lineIndex];
      const resolved = resolveFromLine(line);
      if (resolved) return resolved;
    }
    const lineStart = typeof constraint.lineStartIndex === "number"
      ? resolveFn(constraint.lineStartIndex)
      : null;
    const lineEnd = typeof constraint.lineEndIndex === "number"
      ? resolveFn(constraint.lineEndIndex)
      : null;
    return [lineStart, lineEnd];
  }

  
  function circleFromConstraint(env: ViewerSceneResolverEnv | null, constraint: CircularConstraintJson | null, resolveFn: (index: number) => Point | null) {
    if (!constraint) return null;
    if (constraint.kind === "circle") {
      const center = resolveFn(constraint.centerIndex);
      const radiusPoint = resolveFn(constraint.radiusIndex);
      if (!center || !radiusPoint) return null;
      return {
        kind: "circle",
        center,
        radius: Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y),
      };
    }
    if (constraint.kind === "segment-radius-circle") {
      const center = resolveFn(constraint.centerIndex);
      const lineStart = resolveFn(constraint.lineStartIndex);
      const lineEnd = resolveFn(constraint.lineEndIndex);
      if (!center || !lineStart || !lineEnd) return null;
      return {
        kind: "circle",
        center,
        radius: Math.hypot(lineEnd.x - lineStart.x, lineEnd.y - lineStart.y),
      };
    }
    if (constraint.kind === "parameter-radius-circle") {
      const center = resolveFn(constraint.centerIndex);
      const value = env && "currentDynamics" in env
        ? env.currentDynamics().parameters
          .find(( parameter) => parameter.name === constraint.parameterName)
          ?.value ?? constraint.parameterValue
        : constraint.parameterValue;
      if (!center || !Number.isFinite(value)) return null;
      return {
        kind: "circle",
        center,
        radius: Math.abs(value) * constraint.rawPerUnit,
      };
    }
    if (constraint.kind === "expression-radius-circle") {
      const center = resolveFn(constraint.centerIndex);
      const parameters = env && "currentDynamics" in env
        ? new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]))
        : new Map();
      const value = modules.dynamics?.evaluateExpr?.(constraint.expr, 0, parameters)
        ?? constraint.initialValue;
      if (!center || !Number.isFinite(value)) return null;
      return {
        kind: "circle",
        center,
        radius: Math.abs(value),
      };
    }
    if (constraint.kind === "derived" && constraint.transform.kind === "translate-delta") {
      const source = circleFromConstraint(env, constraint.source, resolveFn);
      if (!source || source.kind !== "circle") return null;
      return {
        kind: "circle",
        center: {
          x: source.center.x + constraint.transform.dx,
          y: source.center.y + constraint.transform.dy,
        },
        radius: source.radius,
      };
    }
    if (constraint.kind === "derived" && constraint.transform.kind === "reflect") {
      const source = circleFromConstraint(env, constraint.source, resolveFn);
      const [lineStart, lineEnd] = reflectionAxisPoints(env, constraint.transform, resolveFn);
      if (!source || source.kind !== "circle" || !lineStart || !lineEnd) return null;
      const reflectedCenter = reflectPointAcrossLine(source.center, lineStart, lineEnd);
      if (!reflectedCenter) return null;
      return {
        kind: "circle",
        center: reflectedCenter,
        radius: source.radius,
      };
    }
    if (constraint.kind === "derived" && constraint.transform.kind === "scale") {
      const source = circleFromConstraint(env, constraint.source, resolveFn);
      const center = resolveFn(constraint.transform.centerIndex);
      if (!source || source.kind !== "circle" || !center) return null;
      return {
        kind: "circle",
        center: scalePointAround(source.center, center, constraint.transform.factor),
        radius: source.radius * Math.abs(constraint.transform.factor),
      };
    }
    if (constraint.kind === "circle-arc") {
      const center = resolveFn(constraint.centerIndex);
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      if (!center || !start || !end) return null;
      const controls = circleArcControlPoints(center, start, end, !!env?.sourceScene?.yUp);
      if (!controls) return null;
      const geometry = threePointArcGeometry(controls.start, controls.mid, controls.end);
      return geometry ? { kind: "three-point-arc", ...geometry } : null;
    }
    if (constraint.kind === "three-point-arc") {
      const start = resolveFn(constraint.startIndex);
      const mid = resolveFn(constraint.midIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !mid || !end) return null;
      const geometry = threePointArcGeometry(start, mid, end);
      return geometry ? { kind: "three-point-arc", ...geometry } : null;
    }
    return null;
  }

  
  function pointLiesOnCircularConstraint(point: Point, constraint: { kind: string; center?: Point; radius?: number; ccwMid?: number; ccwSpan?: number; startAngle?: number; endAngle?: number } | null) {
    if (!constraint) return false;
    if (constraint.kind === "circle") return true;
    if (constraint.kind === "three-point-arc") {
      const radial = Math.hypot(point.x - constraint.center.x, point.y - constraint.center.y);
      if (Math.abs(radial - constraint.radius) > 1e-6) return false;
      const angle = Math.atan2(point.y - constraint.center.y, point.x - constraint.center.x);
      if (constraint.ccwMid <= constraint.ccwSpan + 1e-9) {
        return normalizeAngleDelta(constraint.startAngle, angle) <= constraint.ccwSpan + 1e-9;
      }
      return normalizeAngleDelta(angle, constraint.startAngle)
        <= normalizeAngleDelta(constraint.endAngle, constraint.startAngle) + 1e-9;
    }
    return false;
  }

  
  function getViewBounds(env: ViewerEnv) {
    const spanX = env.baseSpanX / env.view.zoom;
    const spanY = env.baseSpanY / env.view.zoom;
    return {
      minX: env.view.centerX - spanX / 2,
      maxX: env.view.centerX + spanX / 2,
      minY: env.view.centerY - spanY / 2,
      maxY: env.view.centerY + spanY / 2,
      spanX,
      spanY,
    };
  }

  
  function resolveConstrainedPoint(env: ViewerSceneResolverEnv | null, constraint: RuntimePointConstraintJson | null, resolveFn: (index: number) => Point | null, reference: RuntimeScenePointJson | Point | null | undefined) {
    if (!constraint) return null;
    if (constraint.kind === "offset") {
      const origin = resolveFn(constraint.originIndex);
      return origin ? { x: origin.x + constraint.dx, y: origin.y + constraint.dy } : null;
    }
    if (constraint.kind === "segment") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      return start && end ? lerpPoint(start, end, constraint.t) : null;
    }
    if (constraint.kind === "line" || constraint.kind === "ray") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      return start && end ? lerpPoint(start, end, constraint.t) : null;
    }
    if (constraint.kind === "line-constraint" || constraint.kind === "ray-constraint") {
      const scene = typeof env?.currentScene === "function"
        ? env.currentScene()
        : env?.sourceScene || null;
      const line = scene && window.GspViewerModules.dynamics
        ? window.GspViewerModules.dynamics.resolveLineConstraintParameterPoints(
            resolveFn,
            constraint.line,
          )
        : null;
      return line ? lerpPoint(line[0], line[1], constraint.t) : null;
    }
    if (constraint.kind === "polygon-boundary") {
      const count = constraint.vertexIndices.length;
      if (count < 2) return null;
      const start = resolveFn(constraint.vertexIndices[((constraint.edgeIndex % count) + count) % count]);
      const end = resolveFn(constraint.vertexIndices[(constraint.edgeIndex + 1 + count) % count]);
      return start && end ? lerpPoint(start, end, constraint.t) : null;
    }
    if (constraint.kind === "translated-polygon-boundary") {
      const count = constraint.vertexIndices.length;
      if (count < 2) return null;
      const start = resolveFn(constraint.vertexIndices[((constraint.edgeIndex % count) + count) % count]);
      const end = resolveFn(constraint.vertexIndices[(constraint.edgeIndex + 1 + count) % count]);
      const vectorStart = resolveFn(constraint.vectorStartIndex);
      const vectorEnd = resolveFn(constraint.vectorEndIndex);
      if (!start || !end || !vectorStart || !vectorEnd) return null;
      const base = lerpPoint(start, end, constraint.t);
      return {
        x: base.x + (vectorEnd.x - vectorStart.x),
        y: base.y + (vectorEnd.y - vectorStart.y),
      };
    }
    if (constraint.kind === "circular-constraint") {
      const circle = circleFromConstraint(env, constraint.circle, resolveFn);
      if (!circle) return null;
      return {
        x: circle.center.x + circle.radius * constraint.unitX,
        y: circle.center.y - circle.radius * constraint.unitY,
      };
    }
    const extra = extraPointConstraintResolvers[constraint.kind];
    return extra ? extra(env, constraint, resolveFn, reference) : null;
  }

  
  function resolveScenePoint(env: ViewerEnv, index: number) {
    const point = env.currentScene().points[index];
    if (!point) return null;
    if (!point.constraint) return point;
    const resolved = resolveConstrainedPoint(env, point.constraint, (i) => resolveScenePoint(env, i), point);
    if (resolved) return resolved;
    return null;
  }

  
  function resolvePoint(env: ViewerEnv, handle: PointHandle) {
    if (hasPointIndexHandle(handle)) {
      const point = resolveScenePoint(env, handle.pointIndex);
      if (!point) return null;
      return {
        x: point.x + (handle.dx || 0),
        y: point.y + (handle.dy || 0),
      };
    }
    if (hasLineIndexHandle(handle)) {
      const points = resolveLinePoints(env, handle.lineIndex);
      if (!points || points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      const start = points[segmentIndex];
      const end = points[segmentIndex + 1];
      return {
        x: lerpPoint(start, end, t).x + (handle.dx || 0),
        y: lerpPoint(start, end, t).y + (handle.dy || 0),
      };
    }
    return  (handle);
  }

  
  function resolveAnchorBase(env: ViewerEnv, handle: PointHandle) {
    if (hasPointIndexHandle(handle)) {
      return resolveScenePoint(env, handle.pointIndex);
    }
    if (hasLineIndexHandle(handle)) {
      const points = resolveLinePoints(env, handle.lineIndex);
      if (!points || points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      return lerpPoint(points[segmentIndex], points[segmentIndex + 1], t);
    }
    return  (handle);
  }

  
  function resolveHostLinePoints(env: ViewerEnv, binding: HostLineBinding) {
    const hostBinding = binding;
    if (
      typeof hostBinding.lineStartIndex === "number"
      && typeof hostBinding.lineEndIndex === "number"
    ) {
      const lineStart = resolveScenePoint(env, hostBinding.lineStartIndex);
      const lineEnd = resolveScenePoint(env, hostBinding.lineEndIndex);
      if (!lineStart || !lineEnd) return null;
      return [lineStart, lineEnd];
    }
    if (typeof hostBinding.lineIndex === "number") {
      return resolveLinePoints(env, hostBinding.lineIndex);
    }
    return null;
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

  
  function resolveLinePoints(env: ViewerEnv, lineOrIndex: SceneLineJson | number | null | undefined) {
    const line = typeof lineOrIndex === "number" ? env.currentScene().lines[lineOrIndex] : lineOrIndex;
    if (!line) return null;
    if (line.binding?.kind === "segment") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      if (!start || !end) return null;
      return [start, end];
    }
    if (line.binding?.kind === "angle-marker") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const vertex = resolveScenePoint(env, line.binding.vertexIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      if (!start || !vertex || !end) return null;
      return resolveAngleMarkerPoints(start, vertex, end, line.binding.markerClass);
    }
    if (line.binding?.kind === "angle-bisector-ray") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const vertex = resolveScenePoint(env, line.binding.vertexIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      if (!direction) return null;
      return clipParametricLineToBounds(
        vertex,
        { x: vertex.x + direction.x, y: vertex.y + direction.y },
        getViewBounds(env),
        true,
      );
    }
    if (line.binding?.kind === "perpendicular-line") {
      const through = resolveScenePoint(env, line.binding.throughIndex);
      if (!through) return null;
      const hostLine = resolveHostLinePoints(env, line.binding);
      if (!hostLine) return null;
      const [lineStart, lineEnd] = hostLine;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x - dy / len, y: through.y + dx / len },
        getViewBounds(env),
        false,
      );
    }
    if (line.binding?.kind === "parallel-line") {
      const through = resolveScenePoint(env, line.binding.throughIndex);
      if (!through) return null;
      const hostLine = resolveHostLinePoints(env, line.binding);
      if (!hostLine) return null;
      const [lineStart, lineEnd] = hostLine;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x + dx / len, y: through.y + dy / len },
        getViewBounds(env),
        false,
      );
    }
    if (line.binding?.kind === "line") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      if (!start || !end) return null;
      return clipParametricLineToBounds(start, end, getViewBounds(env), false);
    }
    if (line.binding?.kind === "ray") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      if (!start || !end) return null;
      return clipParametricLineToBounds(start, end, getViewBounds(env), true);
    }
    if (line.binding) {
      const extra = extraLineBindingResolvers[line.binding.kind];
      if (extra) {
        const resolved = extra(env, line);
        if (resolved) {
          return resolved;
        }
      }
    }
    const points = line.points.map(( handle) => resolvePoint(env, handle));
    return points.every(Boolean) ? points : null;
  }

  
  function toScreen(env: ViewerEnv, point: Point) {
    const usableWidth = Math.max(1, env.sourceScene.width - env.margin * 2);
    const usableHeight = Math.max(1, env.sourceScene.height - env.margin * 2);
    const bounds = getViewBounds(env);
    const scale = Math.min(usableWidth / bounds.spanX, usableHeight / bounds.spanY);
    return {
      x: env.margin + (point.x - bounds.minX) * scale,
      y: env.sourceScene.yUp
        ? env.sourceScene.height - env.margin - (point.y - bounds.minY) * scale
        : env.margin + (point.y - bounds.minY) * scale,
      scale,
    };
  }

  
  function toWorld(env: ViewerEnv, screenX: number, screenY: number) {
    const usableWidth = Math.max(1, env.sourceScene.width - env.margin * 2);
    const usableHeight = Math.max(1, env.sourceScene.height - env.margin * 2);
    const bounds = getViewBounds(env);
    const scale = Math.min(usableWidth / bounds.spanX, usableHeight / bounds.spanY);
    return {
      x: bounds.minX + (screenX - env.margin) / scale,
      y: env.sourceScene.yUp
        ? bounds.minY + (env.sourceScene.height - env.margin - screenY) / scale
        : bounds.minY + (screenY - env.margin) / scale,
      scale,
    };
  }

  
  function getCanvasCoords(env: ViewerEnv, event: MouseEvent | PointerEvent | WheelEvent) {
    const rect = env.canvas.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left) * (env.sourceScene.width / rect.width),
      y: (event.clientY - rect.top) * (env.sourceScene.height / rect.height),
    };
  }

  
  function appendGridElement(env: ViewerEnv, parent: Element, attrs: Record<string, string | number | boolean | null | undefined>) {
    const tag = String(attrs.tag);
    const nextAttrs = { ...attrs };
    delete nextAttrs.tag;
    const element = env.createSvgElement(tag, nextAttrs);
    parent.append(element);
    return element;
  }

  
  function appendGridLine(env: ViewerEnv, parent: Element, x1: number, y1: number, x2: number, y2: number, color: string) {
    appendGridElement(env, parent, {
      tag: "line",
      x1,
      y1,
      x2,
      y2,
      stroke: color,
      "stroke-width": 1,
      "shape-rendering": "crispEdges",
    });
  }

  
  function appendGridText(env: ViewerEnv, parent: Element, x: number, y: number, text: string, anchor: "start" | "middle" | "end") {
    const label = appendGridElement(env, parent, {
      tag: "text",
      x,
      y,
      fill: "rgb(20,20,20)",
      "font-size": 12,
      "font-family": "\"Noto Sans\", \"Segoe UI\", sans-serif",
      "text-anchor": anchor,
      "dominant-baseline": "middle",
    });
    label.textContent = text;
  }

  
  function chooseGridStep(span: number, targetLines: number) {
    const rough = Math.max(1e-6, span / Math.max(1, targetLines));
    const magnitude = 10 ** Math.floor(Math.log10(rough));
    const normalized = rough / magnitude;
    if (normalized <= 1) return magnitude;
    if (normalized <= 2) return magnitude * 2;
    if (normalized <= 5) return magnitude * 5;
    return magnitude * 10;
  }

  
  function drawGrid(env: ViewerEnv) {
    env.clearSvgChildren(env.gridLayer);
    if (!env.currentScene().graphMode) return;
    const gridLayer = env.gridLayer;
    const snapStroke = ( value) => Math.round(value) + 0.5;
    const bounds = getViewBounds(env);
    const spanX = bounds.maxX - bounds.minX;
    const spanY = bounds.maxY - bounds.minY;
    const xMinorStep = env.savedViewportMode ? 1 : chooseGridStep(spanX, 14);
    const xMajorStep = env.savedViewportMode ? 2 : chooseGridStep(spanX, 7);
    const yMinorStep = env.savedViewportMode ? 1 : chooseGridStep(spanY, 14);
    const yMajorStep = env.savedViewportMode ? 2 : chooseGridStep(spanY, 7);
    const minXIndex = Math.floor(bounds.minX / xMinorStep);
    const maxXIndex = Math.ceil(bounds.maxX / xMinorStep);
    const minYIndex = Math.floor(bounds.minY / yMinorStep);
    const maxYIndex = Math.ceil(bounds.maxY / yMinorStep);

    const xAxisY = bounds.minY <= 0 && 0 <= bounds.maxY
      ? toScreen(env, { x: bounds.minX, y: 0 }).y
      : env.sourceScene.height - 18;
    const yAxisX = bounds.minX <= 0 && 0 <= bounds.maxX
      ? toScreen(env, { x: 0, y: bounds.minY }).x
      : env.sourceScene.width / 2;

    for (let xIndex = minXIndex; xIndex <= maxXIndex; xIndex += 1) {
      const x = xIndex * xMinorStep;
      const screen = toScreen(env, { x, y: bounds.minY });
      const major = Math.abs((x / xMajorStep) - Math.round(x / xMajorStep)) < 1e-6;
      const stroke = Math.abs(x) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      appendGridLine(
        env,
        gridLayer,
        snapStroke(screen.x),
        0,
        snapStroke(screen.x),
        env.sourceScene.height,
        stroke,
      );
      if (bounds.minY <= 0 && 0 <= bounds.maxY) {
        appendGridLine(
          env,
          gridLayer,
          snapStroke(screen.x),
          snapStroke(xAxisY - (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.x),
          snapStroke(xAxisY + (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2)),
          "rgb(40,40,40)",
        );
      }
      if (major && Math.abs(x) >= 1e-6) {
        const label = env.formatAxisNumber(x);
        appendGridText(
          env,
          gridLayer,
          Math.round(screen.x),
          Math.round(Math.min(env.sourceScene.height - 4, xAxisY + 16)),
          label,
          "middle",
        );
      }
    }

    for (let yIndex = minYIndex; yIndex <= maxYIndex; yIndex += 1) {
      const y = yIndex * yMinorStep;
      const major = Math.abs((y / yMajorStep) - Math.round(y / yMajorStep)) < 1e-6;
      const screen = toScreen(env, { x: bounds.minX, y });
      const stroke = Math.abs(y) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      appendGridLine(
        env,
        gridLayer,
        0,
        snapStroke(screen.y),
        env.sourceScene.width,
        snapStroke(screen.y),
        stroke,
      );
      if (bounds.minX <= 0 && 0 <= bounds.maxX) {
        appendGridLine(
          env,
          gridLayer,
          snapStroke(yAxisX - (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.y),
          snapStroke(yAxisX + (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.y),
          "rgb(40,40,40)",
        );
      }
      if (major && Math.abs(y) >= 1e-6) {
        const label = env.formatAxisNumber(y);
        appendGridText(
          env,
          gridLayer,
          Math.round(yAxisX - env.measureText(label, 12) - 8),
          Math.round(screen.y - 6),
          label,
          "start",
        );
      }
    }

    const originHandle = env.currentScene().origin;
    if (originHandle) {
      const resolvedOrigin = resolvePoint(env, originHandle);
      if (!resolvedOrigin) return;
      const origin = toScreen(env, resolvedOrigin);
      appendGridElement(env, gridLayer, {
        tag: "circle",
        cx: origin.x,
        cy: origin.y,
        r: 3,
        fill: "rgba(255, 60, 40, 1)",
      });
    }
  }

  
  modules.scene = {
    registerPointConstraintResolver,
    registerLineBindingResolver,
    _circleFromConstraint: circleFromConstraint,
    _pointLiesOnCircularConstraint: pointLiesOnCircularConstraint,
    _threePointArcGeometry: threePointArcGeometry,
    _circleArcControlPoints: circleArcControlPoints,
    _pointOnThreePointArcComplement: pointOnThreePointArcComplement,
    getViewBounds,
    resolveConstrainedPoint,
    resolveScenePoint,
    resolvePoint,
    resolveAnchorBase,
    resolveLinePoints,
    toScreen,
    toWorld,
    getCanvasCoords,
    chooseGridStep,
    lerpPoint,
    projectToSegment,
    projectToLineLike,
    pointOnCircleArc,
    projectToCircleArc,
    pointOnThreePointArc,
    projectToThreePointArc,
    resolveAngleMarkerPoints,
    drawGrid,
  } as unknown as ViewerSceneModule;
})();
