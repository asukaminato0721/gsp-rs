// @ts-nocheck

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const scene = /** @type {any} */ (modules.scene);

  /**
   * @param {number} t
   * @param {string} kind
   */
  function lineLikeAllowsParam(t, kind) {
    if (kind === "line") return true;
    if (kind === "ray") return t >= -1e-9;
    return t >= -1e-9 && t <= 1 + 1e-9;
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {string} kind
   * @param {Point} point
   */
  function lineLikeContainsPoint(start, end, kind, point) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lenSquared = dx * dx + dy * dy;
    if (lenSquared <= 1e-9) return false;
    const t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSquared;
    return lineLikeAllowsParam(t, kind);
  }

  /**
   * @param {Point} leftStart
   * @param {Point} leftEnd
   * @param {string} leftKind
   * @param {Point} rightStart
   * @param {Point} rightEnd
   * @param {string} rightKind
   */
  function lineLineIntersection(leftStart, leftEnd, leftKind, rightStart, rightEnd, rightKind) {
    const leftDx = leftEnd.x - leftStart.x;
    const leftDy = leftEnd.y - leftStart.y;
    const rightDx = rightEnd.x - rightStart.x;
    const rightDy = rightEnd.y - rightStart.y;
    const determinant = leftDx * rightDy - leftDy * rightDx;
    if (Math.abs(determinant) <= 1e-9) return null;
    const deltaX = rightStart.x - leftStart.x;
    const deltaY = rightStart.y - leftStart.y;
    const t = (deltaX * rightDy - deltaY * rightDx) / determinant;
    const point = {
      x: leftStart.x + t * leftDx,
      y: leftStart.y + t * leftDy,
    };
    return lineLikeContainsPoint(leftStart, leftEnd, leftKind, point)
      && lineLikeContainsPoint(rightStart, rightEnd, rightKind, point)
      ? point
      : null;
  }

  /**
   * @param {Point} lineStart
   * @param {Point} lineEnd
   * @param {string} lineKind
   * @param {Point[] | null} points
   */
  function linePolylineIntersection(lineStart, lineEnd, lineKind, points) {
    if (!Array.isArray(points) || points.length < 2) return null;
    for (let index = 0; index < points.length - 1; index += 1) {
      const start = points[index];
      const end = points[index + 1];
      if (!start || !end) continue;
      const hit = lineLineIntersection(lineStart, lineEnd, lineKind, start, end, "segment");
      if (hit) return hit;
    }
    return null;
  }

  /**
   * @param {Point[] | null} candidates
   * @param {RuntimeScenePointJson | Point | null | undefined} reference
   * @param {number} variant
   */
  function choosePointCandidate(candidates, reference, variant) {
    if (!Array.isArray(candidates) || candidates.length === 0) return null;
    if (reference && Number.isFinite(reference.x) && Number.isFinite(reference.y)) {
      return candidates.reduce((best, candidate) => {
        if (!best) return candidate;
        const bestDistance = (best.x - reference.x) ** 2 + (best.y - reference.y) ** 2;
        const candidateDistance = (candidate.x - reference.x) ** 2 + (candidate.y - reference.y) ** 2;
        return candidateDistance < bestDistance ? candidate : best;
      }, null);
    }
    return candidates[Math.max(0, Math.min(candidates.length - 1, variant || 0))] || null;
  }

  /**
   * @param {ViewerEnv | null} env
   * @param {LineConstraintJson} constraint
   * @param {(index: number) => Point | null} resolveFn
   * @returns {{ start: Point; end: Point; kind: string } | null}
   */
  function resolveLineConstraint(env, constraint, resolveFn) {
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
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolveFn(constraint.startIndex);
      const vertex = resolveFn(constraint.vertexIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = scene.resolveLinePoints
        ? (() => {
            const dir = (() => {
              const startDx = start.x - vertex.x;
              const startDy = start.y - vertex.y;
              const startLen = Math.hypot(startDx, startDy);
              const endDx = end.x - vertex.x;
              const endDy = end.y - vertex.y;
              const endLen = Math.hypot(endDx, endDy);
              if (startLen <= 1e-9 || endLen <= 1e-9) return null;
              const sumX = startDx / startLen + endDx / endLen;
              const sumY = startDy / startLen + endDy / endLen;
              const sumLen = Math.hypot(sumX, sumY);
              return sumLen > 1e-9 ? { x: sumX / sumLen, y: sumY / sumLen } : { x: -startDy / startLen, y: startDx / startLen };
            })();
            return dir;
          })()
        : null;
      if (!direction) return null;
      return { start: vertex, end: { x: vertex.x + direction.x, y: vertex.y + direction.y }, kind: "ray" };
    }
    if (constraint.kind === "translated") {
      const base = resolveLineConstraint(env, constraint.line, resolveFn);
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
    return null;
  }

  /**
   * @param {Point} lineStart
   * @param {Point} lineEnd
   * @param {string} lineKind
   * @param {Point} center
   * @param {Point} radiusPoint
   * @param {number} variant
   * @param {RuntimeScenePointJson | Point | null | undefined} reference
   */
  function lineCircleIntersection(lineStart, lineEnd, lineKind, center, radiusPoint, variant, reference) {
    const dx = lineEnd.x - lineStart.x;
    const dy = lineEnd.y - lineStart.y;
    const a = dx * dx + dy * dy;
    if (a <= 1e-9) return null;
    const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
    if (radius <= 1e-9) return null;
    const fx = lineStart.x - center.x;
    const fy = lineStart.y - center.y;
    const b = 2 * (fx * dx + fy * dy);
    const c = fx * fx + fy * fy - radius * radius;
    const discriminant = b * b - 4 * a * c;
    if (discriminant < -1e-9) return null;
    const root = Math.sqrt(Math.max(0, discriminant));
    const ts = [(-b - root) / (2 * a), (-b + root) / (2 * a)]
      .filter((t) => lineLikeAllowsParam(t, lineKind))
      .sort((left, right) => left - right);
    if (ts.length === 0) return null;
    return choosePointCandidate(
      ts.map((t) => ({ x: lineStart.x + dx * t, y: lineStart.y + dy * t })),
      reference,
      variant,
    );
  }

  /**
   * @param {Point} leftCenter
   * @param {number} leftRadius
   * @param {Point} rightCenter
   * @param {number} rightRadius
   * @returns {Point[] | null}
   */
  function circleCircleIntersections(leftCenter, leftRadius, rightCenter, rightRadius) {
    if (leftRadius <= 1e-9 || rightRadius <= 1e-9) return null;
    const dx = rightCenter.x - leftCenter.x;
    const dy = rightCenter.y - leftCenter.y;
    const distance = Math.hypot(dx, dy);
    if (
      distance <= 1e-9 ||
      distance > leftRadius + rightRadius + 1e-9 ||
      distance < Math.abs(leftRadius - rightRadius) - 1e-9
    ) {
      return null;
    }
    const along = (leftRadius * leftRadius - rightRadius * rightRadius + distance * distance) / (2 * distance);
    const heightSquared = leftRadius * leftRadius - along * along;
    if (heightSquared < -1e-9) return null;
    const height = Math.sqrt(Math.max(0, heightSquared));
    const ux = dx / distance;
    const uy = dy / distance;
    const base = { x: leftCenter.x + along * ux, y: leftCenter.y + along * uy };
    return [
      { x: base.x - height * uy, y: base.y + height * ux },
      { x: base.x + height * uy, y: base.y - height * ux },
    ].sort((left, right) => (left.y - right.y) || (left.x - right.x));
  }

  /**
   * @param {Point} leftCenter
   * @param {Point} leftRadiusPoint
   * @param {Point} rightCenter
   * @param {Point} rightRadiusPoint
   * @param {number} variant
   * @param {RuntimeScenePointJson | Point | null | undefined} reference
   */
  function circleCircleIntersection(leftCenter, leftRadiusPoint, rightCenter, rightRadiusPoint, variant, reference) {
    const leftRadius = Math.hypot(leftRadiusPoint.x - leftCenter.x, leftRadiusPoint.y - leftCenter.y);
    const rightRadius = Math.hypot(rightRadiusPoint.x - rightCenter.x, rightRadiusPoint.y - rightCenter.y);
    const points = circleCircleIntersections(leftCenter, leftRadius, rightCenter, rightRadius);
    return points && points.length ? choosePointCandidate(points, reference, variant) : null;
  }

  /**
   * @param {Point} point
   * @param {any} circle
   * @param {number} variant
   * @param {RuntimeScenePointJson | Point | null | undefined} reference
   */
  function pointCircularTangent(point, circle, variant, reference) {
    if (!circle) return null;
    const dx = point.x - circle.center.x;
    const dy = point.y - circle.center.y;
    const distanceSquared = dx * dx + dy * dy;
    if (distanceSquared <= circle.radius * circle.radius + 1e-9) return null;
    const distance = Math.sqrt(distanceSquared);
    const baseAngle = Math.atan2(dy, dx);
    const offset = Math.acos(circle.radius / distance);
    const liesOn = scene._pointLiesOnCircularConstraint;
    const candidates = [
      { x: circle.center.x + circle.radius * Math.cos(baseAngle - offset), y: circle.center.y + circle.radius * Math.sin(baseAngle - offset) },
      { x: circle.center.x + circle.radius * Math.cos(baseAngle + offset), y: circle.center.y + circle.radius * Math.sin(baseAngle + offset) },
    ]
      .filter((candidate) => typeof liesOn === "function" ? liesOn(candidate, circle) : true)
      .sort((left, right) => (left.y - right.y) || (left.x - right.x));
    return choosePointCandidate(candidates, reference, variant);
  }

  scene.registerPointConstraintResolver("line-intersection", /** @type {any} */((env, constraint, resolveFn) => {
    const left = resolveLineConstraint(env, constraint.left, resolveFn);
    const right = resolveLineConstraint(env, constraint.right, resolveFn);
    return left && right ? lineLineIntersection(left.start, left.end, left.kind, right.start, right.end, right.kind) : null;
  }));
  scene.registerPointConstraintResolver("line-trace-intersection", /** @type {any} */((env, constraint, resolveFn) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const tracePoints = typeof scene.sampleCoordinateTracePoints === "function"
      ? scene.sampleCoordinateTracePoints(env, constraint)
      : null;
    return line && tracePoints ? linePolylineIntersection(line.start, line.end, line.kind, tracePoints) : null;
  }));
  scene.registerPointConstraintResolver("point-circular-tangent", /** @type {any} */((env, constraint, resolveFn, reference) => {
    const point = resolveFn(constraint.pointIndex);
    const circleFromConstraint = scene._circleFromConstraint;
    const circle = typeof circleFromConstraint === "function" ? circleFromConstraint(env, constraint.circle, resolveFn) : null;
    return point && circle ? pointCircularTangent(point, circle, constraint.variant, reference) : null;
  }));
  scene.registerPointConstraintResolver("line-circle-intersection", /** @type {any} */((env, constraint, resolveFn, reference) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const center = resolveFn(constraint.centerIndex);
    const radiusPoint = resolveFn(constraint.radiusIndex);
    return line && center && radiusPoint
      ? lineCircleIntersection(line.start, line.end, line.kind, center, radiusPoint, constraint.variant, reference)
      : null;
  }));
  scene.registerPointConstraintResolver("line-circular-intersection", /** @type {any} */((env, constraint, resolveFn, reference) => {
    const line = resolveLineConstraint(env, constraint.line, resolveFn);
    const circleFromConstraint = scene._circleFromConstraint;
    const circle = typeof circleFromConstraint === "function" ? circleFromConstraint(env, constraint.circle, resolveFn) : null;
    if (!line || !circle) return null;
    const radiusPoint = { x: circle.center.x + circle.radius, y: circle.center.y };
    return lineCircleIntersection(line.start, line.end, line.kind, circle.center, radiusPoint, constraint.variant, reference);
  }));
  scene.registerPointConstraintResolver("circle-circle-intersection", /** @type {any} */((_env, constraint, resolveFn, reference) => {
    const leftCenter = resolveFn(constraint.leftCenterIndex);
    const leftRadiusPoint = resolveFn(constraint.leftRadiusIndex);
    const rightCenter = resolveFn(constraint.rightCenterIndex);
    const rightRadiusPoint = resolveFn(constraint.rightRadiusIndex);
    return leftCenter && leftRadiusPoint && rightCenter && rightRadiusPoint
      ? circleCircleIntersection(leftCenter, leftRadiusPoint, rightCenter, rightRadiusPoint, constraint.variant, reference)
      : null;
  }));
  scene.registerPointConstraintResolver("circular-intersection", /** @type {any} */((env, constraint, resolveFn, reference) => {
    const circleFromConstraint = scene._circleFromConstraint;
    const left = typeof circleFromConstraint === "function" ? circleFromConstraint(env, constraint.left, resolveFn) : null;
    const right = typeof circleFromConstraint === "function" ? circleFromConstraint(env, constraint.right, resolveFn) : null;
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
