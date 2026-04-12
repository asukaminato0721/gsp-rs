// @ts-nocheck

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const scene = /** @type {any} */ (modules.scene);

  /**
   * @param {ViewerEnv | null} env
   * @param {Extract<LineBindingJson, { kind: "coordinate-trace" }> | Extract<RuntimePointConstraintJson, { kind: "line-trace-intersection" }>} binding
   * @returns {Point[] | null}
   */
  function sampleCoordinateTracePoints(env, binding) {
    if (!binding) return null;
    const evaluateExpr = modules.dynamics?.evaluateExpr;
    if (typeof evaluateExpr !== "function") return null;
    const point = env?.currentScene?.().points?.[binding.pointIndex];
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
    /** @type {Point[]} */
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

  /**
   * @param {ViewerEnv | null} env
   * @param {RuntimePointConstraintJson} constraint
   * @param {(index: number) => Point | null} resolveFn
   * @returns {Point[] | null}
   */
  function resolvePolylineConstraintPoints(env, constraint, resolveFn) {
    const hasRuntimeScene = typeof env?.currentScene === "function";
    const currentScene = hasRuntimeScene ? env.currentScene() : env?.sourceScene;
    if (typeof constraint.functionKey === "number") {
      const hostLine = currentScene?.lines?.find((/** @type {RuntimeLineJson} */ line) =>
        line?.binding?.kind === "arc-boundary" && line.binding.hostKey === constraint.functionKey
      );
      if (hostLine?.binding?.kind === "arc-boundary") {
        if (hasRuntimeScene && typeof scene.sampleArcBoundaryPoints === "function") {
          return scene.sampleArcBoundaryPoints(env, hostLine.binding);
        }
        return hostLine.points.map((/** @type {PointHandle} */ handle) => {
          if (handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number") {
            return resolveFn(handle.pointIndex);
          }
          return /** @type {Point} */ (handle);
        });
      }
    }
    return constraint.points.map((/** @type {PointHandle} */ handle) => {
      if (handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number") {
        return resolveFn(handle.pointIndex);
      }
      return /** @type {Point} */ (handle);
    });
  }

  scene.registerPointConstraintResolver("polyline", /** @type {any} */((env, constraint, resolveFn) => {
    const points = resolvePolylineConstraintPoints(env, constraint, resolveFn);
    if (!points || points.length < 2) return null;
    const segmentIndex = Math.max(0, Math.min(points.length - 2, constraint.segmentIndex));
    const start = points[segmentIndex];
    const end = points[segmentIndex + 1];
    return start && end ? scene.lerpPoint(start, end, constraint.t) : null;
  }));
  scene.registerLineBindingResolver("coordinate-trace", /** @type {any} */((env, line) => sampleCoordinateTracePoints(env, line.binding)));
  scene.sampleCoordinateTracePoints = sampleCoordinateTracePoints;
})();
