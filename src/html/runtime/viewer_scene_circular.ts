(function() {
  const modules = (window.GspViewerModules || (window.GspViewerModules = {})) as Partial<ViewerModules> & {
    scene: ViewerSceneModule;
  };
  const scene = modules.scene;

  
  function sampleArcBoundaryPoints(env: ViewerEnv, binding: RuntimeLineBindingJson | RuntimeShapeBindingJson | RuntimePointConstraintJson) {
    const steps = 48;
    if (typeof binding.startIndex !== "number" || typeof binding.endIndex !== "number") return null;
    const start = scene.resolveScenePoint(env, binding.startIndex);
    const end = scene.resolveScenePoint(env, binding.endIndex);
    if (!start || !end) return null;
    const reversed = !!binding.reversed;
    
    const sampledArc: Point[] = [];

    if (typeof binding.centerIndex === "number") {
      const center = scene.resolveScenePoint(env, binding.centerIndex);
      if (!center) return null;
      const points = window.GspRuntimeCore.sampleCircleArc(center, start, end, steps, !!env.sourceScene.yUp);
      if (!points) return null;
      sampledArc.push(...points);
      if (binding.boundaryKind === "sector") {
        return reversed
          ? [end, center, start, ...sampledArc.slice(1)]
          : [center, start, ...sampledArc.slice(1), center];
      }
      return reversed
        ? [end, start, ...sampledArc.slice(1)]
        : [start, ...sampledArc.slice(1), start];
    }

    if (typeof binding.midIndex !== "number") return null;
    const mid = scene.resolveScenePoint(env, binding.midIndex);
    if (!mid) return null;
    const points = window.GspRuntimeCore.sampleThreePointArc(
      start,
      mid,
      end,
      steps,
      binding.complement === true,
    );
    if (!points) return null;
    sampledArc.push(...points);
    if (binding.boundaryKind === "sector") {
      return reversed ? [end, mid, start, ...sampledArc.slice(1)] : [start, ...sampledArc.slice(1)];
    }
    return reversed ? [end, start, ...sampledArc.slice(1)] : [start, ...sampledArc.slice(1), start];
  }

  scene.registerPointConstraintResolver("circle", ((_env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const center = resolveFn(constraint.centerIndex);
    const radiusPoint = resolveFn(constraint.radiusIndex);
    if (!center || !radiusPoint) return null;
    const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
    return {
      x: center.x + radius * constraint.unitX,
      y: center.y + radius * constraint.unitY,
    };
  }));
  scene.registerPointConstraintResolver("circle-arc", ((env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const center = resolveFn(constraint.centerIndex);
    const start = resolveFn(constraint.startIndex);
    const end = resolveFn(constraint.endIndex);
    return center && start && end
      ? window.GspRuntimeCore.pointOnCircleArc(center, start, end, constraint.t, !!env?.sourceScene?.yUp)
      : null;
  }));
  scene.registerPointConstraintResolver("arc", ((_env: ViewerSceneResolverEnv | null, constraint, resolveFn) => {
    const start = resolveFn(constraint.startIndex);
    const mid = resolveFn(constraint.midIndex);
    const end = resolveFn(constraint.endIndex);
    return start && mid && end
      ? window.GspRuntimeCore.pointOnThreePointArc(start, mid, end, constraint.t, false)
      : null;
  }));
  scene.registerLineBindingResolver("arc-boundary", ((env: ViewerEnv, line) => sampleArcBoundaryPoints(env, line.binding)));

  scene.sampleArcBoundaryPoints = sampleArcBoundaryPoints;
})();
