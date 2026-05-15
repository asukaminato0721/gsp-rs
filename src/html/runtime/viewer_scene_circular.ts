// @ts-nocheck

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const scene = /** @type {any} */ (modules.scene);
  const pointOnCircleArc = scene.pointOnCircleArc;
  const pointOnThreePointArc = scene.pointOnThreePointArc;
  const pointOnThreePointArcComplement = scene._pointOnThreePointArcComplement;

  /**
   * @param {ViewerEnv} env
   * @param {Extract<LineBindingJson, { kind: "arc-boundary" }> | RuntimePointConstraintJson} binding
   * @returns {Point[] | null}
   */
  function sampleArcBoundaryPoints(env, binding) {
    const steps = 48;
    const start = scene.resolveScenePoint(env, binding.startIndex);
    const end = scene.resolveScenePoint(env, binding.endIndex);
    if (!start || !end) return null;
    const reversed = !!binding.reversed;
    /** @type {Point[]} */
    const sampledArc = [];

    if (typeof binding.centerIndex === "number") {
      const center = scene.resolveScenePoint(env, binding.centerIndex);
      if (!center) return null;
      for (let step = 0; step <= steps; step += 1) {
        const point = pointOnCircleArc(center, start, end, step / steps, !!env.sourceScene.yUp);
        if (!point) return null;
        sampledArc.push(point);
      }
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
    for (let step = 0; step <= steps; step += 1) {
      const point = binding.complement
        ? pointOnThreePointArcComplement(start, mid, end, step / steps)
        : pointOnThreePointArc(start, mid, end, step / steps);
      if (!point) return null;
      sampledArc.push(point);
    }
    if (binding.boundaryKind === "sector") {
      return reversed ? [end, mid, start, ...sampledArc.slice(1)] : [start, ...sampledArc.slice(1)];
    }
    return reversed ? [end, start, ...sampledArc.slice(1)] : [start, ...sampledArc.slice(1), start];
  }

  scene.registerPointConstraintResolver("circle", /** @type {any} */((_env, constraint, resolveFn) => {
    const center = resolveFn(constraint.centerIndex);
    const radiusPoint = resolveFn(constraint.radiusIndex);
    if (!center || !radiusPoint) return null;
    const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
    return {
      x: center.x + radius * constraint.unitX,
      y: center.y + radius * constraint.unitY,
    };
  }));
  scene.registerPointConstraintResolver("circle-arc", /** @type {any} */((env, constraint, resolveFn) => {
    const center = resolveFn(constraint.centerIndex);
    const start = resolveFn(constraint.startIndex);
    const end = resolveFn(constraint.endIndex);
    return center && start && end ? pointOnCircleArc(center, start, end, constraint.t, !!env?.sourceScene?.yUp) : null;
  }));
  scene.registerPointConstraintResolver("arc", /** @type {any} */((_env, constraint, resolveFn) => {
    const start = resolveFn(constraint.startIndex);
    const mid = resolveFn(constraint.midIndex);
    const end = resolveFn(constraint.endIndex);
    return start && mid && end ? pointOnThreePointArc(start, mid, end, constraint.t) : null;
  }));
  scene.registerLineBindingResolver("arc-boundary", /** @type {any} */((env, line) => sampleArcBoundaryPoints(env, line.binding)));

  scene.sampleArcBoundaryPoints = sampleArcBoundaryPoints;
})();
