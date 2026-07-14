(function() {
  const modules = (window.GspViewerModules || (window.GspViewerModules = {})) as Partial<ViewerModules> & {
    scene: ViewerSceneModule;
  };
  const scene = modules.scene;
  type LineKind = "segment" | "line" | "ray";
  type ResolvedLineConstraint = { start: Point; end: Point; kind: LineKind };

  
  function lineLineIntersection(leftStart: Point, leftEnd: Point, leftKind: LineKind, rightStart: Point, rightEnd: Point, rightKind: LineKind) {
    return window.GspRuntimeCore.lineLineIntersection(
      leftStart,
      leftEnd,
      leftKind,
      rightStart,
      rightEnd,
      rightKind,
    );
  }

  
  function linePolylineIntersection(
    lineStart: Point,
    lineEnd: Point,
    lineKind: LineKind,
    points: Point[] | null,
    sampleHint: number | null | undefined = null,
    variant = 0,
  ) {
    if (!Array.isArray(points) || points.length < 2) return null;
    return window.GspRuntimeCore.linePolylineIntersection(
      lineStart,
      lineEnd,
      lineKind,
      points,
      typeof sampleHint === "number" && Number.isFinite(sampleHint) ? sampleHint : null,
      variant,
    );
  }

  function circularPolylineIntersection(
    circle: { kind: string; center?: Point; radius?: number; ccwMid?: number; ccwSpan?: number; startAngle?: number; endAngle?: number },
    points: Point[],
    sampleHint: number | null | undefined,
    variant: number,
  ): Point | null {
    if (!circle.center || !Number.isFinite(circle.radius) || points.length < 2) return null;
    const radius = circle.radius as number;
    const liesOn = scene._pointLiesOnCircularConstraint;
    const candidates: Array<{ segmentIndex: number; point: Point }> = [];
    for (let segmentIndex = 0; segmentIndex + 1 < points.length; segmentIndex += 1) {
      for (const point of window.GspRuntimeCore.lineCircleIntersections(
        points[segmentIndex],
        points[segmentIndex + 1],
        "segment",
        circle.center,
        radius,
      )) {
        if (typeof liesOn === "function" && !liesOn(point, circle)) continue;
        if (candidates.some((candidate) => Math.hypot(candidate.point.x - point.x, candidate.point.y - point.y) <= 1e-7)) continue;
        candidates.push({ segmentIndex, point });
      }
    }
    if (typeof sampleHint === "number" && Number.isFinite(sampleHint)) {
      candidates.sort((left, right) =>
        Math.abs(left.segmentIndex - sampleHint) - Math.abs(right.segmentIndex - sampleHint));
    }
    return candidates[variant]?.point ?? null;
  }

  
  function sampleFunctionIntersectionPoints(env: ViewerSceneResolverEnv | null, constraint: Extract<RuntimePointConstraintJson, { kind: "line-function-intersection" }>): Point[] | null {
    const currentScene = typeof env?.currentScene === "function" ? env.currentScene() : env?.sourceScene;
    const interactiveEnv: ViewerEnv | null = env && "currentDynamics" in env ? env : null;
    const parameters = typeof modules.dynamics?.parameterMapForScene === "function"
      && interactiveEnv
      && currentScene
      ? modules.dynamics.parameterMapForScene(interactiveEnv, currentScene as ViewerSceneData)
      : interactiveEnv
        ? new Map(interactiveEnv.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]))
        : new Map();
    const sampleCount = Math.max(2, constraint.sampleCount || 0);
    const points = window.GspRuntimeCore.sampleFunction(
      constraint.expr,
      parameters,
      constraint.xMin,
      constraint.xMax,
      sampleCount,
      constraint.plotMode,
    ).flat();
    return points.length >= 2 ? points : null;
  }

  
  function choosePointCandidate(candidates: Point[] | null, reference: RuntimeScenePointJson | Point | null | undefined, variant: number) {
    if (!Array.isArray(candidates) || candidates.length === 0) return null;
    return window.GspRuntimeCore.choosePointCandidate(
      candidates,
      reference && Number.isFinite(reference.x) && Number.isFinite(reference.y) ? reference : null,
      variant,
    );
  }

  function resolveLineConstraint(_env: ViewerSceneResolverEnv | null, constraint: LineConstraintJson, resolveFn: (index: number) => Point | null): ResolvedLineConstraint | null {
    if (!constraint) return null;
    if (constraint.kind === "segment" || constraint.kind === "line" || constraint.kind === "ray") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      return start && end ? { start, end, kind: constraint.kind } : null;
    }
    if (constraint.kind === "perpendicular-line" || constraint.kind === "parallel-line") {
      const through = resolveFn(constraint.throughIndex);
      const lineStart = resolveFn(constraint.lineStartIndex);
      const lineEnd = resolveFn(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return constraint.kind === "perpendicular-line"
        ? { start: through, end: { x: through.x - dy / len, y: through.y + dx / len }, kind: "line" }
        : { start: through, end: { x: through.x + dx / len, y: through.y + dy / len }, kind: "line" };
    }
    if (constraint.kind === "perpendicular-to" || constraint.kind === "parallel-to") {
      const through = resolveFn(constraint.throughIndex);
      const base = resolveLineConstraint(_env, constraint.line, resolveFn);
      if (!through || !base) return null;
      const dx = base.end.x - base.start.x;
      const dy = base.end.y - base.start.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return constraint.kind === "perpendicular-to"
        ? { start: through, end: { x: through.x - dy / len, y: through.y + dx / len }, kind: "line" }
        : { start: through, end: { x: through.x + dx / len, y: through.y + dy / len }, kind: "line" };
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolveFn(constraint.startIndex);
      const vertex = resolveFn(constraint.vertexIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = modules.geometry?.angleBisectorDirection(start, vertex, end) ?? null;
      if (!direction) return null;
      return { start: vertex, end: { x: vertex.x + direction.x, y: vertex.y + direction.y }, kind: "ray" };
    }
    if (constraint.kind === "translated") {
      const base: ResolvedLineConstraint | null = resolveLineConstraint(_env, constraint.line, resolveFn);
      const vectorStart = resolveFn(constraint.vectorStartIndex);
      const vectorEnd = resolveFn(constraint.vectorEndIndex);
      if (!base || !vectorStart || !vectorEnd) return null;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      return {
        start: { x: base.start.x + dx, y: base.start.y + dy },
        end: { x: base.end.x + dx, y: base.end.y + dy },
        kind: base.kind,
      };
    }
    if (constraint.kind === "translated-delta") {
      const base: ResolvedLineConstraint | null = resolveLineConstraint(_env, constraint.line, resolveFn);
      if (!base) return null;
      return {
        start: { x: base.start.x + constraint.dx, y: base.start.y + constraint.dy },
        end: { x: base.end.x + constraint.dx, y: base.end.y + constraint.dy },
        kind: base.kind,
      };
    }
    return null;
  }

  
  function lineCircleIntersection(lineStart: Point, lineEnd: Point, lineKind: LineKind, center: Point, radiusPoint: Point, variant: number, _reference: RuntimeScenePointJson | Point | null | undefined) {
    const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
    return window.GspRuntimeCore.lineCircleIntersectionCandidate(
      lineStart,
      lineEnd,
      lineKind,
      center,
      radius,
      variant,
    );
  }

  
  function circleCircleIntersections(leftCenter: Point, leftRadius: number, rightCenter: Point, rightRadius: number) {
    return window.GspRuntimeCore.circleCircleIntersections(
      leftCenter,
      leftRadius,
      rightCenter,
      rightRadius,
    );
  }

  
  function circleCircleIntersection(leftCenter: Point, leftRadiusPoint: Point, rightCenter: Point, rightRadiusPoint: Point, variant: number, reference: RuntimeScenePointJson | Point | null | undefined) {
    const leftRadius = Math.hypot(leftRadiusPoint.x - leftCenter.x, leftRadiusPoint.y - leftCenter.y);
    const rightRadius = Math.hypot(rightRadiusPoint.x - rightCenter.x, rightRadiusPoint.y - rightCenter.y);
    const points = circleCircleIntersections(leftCenter, leftRadius, rightCenter, rightRadius);
    return choosePointCandidate(points, reference, variant);
  }

  
  function pointCircularTangent(point: Point, circle: { kind: string; center?: Point; radius?: number; ccwMid?: number; ccwSpan?: number; startAngle?: number; endAngle?: number }, variant: number, reference: RuntimeScenePointJson | Point | null | undefined) {
    if (!circle?.center || !Number.isFinite(circle.radius)) return null;
    const liesOn = scene._pointLiesOnCircularConstraint;
    const radius = circle.radius;
    if (typeof radius !== "number" || !Number.isFinite(radius)) return null;
    const candidates = window.GspRuntimeCore.pointCircleTangents(point, circle.center, radius)
      .filter((candidate) => typeof liesOn === "function" ? liesOn(candidate, circle as { kind: string; center?: Point; radius?: number; ccwMid?: number; ccwSpan?: number; startAngle?: number; endAngle?: number }) : true);
    return choosePointCandidate(candidates, reference, variant);
  }

  scene.registerPointConstraintResolver("line-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const left = resolveLineConstraint(env, constraint.left, resolveFn);
    const right = resolveLineConstraint(env, constraint.right, resolveFn);
    return left && right ? lineLineIntersection(left.start, left.end, left.kind, right.start, right.end, right.kind) : null;
  }));
  scene.registerPointConstraintResolver("line-polygon-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const polygon = constraint.vertexIndices
      .map((index) => resolveFn(index))
      .filter((point): point is Point => point !== null);
    if (!line || polygon.length !== constraint.vertexIndices.length || polygon.length < 2) return null;
    return linePolylineIntersection(
      line.start,
      line.end,
      line.kind,
      [...polygon, polygon[0]],
      null,
      constraint.variant,
    );
  }));
  scene.registerPointConstraintResolver("line-trace-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const tracePoints = typeof scene.sampleCoordinateTracePoints === "function"
      ? scene.sampleCoordinateTracePoints(env as ViewerEnv | null, constraint)
      : null;
    return line && tracePoints
      ? linePolylineIntersection(
          line.start,
          line.end,
          line.kind,
          tracePoints,
          null,
          constraint.variant,
        )
      : null;
  }));
  scene.registerPointConstraintResolver("circular-trace-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const circleFromConstraint = scene._circleFromConstraint;
    const circle = typeof circleFromConstraint === "function"
      ? circleFromConstraint(env as ViewerEnv | null, constraint.circle, resolveFn)
      : null;
    const tracePoints = typeof scene.sampleCoordinateTracePoints === "function"
      ? scene.sampleCoordinateTracePoints(env as ViewerEnv | null, constraint)
      : null;
    return circle && tracePoints
      ? circularPolylineIntersection(circle, tracePoints, constraint.sampleHint, constraint.variant)
      : null;
  }));
  scene.registerPointConstraintResolver("line-function-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const tracePoints = sampleFunctionIntersectionPoints(env, constraint);
    return line && tracePoints
      ? linePolylineIntersection(line.start, line.end, line.kind, tracePoints, constraint.sampleHint)
      : null;
  }));
  scene.registerPointConstraintResolver("point-circular-tangent", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn, reference) => {
    const point = resolveFn(constraint.pointIndex);
    const circleFromConstraint = scene._circleFromConstraint;
    const circle = typeof circleFromConstraint === "function" ? circleFromConstraint(env as ViewerEnv | null, constraint.circle, resolveFn) : null;
    return point && circle ? pointCircularTangent(point, circle, constraint.variant, reference) : null;
  }));
  scene.registerPointConstraintResolver("line-circle-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn, reference) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const center = resolveFn(constraint.centerIndex);
    const radiusPoint = resolveFn(constraint.radiusIndex);
    return line && center && radiusPoint
      ? lineCircleIntersection(line.start, line.end, line.kind, center, radiusPoint, constraint.variant, reference)
      : null;
  }));
  scene.registerPointConstraintResolver("line-circular-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn, reference) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const circleFromConstraint = scene._circleFromConstraint;
    const circle = typeof circleFromConstraint === "function" ? circleFromConstraint(env as ViewerEnv | null, constraint.circle, resolveFn) : null;
    if (!line || !circle) return null;
    const radiusPoint = { x: circle.center.x + circle.radius, y: circle.center.y };
    return lineCircleIntersection(line.start, line.end, line.kind, circle.center, radiusPoint, constraint.variant, reference);
  }));
  scene.registerPointConstraintResolver("circle-circle-intersection", ((_env: ViewerSceneResolverEnv | null, constraint, resolveFn, reference) => {
    const leftCenter = resolveFn(constraint.leftCenterIndex);
    const leftRadiusPoint = resolveFn(constraint.leftRadiusIndex);
    const rightCenter = resolveFn(constraint.rightCenterIndex);
    const rightRadiusPoint = resolveFn(constraint.rightRadiusIndex);
    return leftCenter && leftRadiusPoint && rightCenter && rightRadiusPoint
      ? circleCircleIntersection(leftCenter, leftRadiusPoint, rightCenter, rightRadiusPoint, constraint.variant, reference)
      : null;
  }));
  scene.registerPointConstraintResolver("circular-intersection", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn, reference) => {
    const circleFromConstraint = scene._circleFromConstraint;
    const left = typeof circleFromConstraint === "function" ? circleFromConstraint(env as ViewerEnv | null, constraint.left, resolveFn) : null;
    const right = typeof circleFromConstraint === "function" ? circleFromConstraint(env as ViewerEnv | null, constraint.right, resolveFn) : null;
    if (!left || !right) return null;
    const intersections = circleCircleIntersections(left.center, left.radius, right.center, right.radius);
    if (!intersections || intersections.length === 0) return null;
    const liesOn = scene._pointLiesOnCircularConstraint;
    const onBoth = intersections.filter((point) =>
      (typeof liesOn === "function" ? liesOn(point, left) : true)
      && (typeof liesOn === "function" ? liesOn(point, right) : true)
    );
    return onBoth.length ? choosePointCandidate(onBoth, reference, constraint.variant) : null;
  }));

  scene.lineLineIntersection = lineLineIntersection;
  scene.lineCircleIntersection = lineCircleIntersection;
  scene.circleCircleIntersection = circleCircleIntersection;
})();
