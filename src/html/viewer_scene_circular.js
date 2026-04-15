// @ts-nocheck

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const scene = /** @type {any} */ (modules.scene);

  /**
   * @param {number} from
   * @param {number} to
   */
  function normalizeAngleDelta(from, to) {
    const tau = Math.PI * 2;
    return ((to - from) % tau + tau) % tau;
  }

  /**
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   */
  function threePointArcGeometry(start, mid, end) {
    const determinant = 2 * (
      start.x * (mid.y - end.y)
      + mid.x * (end.y - start.y)
      + end.x * (start.y - mid.y)
    );
    if (Math.abs(determinant) <= 1e-9) return null;

    const startSq = start.x * start.x + start.y * start.y;
    const midSq = mid.x * mid.x + mid.y * mid.y;
    const endSq = end.x * end.x + end.y * end.y;
    const center = {
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
    const radius = Math.hypot(start.x - center.x, start.y - center.y);
    if (radius <= 1e-9) return null;

    const startAngle = Math.atan2(start.y - center.y, start.x - center.x);
    const midAngle = Math.atan2(mid.y - center.y, mid.x - center.x);
    const endAngle = Math.atan2(end.y - center.y, end.x - center.x);
    return {
      start,
      mid,
      end,
      center,
      radius,
      startAngle,
      midAngle,
      endAngle,
      ccwSpan: normalizeAngleDelta(startAngle, endAngle),
      ccwMid: normalizeAngleDelta(startAngle, midAngle),
    };
  }

  /**
   * @param {Point} center
   * @param {Point} start
   * @param {Point} end
   * @param {boolean} yUp
   */
  function circleArcControlPoints(center, start, end, yUp) {
    const startDx = start.x - center.x;
    const startDy = start.y - center.y;
    const endDx = end.x - center.x;
    const endDy = end.y - center.y;
    const startRadius = Math.hypot(startDx, startDy);
    const endRadius = Math.hypot(endDx, endDy);
    const radius = (startRadius + endRadius) * 0.5;
    if (radius <= 1e-9) return null;

    const ySign = yUp ? 1 : -1;
    const startAngle = Math.atan2(startDy * ySign, startDx);
    const endAngle = Math.atan2(endDy * ySign, endDx);
    const ccwSpan = normalizeAngleDelta(startAngle, endAngle);
    const midpointAngle = startAngle + ccwSpan * 0.5;
    return {
      start,
      mid: {
        x: center.x + radius * Math.cos(midpointAngle),
        y: center.y + ySign * radius * Math.sin(midpointAngle),
      },
      end,
    };
  }

  /**
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   * @param {number} t
   * @returns {Point | null}
   */
  function pointOnThreePointArc(start, mid, end, t) {
    const geometry = threePointArcGeometry(start, mid, end);
    if (!geometry) return null;
    return pointOnThreePointArcWithGeometry(geometry, t, false);
  }

  /**
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   * @param {number} t
   * @returns {Point | null}
   */
  function pointOnThreePointArcComplement(start, mid, end, t) {
    const geometry = threePointArcGeometry(start, mid, end);
    if (!geometry) return null;
    return pointOnThreePointArcWithGeometry(geometry, t, true);
  }

  /**
   * @param {{ center: Point; radius: number; startAngle: number; endAngle: number; ccwSpan: number; ccwMid: number }} geometry
   * @param {number} t
   * @param {boolean} complement
   */
  function pointOnThreePointArcWithGeometry(geometry, t, complement) {
    const clampedT = Math.max(0, Math.min(1, t));
    const useCcw = complement
      ? geometry.ccwMid > geometry.ccwSpan + 1e-9
      : geometry.ccwMid <= geometry.ccwSpan + 1e-9;
    const angle = useCcw
      ? geometry.startAngle + geometry.ccwSpan * clampedT
      : geometry.startAngle - normalizeAngleDelta(geometry.endAngle, geometry.startAngle) * clampedT;
    return {
      x: geometry.center.x + geometry.radius * Math.cos(angle),
      y: geometry.center.y + geometry.radius * Math.sin(angle),
    };
  }

  /**
   * @param {Point} center
   * @param {Point} start
   * @param {Point} end
   * @param {number} t
   * @param {boolean} yUp
   */
  function pointOnCircleArc(center, start, end, t, yUp) {
    const controls = circleArcControlPoints(center, start, end, yUp);
    if (!controls) return null;
    return pointOnThreePointArc(controls.start, controls.mid, controls.end, t);
  }

  /**
   * @param {Point} point
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   */
  function projectToThreePointArc(point, start, mid, end) {
    let best = null;
    const steps = 256;
    for (let step = 0; step <= steps; step += 1) {
      const t = step / steps;
      const projected = pointOnThreePointArc(start, mid, end, t);
      if (!projected) return null;
      const distanceSquared = (point.x - projected.x) ** 2 + (point.y - projected.y) ** 2;
      if (!best || distanceSquared < best.distanceSquared) {
        best = { t, projected, distanceSquared };
      }
    }
    return best;
  }

  /**
   * @param {Point} point
   * @param {Point} center
   * @param {Point} start
   * @param {Point} end
   * @param {boolean} yUp
   */
  function projectToCircleArc(point, center, start, end, yUp) {
    const controls = circleArcControlPoints(center, start, end, yUp);
    if (!controls) return null;
    return projectToThreePointArc(point, controls.start, controls.mid, controls.end);
  }

  /**
   * @param {ViewerEnv | null} env
   * @param {CircularConstraintJson | null} constraint
   * @param {(index: number) => Point | null} resolveFn
   */
  function circleFromConstraint(env, constraint, resolveFn) {
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

  /**
   * @param {Point} point
   * @param {any} constraint
   */
  function pointLiesOnCircularConstraint(point, constraint) {
    if (!constraint) return false;
    if (constraint.kind === "circle") {
      return true;
    }
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

  scene.pointOnCircleArc = pointOnCircleArc;
  scene.projectToCircleArc = projectToCircleArc;
  scene.pointOnThreePointArc = pointOnThreePointArc;
  scene.projectToThreePointArc = projectToThreePointArc;
  scene.sampleArcBoundaryPoints = sampleArcBoundaryPoints;
  scene._threePointArcGeometry = threePointArcGeometry;
  scene._pointOnThreePointArcComplement = pointOnThreePointArcComplement;
  scene._circleFromConstraint = circleFromConstraint;
  scene._pointLiesOnCircularConstraint = pointLiesOnCircularConstraint;
})();
