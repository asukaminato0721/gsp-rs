(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const scene =  (modules.scene);

  
  function sampleCoordinateTracePoints(env: ViewerEnv | null, binding: RuntimeLineBindingJson | RuntimePointConstraintJson) {
    if (!binding) return null;
    const evaluateExpr = modules.dynamics?.evaluateExpr;
    if (typeof evaluateExpr !== "function") return null;
    const currentScene = env?.currentScene?.();
    const sampledPointTrace = currentScene?.lines?.find((line) =>
      line.binding?.kind === "point-trace"
      && line.binding.pointIndex === binding.pointIndex
      && Array.isArray(line.points)
      && line.points.length >= 2
    );
    if (sampledPointTrace) {
      return sampledPointTrace.points;
    }
    const point = currentScene?.points?.[binding.pointIndex];
    const pointBinding = point?.binding;
    const source = pointBinding?.kind === "coordinate-source" || pointBinding?.kind === "coordinate-source-2d"
      ? env.resolveScenePoint(pointBinding.sourceIndex)
      : null;
    if (
      !source
      || (pointBinding?.kind !== "coordinate-source"
        && pointBinding?.kind !== "coordinate-source-2d")
    ) return null;
    const parameters = env?.currentDynamics
      ? new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]))
      : new Map();
    
    const points = [];
    const last = Math.max(1, (binding.sampleCount || 0) - 1);
    for (let index = 0; index < (binding.sampleCount || 0); index += 1) {
      const t = index / last;
      const value = binding.xMin + (binding.xMax - binding.xMin) * t;
      const exprParameters = new Map(parameters);
      if (pointBinding.kind === "coordinate-source-2d") {
        if (typeof pointBinding.xName === "string" && pointBinding.xName.length > 0) {
          exprParameters.set(pointBinding.xName, value);
        }
        if (typeof pointBinding.yName === "string" && pointBinding.yName.length > 0) {
          exprParameters.set(pointBinding.yName, value);
        }
        const dx = evaluateExpr(pointBinding.xExpr, 0, exprParameters);
        const dy = evaluateExpr(pointBinding.yExpr, 0, exprParameters);
        if (dx === null || dy === null) continue;
        points.push({ x: source.x + dx, y: source.y + dy });
      } else {
        if (typeof pointBinding.name === "string" && pointBinding.name.length > 0) {
          exprParameters.set(pointBinding.name, value);
        }
        const offset = evaluateExpr(pointBinding.expr, 0, exprParameters);
        if (offset === null) continue;
        points.push(
          pointBinding.axis === "horizontal"
            ? { x: source.x + offset, y: source.y }
            : { x: source.x, y: source.y + offset }
        );
      }
    }
    return points.length >= 2 ? points : null;
  }

  
  function resolvePolylineConstraintPoints(env: ViewerEnv | null, constraint: RuntimePointConstraintJson, resolveFn: (index: number) => Point | null) {
    const hasRuntimeScene = typeof env?.currentScene === "function";
    const currentScene = hasRuntimeScene ? env.currentScene() : env?.sourceScene;
    if (typeof constraint.functionKey === "number") {
      const hostLine = currentScene?.lines?.find(( line) =>
        line?.binding?.kind === "arc-boundary" && line.binding.hostKey === constraint.functionKey
        || line?.debug?.groupOrdinal === constraint.functionKey
          && (
            line?.binding?.kind === "point-trace"
            || line?.binding?.kind === "coordinate-trace"
            || line?.binding?.kind === "custom-transform-trace"
          )
      );
      if (hostLine?.binding?.kind === "arc-boundary") {
        if (hasRuntimeScene && typeof scene.sampleArcBoundaryPoints === "function") {
          return scene.sampleArcBoundaryPoints(env, hostLine.binding);
        }
        return hostLine.points.map(( handle) => {
          if (handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number") {
            return resolveFn(handle.pointIndex);
          }
          return  (handle);
        });
      }
      if (
        hostLine?.binding?.kind === "point-trace"
        || hostLine?.binding?.kind === "coordinate-trace"
        || hostLine?.binding?.kind === "custom-transform-trace"
      ) {
        return scene.resolveLinePoints(env, hostLine) || hostLine.points;
      }
    }
    return constraint.points.map(( handle) => {
      if (handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number") {
        return resolveFn(handle.pointIndex);
      }
      return  (handle);
    });
  }

  scene.registerPointConstraintResolver("polyline", ((env: ViewerEnv, constraint, resolveFn) => {
    const points = resolvePolylineConstraintPoints(env, constraint, resolveFn);
    if (!points || points.length < 2) return null;
    const segmentIndex = Math.max(0, Math.min(points.length - 2, constraint.segmentIndex));
    const start = points[segmentIndex];
    const end = points[segmentIndex + 1];
    return start && end ? scene.lerpPoint(start, end, constraint.t) : null;
  }));
  scene.registerLineBindingResolver("coordinate-trace", ((env: ViewerEnv, line) => sampleCoordinateTracePoints(env, line.binding)));
  scene.sampleCoordinateTracePoints = sampleCoordinateTracePoints;
})();
