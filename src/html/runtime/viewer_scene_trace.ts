(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const scene =  (modules.scene);

  
  function sampleCoordinateTracePoints(
    env: ViewerEnv | null,
    binding: RuntimeLineBindingJson | RuntimePointConstraintJson,
  ): Point[] | null {
    if (!binding) return null;
    const currentScene = env?.currentScene?.();
    const sampledPointTrace = currentScene?.lines?.find((line) =>
      line.binding?.kind === "point-trace"
      && line.binding.pointIndex === binding.pointIndex
      && Array.isArray(line.points)
      && line.points.length >= 2
    );
    if (sampledPointTrace) {
      const points = sampledPointTrace.points.filter((point): point is Point =>
        typeof point.x === "number" && typeof point.y === "number");
      if (points.length === sampledPointTrace.points.length) return points;
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
    const points = pointBinding.kind === "coordinate-source-2d"
      ? window.GspRuntimeCore.sampleCoordinateTrace(
          pointBinding.xExpr,
          pointBinding.yExpr,
          parameters,
          pointBinding.xName || null,
          pointBinding.yName || null,
          source,
          binding.xMin,
          binding.xMax,
          binding.sampleCount || 0,
          false,
          "two-dimensional",
        )
      : window.GspRuntimeCore.sampleCoordinateTrace(
          pointBinding.expr,
          null,
          parameters,
          pointBinding.name || null,
          null,
          source,
          binding.xMin,
          binding.xMax,
          binding.sampleCount || 0,
          false,
          pointBinding.axis,
        );
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
