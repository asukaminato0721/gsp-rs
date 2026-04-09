// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function normalizeAngleDelta(from, to) {
    const tau = Math.PI * 2;
    return ((to - from) % tau + tau) % tau;
  }

  function lerpPoint(start, end, t) {
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }

  function resolvePolylinePoint(env, handle, resolveFn) {
    if (!handle) return null;
    if (typeof handle.pointIndex === "number") {
      return resolveFn(handle.pointIndex);
    }
    return handle;
  }

  function projectToSegment(point, start, end) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lengthSquared = dx * dx + dy * dy;
    if (lengthSquared <= 1e-9) {
      return null;
    }
    const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared));
    const projected = lerpPoint(start, end, t);
    return {
      t,
      projected,
      distanceSquared: (point.x - projected.x) ** 2 + (point.y - projected.y) ** 2,
    };
  }

  function clipParametricLineToBounds(start, end, bounds, rayOnly) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return null;

    const hits = [];
    const pushHit = (t, point) => {
      if (!Number.isFinite(t)) return;
      if (rayOnly && t < -1e-9) return;
      if (
        point.x < bounds.minX - 1e-6 || point.x > bounds.maxX + 1e-6 ||
        point.y < bounds.minY - 1e-6 || point.y > bounds.maxY + 1e-6
      ) return;
      if (hits.some((hit) =>
        Math.abs(hit.t - t) < 1e-6 ||
        (Math.abs(hit.point.x - point.x) < 1e-6 && Math.abs(hit.point.y - point.y) < 1e-6)
      )) return;
      hits.push({ t, point });
    };

    if (Math.abs(dx) > 1e-9) {
      for (const x of [bounds.minX, bounds.maxX]) {
        const t = (x - start.x) / dx;
        pushHit(t, { x, y: start.y + dy * t });
      }
    }
    if (Math.abs(dy) > 1e-9) {
      for (const y of [bounds.minY, bounds.maxY]) {
        const t = (y - start.y) / dy;
        pushHit(t, { x: start.x + dx * t, y });
      }
    }
    if (
      rayOnly &&
      start.x >= bounds.minX - 1e-6 && start.x <= bounds.maxX + 1e-6 &&
      start.y >= bounds.minY - 1e-6 && start.y <= bounds.maxY + 1e-6
    ) {
      pushHit(0, { ...start });
    }
    if (hits.length < 2) return null;
    hits.sort((a, b) => a.t - b.t);
    return [hits[0].point, hits[hits.length - 1].point];
  }

  function angleBisectorDirection(start, vertex, end) {
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
    if (sumLen > 1e-9) {
      return { x: sumX / sumLen, y: sumY / sumLen };
    }

    return { x: -startDy / startLen, y: startDx / startLen };
  }

  function pointOnThreePointArc(start, mid, end, t) {
    const geometry = threePointArcGeometry(start, mid, end);
    if (!geometry) return null;
    const clampedT = Math.max(0, Math.min(1, t));
    const angle = geometry.ccwMid <= geometry.ccwSpan + 1e-9
      ? geometry.startAngle + geometry.ccwSpan * clampedT
      : geometry.startAngle - normalizeAngleDelta(geometry.endAngle, geometry.startAngle) * clampedT;
    return {
      x: geometry.center.x + geometry.radius * Math.cos(angle),
      y: geometry.center.y + geometry.radius * Math.sin(angle),
    };
  }

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

  function pointOnCircleArc(center, start, end, t, yUp) {
    const controls = circleArcControlPoints(center, start, end, yUp);
    if (!controls) return null;
    return pointOnThreePointArc(controls.start, controls.mid, controls.end, t);
  }

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
    if (constraint.kind === "three-point-arc") {
      const start = resolveFn(constraint.startIndex);
      const mid = resolveFn(constraint.midIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !mid || !end) return null;
      const geometry = threePointArcGeometry(start, mid, end);
      if (!geometry) return null;
      return {
        kind: "three-point-arc",
        ...geometry,
      };
    }
    return null;
  }

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

  function sampleArcBoundaryPoints(env, binding) {
    const steps = 48;
    const start = resolveScenePoint(env, binding.startIndex);
    const end = resolveScenePoint(env, binding.endIndex);
    if (!start || !end) return null;
    const reversed = !!binding.reversed;
    const sampledArc = [];

    if (typeof binding.centerIndex === "number") {
      const center = resolveScenePoint(env, binding.centerIndex);
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
    const mid = resolveScenePoint(env, binding.midIndex);
    if (!mid) return null;
    for (let step = 0; step <= steps; step += 1) {
      const point = pointOnThreePointArc(start, mid, end, step / steps);
      if (!point) return null;
      sampledArc.push(point);
    }
    if (binding.boundaryKind === "sector") {
      return reversed
        ? [end, mid, start, ...sampledArc.slice(1)]
        : [start, ...sampledArc.slice(1)];
    }
    return reversed
      ? [end, start, ...sampledArc.slice(1)]
      : [start, ...sampledArc.slice(1), start];
  }

  function resolvePolylineConstraintPoints(env, constraint, resolveFn) {
    const hasRuntimeScene = typeof env?.currentScene === "function";
    const scene = hasRuntimeScene ? env.currentScene() : env?.sourceScene;
    if (typeof constraint.functionKey === "number") {
      const hostLine = scene?.lines?.find((line) =>
        line?.binding?.kind === "arc-boundary" && line.binding.hostKey === constraint.functionKey
      );
      if (hostLine?.binding?.kind === "arc-boundary") {
        if (hasRuntimeScene) {
          return sampleArcBoundaryPoints(env, hostLine.binding);
        }
        return hostLine.points.map((handle) => resolvePolylinePoint(env, handle, resolveFn));
      }
    }
    return constraint.points.map((handle) => resolvePolylinePoint(env, handle, resolveFn));
  }

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

  function projectToCircleArc(point, center, start, end, yUp) {
    const controls = circleArcControlPoints(center, start, end, yUp);
    if (!controls) return null;
    return projectToThreePointArc(point, controls.start, controls.mid, controls.end);
  }

  function lineLikeAllowsParam(t, kind) {
    if (kind === "line") return true;
    if (kind === "ray") return t >= -1e-9;
    return t >= -1e-9 && t <= 1 + 1e-9;
  }

  function lineLikeContainsPoint(start, end, kind, point) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lenSquared = dx * dx + dy * dy;
    if (lenSquared <= 1e-9) return false;
    const t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSquared;
    return lineLikeAllowsParam(t, kind);
  }

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

  /** @param {ViewerEnv | null} env */
  function sampleCoordinateTracePoints(env, binding) {
    if (!binding) return null;
    const evaluateExpr = modules.dynamics?.evaluateExpr;
    if (typeof evaluateExpr !== "function") return null;
    const point = env?.currentScene?.().points?.[binding.pointIndex];
    const pointBinding = point?.binding;
    const source = pointBinding?.sourceIndex !== undefined
      ? env.resolveScenePoint(pointBinding.sourceIndex)
      : null;
    if (!source || pointBinding?.kind !== "coordinate-source") return null;
    const parameters = env?.currentDynamics
      ? new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]))
      : new Map();
    const points = [];
    const last = Math.max(1, (binding.sampleCount || 0) - 1);
    for (let index = 0; index < (binding.sampleCount || 0); index += 1) {
      const t = index / last;
      const value = binding.xMin + (binding.xMax - binding.xMin) * t;
      const exprParameters = new Map(parameters);
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
    return points.length >= 2 ? points : null;
  }

  function resolveLineConstraint(env, constraint, resolveFn) {
    if (!constraint) return null;
    if (constraint.kind === "segment" || constraint.kind === "line" || constraint.kind === "ray") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !end) return null;
      return { start, end, kind: constraint.kind };
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
        ? {
            start: through,
            end: { x: through.x - dy / len, y: through.y + dx / len },
            kind: "line",
          }
        : {
            start: through,
            end: { x: through.x + dx / len, y: through.y + dy / len },
            kind: "line",
          };
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolveFn(constraint.startIndex);
      const vertex = resolveFn(constraint.vertexIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      if (!direction) return null;
      return {
        start: vertex,
        end: {
          x: vertex.x + direction.x,
          y: vertex.y + direction.y,
        },
        kind: "ray",
      };
    }
    return null;
  }

  function lineCircleIntersection(lineStart, lineEnd, lineKind, center, radiusPoint, variant) {
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
    const t = ts[Math.max(0, Math.min(ts.length - 1, variant || 0))];
    return {
      x: lineStart.x + dx * t,
      y: lineStart.y + dy * t,
    };
  }

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
    const along = (leftRadius * leftRadius - rightRadius * rightRadius + distance * distance)
      / (2 * distance);
    const heightSquared = leftRadius * leftRadius - along * along;
    if (heightSquared < -1e-9) return null;
    const height = Math.sqrt(Math.max(0, heightSquared));
    const ux = dx / distance;
    const uy = dy / distance;
    const base = {
      x: leftCenter.x + along * ux,
      y: leftCenter.y + along * uy,
    };
    const points = [
      { x: base.x - height * uy, y: base.y + height * ux },
      { x: base.x + height * uy, y: base.y - height * ux },
    ].sort((left, right) => (left.y - right.y) || (left.x - right.x));
    return points;
  }

  function circleCircleIntersection(leftCenter, leftRadiusPoint, rightCenter, rightRadiusPoint, variant) {
    const leftRadius = Math.hypot(leftRadiusPoint.x - leftCenter.x, leftRadiusPoint.y - leftCenter.y);
    const rightRadius = Math.hypot(rightRadiusPoint.x - rightCenter.x, rightRadiusPoint.y - rightCenter.y);
    const points = circleCircleIntersections(leftCenter, leftRadius, rightCenter, rightRadius);
    if (!points || points.length === 0) return null;
    return points[Math.max(0, Math.min(points.length - 1, variant || 0))];
  }

  /** @param {ViewerEnv} env */
  function getViewBounds(env) {
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

  /**
   * @param {ViewerEnv | null} env
   * @param {any} constraint
   * @param {(index: number) => Point} resolveFn
   */
  function resolveConstrainedPoint(env, constraint, resolveFn) {
    if (!constraint) return null;
    if (constraint.kind === "offset") {
      const origin = resolveFn(constraint.originIndex);
      if (!origin) return null;
      return { x: origin.x + constraint.dx, y: origin.y + constraint.dy };
    }
    if (constraint.kind === "segment") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !end) return null;
      return lerpPoint(start, end, constraint.t);
    }
    if (constraint.kind === "polyline") {
      const points = resolvePolylineConstraintPoints(env, constraint, resolveFn);
      if (!points) return null;
      const count = points.length;
      if (count < 2) return null;
      const segmentIndex = Math.max(0, Math.min(count - 2, constraint.segmentIndex));
      const start = points[segmentIndex];
      const end = points[segmentIndex + 1];
      if (!start || !end) return null;
      return lerpPoint(start, end, constraint.t);
    }
    if (constraint.kind === "polygon-boundary") {
      const count = constraint.vertexIndices.length;
      if (count < 2) return null;
      const start = resolveFn(constraint.vertexIndices[((constraint.edgeIndex % count) + count) % count]);
      const end = resolveFn(constraint.vertexIndices[(constraint.edgeIndex + 1 + count) % count]);
      if (!start || !end) return null;
      return lerpPoint(start, end, constraint.t);
    }
    if (constraint.kind === "circle") {
      const center = resolveFn(constraint.centerIndex);
      const radiusPoint = resolveFn(constraint.radiusIndex);
      if (!center || !radiusPoint) return null;
      const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
      return {
        x: center.x + radius * constraint.unitX,
        y: center.y + radius * constraint.unitY,
      };
    }
    if (constraint.kind === "circle-arc") {
      const center = resolveFn(constraint.centerIndex);
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      if (!center || !start || !end) return null;
      return pointOnCircleArc(center, start, end, constraint.t, !!env?.sourceScene?.yUp);
    }
    if (constraint.kind === "arc") {
      const start = resolveFn(constraint.startIndex);
      const mid = resolveFn(constraint.midIndex);
      const end = resolveFn(constraint.endIndex);
      if (!start || !mid || !end) return null;
      return pointOnThreePointArc(start, mid, end, constraint.t);
    }
    if (constraint.kind === "line-intersection") {
      const left = resolveLineConstraint(env, constraint.left, resolveFn);
      const right = resolveLineConstraint(env, constraint.right, resolveFn);
      if (!left || !right) return null;
      return lineLineIntersection(
        left.start,
        left.end,
        left.kind,
        right.start,
        right.end,
        right.kind,
      );
    }
    if (constraint.kind === "line-trace-intersection") {
      const line = resolveLineConstraint(env, constraint.line, resolveFn);
      const tracePoints = sampleCoordinateTracePoints(env, constraint);
      if (!line || !tracePoints) return null;
      return linePolylineIntersection(line.start, line.end, line.kind, tracePoints);
    }
    if (constraint.kind === "line-circle-intersection") {
      const line = resolveLineConstraint(env, constraint.line, resolveFn);
      const center = resolveFn(constraint.centerIndex);
      const radiusPoint = resolveFn(constraint.radiusIndex);
      if (!line || !center || !radiusPoint) return null;
      return lineCircleIntersection(
        line.start,
        line.end,
        line.kind,
        center,
        radiusPoint,
        constraint.variant,
      );
    }
    if (constraint.kind === "circle-circle-intersection") {
      const leftCenter = resolveFn(constraint.leftCenterIndex);
      const leftRadiusPoint = resolveFn(constraint.leftRadiusIndex);
      const rightCenter = resolveFn(constraint.rightCenterIndex);
      const rightRadiusPoint = resolveFn(constraint.rightRadiusIndex);
      if (!leftCenter || !leftRadiusPoint || !rightCenter || !rightRadiusPoint) return null;
      return circleCircleIntersection(
        leftCenter,
        leftRadiusPoint,
        rightCenter,
        rightRadiusPoint,
        constraint.variant,
      );
    }
    if (constraint.kind === "circular-intersection") {
      const left = circleFromConstraint(env, constraint.left, resolveFn);
      const right = circleFromConstraint(env, constraint.right, resolveFn);
      if (!left || !right) return null;
      const intersections = circleCircleIntersections(
        left.center,
        left.radius,
        right.center,
        right.radius,
      );
      if (!intersections || intersections.length === 0) return null;
      const onBoth = intersections.filter((point) =>
        pointLiesOnCircularConstraint(point, left)
        && pointLiesOnCircularConstraint(point, right)
      );
      if (onBoth.length === 0) return null;
      return onBoth[Math.min(constraint.variant || 0, onBoth.length - 1)] || null;
    }
    return null;
  }

  /** @param {ViewerEnv} env */
  function resolveScenePoint(env, index) {
    const point = env.currentScene().points[index];
    if (!point) return null;
    const resolved = resolveConstrainedPoint(env, point.constraint, (i) => resolveScenePoint(env, i));
    if (resolved) return resolved;
    return point.constraint ? null : point;
  }

  /** @param {ViewerEnv} env */
  function resolvePoint(env, handle) {
    if (typeof handle.pointIndex === "number") {
      const point = resolveScenePoint(env, handle.pointIndex);
      if (!point) return null;
      return {
        x: point.x + (handle.dx || 0),
        y: point.y + (handle.dy || 0),
      };
    }
    if (typeof handle.lineIndex === "number") {
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
    return handle;
  }

  /** @param {ViewerEnv} env */
  function resolveAnchorBase(env, handle) {
    if (typeof handle.pointIndex === "number") {
      return resolveScenePoint(env, handle.pointIndex);
    }
    if (typeof handle.lineIndex === "number") {
      const points = resolveLinePoints(env, handle.lineIndex);
      if (!points || points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      const start = points[segmentIndex];
      const end = points[segmentIndex + 1];
      return lerpPoint(start, end, t);
    }
    return handle;
  }

  /** @param {ViewerEnv} env */
  function resolveHostLinePoints(env, binding) {
    if (typeof binding?.lineIndex === "number") {
      return resolveLinePoints(env, binding.lineIndex);
    }
    if (
      typeof binding?.lineStartIndex === "number"
      && typeof binding?.lineEndIndex === "number"
    ) {
      const lineStart = resolveScenePoint(env, binding.lineStartIndex);
      const lineEnd = resolveScenePoint(env, binding.lineEndIndex);
      if (!lineStart || !lineEnd) return null;
      return [lineStart, lineEnd];
    }
    return null;
  }

  function resolveRightAngleMarkerPoints(vertex, first, second, shortestLen) {
    const side = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
    if (side <= 1e-9) return null;
    return [
      { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
      { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
      { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
    ];
  }

  function resolveArcAngleMarkerPoints(vertex, first, second, shortestLen, cross, dot, markerClass) {
    const classScale = 1 + 0.18 * Math.max(0, (markerClass || 1) - 1);
    const radius = Math.min(Math.max(shortestLen * 0.12, 10), 28) * classScale;
    const clampedRadius = Math.min(radius, shortestLen * 0.42);
    if (clampedRadius <= 1e-9) return null;
    const delta = Math.atan2(cross, dot);
    if (Math.abs(delta) <= 1e-6) return null;
    const startAngle = Math.atan2(first.y, first.x);
    const samples = 9;
    return Array.from({ length: samples }, (_, index) => {
      const t = index / (samples - 1);
      const angle = startAngle + delta * t;
      return {
        x: vertex.x + clampedRadius * Math.cos(angle),
        y: vertex.y + clampedRadius * Math.sin(angle),
      };
    });
  }

  function resolveAngleMarkerPoints(start, vertex, end, markerClass) {
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
    return resolveArcAngleMarkerPoints(vertex, first, second, shortestLen, cross, dot, markerClass);
  }

  /** @param {ViewerEnv} env */
  function resolveLinePoints(env, lineOrIndex) {
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
        {
          x: vertex.x + direction.x,
          y: vertex.y + direction.y,
        },
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
        {
          x: through.x - dy / len,
          y: through.y + dx / len,
        },
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
        {
          x: through.x + dx / len,
          y: through.y + dy / len,
        },
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
    if (line.binding?.kind === "arc-boundary") {
      return sampleArcBoundaryPoints(env, line.binding);
    }
    if (line.binding?.kind === "coordinate-trace") {
      return sampleCoordinateTracePoints(env, line.binding);
    }
    const points = line.points.map((handle) => resolvePoint(env, handle));
    return points.every(Boolean) ? points : null;
  }

  /** @param {ViewerEnv} env */
  function toScreen(env, point) {
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

  /** @param {ViewerEnv} env */
  function toWorld(env, screenX, screenY) {
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

  /** @param {ViewerEnv} env */
  function getCanvasCoords(env, event) {
    const rect = env.canvas.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left) * (env.sourceScene.width / rect.width),
      y: (event.clientY - rect.top) * (env.sourceScene.height / rect.height),
    };
  }

  function chooseGridStep(span, targetLines) {
    const rough = Math.max(1e-6, span / Math.max(1, targetLines));
    const magnitude = 10 ** Math.floor(Math.log10(rough));
    const normalized = rough / magnitude;
    if (normalized <= 1) return magnitude;
    if (normalized <= 2) return magnitude * 2;
    if (normalized <= 5) return magnitude * 5;
    return magnitude * 10;
  }

  function hasPolarPlot(env) {
    return !!env.currentDynamics().functions?.some((functionDef) => functionDef.domain?.plotMode === "polar");
  }

  function maxVisibleRadius(bounds) {
    return Math.max(
      Math.hypot(bounds.minX, bounds.minY),
      Math.hypot(bounds.minX, bounds.maxY),
      Math.hypot(bounds.maxX, bounds.minY),
      Math.hypot(bounds.maxX, bounds.maxY),
    );
  }

  function drawPolarGrid(env, bounds) {
    const origin = env.currentScene().origin
      ? resolvePoint(env, env.currentScene().origin)
      : { x: 0, y: 0 };
    const originScreen = toScreen(env, origin);
    const maxRadius = maxVisibleRadius(bounds);
    const radialMinorStep = chooseGridStep(maxRadius, 10);
    const radialMajorStep = chooseGridStep(maxRadius, 5);
    const circleCount = Math.ceil(maxRadius / radialMinorStep);
    const minorAngleStep = Math.PI / 12;
    const majorEvery = 2;

    env.ctx.save();
    env.ctx.lineWidth = 1;
    env.ctx.font = "12px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.fillStyle = "rgb(20,20,20)";

    for (let circleIndex = 1; circleIndex <= circleCount; circleIndex += 1) {
      const radius = circleIndex * radialMinorStep;
      const radiusScreen = Math.abs(toScreen(env, { x: origin.x + radius, y: origin.y }).x - originScreen.x);
      const major = Math.abs((radius / radialMajorStep) - Math.round(radius / radialMajorStep)) < 1e-6;
      env.ctx.strokeStyle = major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      env.ctx.beginPath();
      env.ctx.arc(originScreen.x, originScreen.y, radiusScreen, 0, Math.PI * 2);
      env.ctx.stroke();
      if (major) {
        const top = toScreen(env, { x: origin.x, y: origin.y + radius });
        const left = toScreen(env, { x: origin.x - radius, y: origin.y });
        const label = env.formatAxisNumber(radius);
        const width = env.ctx.measureText(label).width;
        env.ctx.fillText(label, top.x + 6, top.y - 4);
        env.ctx.fillText(label, left.x - width - 6, left.y - 4);
      }
    }

    const spokeCount = Math.round((Math.PI * 2) / minorAngleStep);
    for (let index = 0; index < spokeCount; index += 1) {
      const angle = index * minorAngleStep;
      const endpoint = {
        x: origin.x + maxRadius * Math.cos(angle),
        y: origin.y + maxRadius * Math.sin(angle),
      };
      const endScreen = toScreen(env, endpoint);
      const major = index % majorEvery === 0;
      env.ctx.strokeStyle = major ? "rgb(190,190,190)" : "rgb(225,225,225)";
      if (index % 6 === 0) {
        env.ctx.strokeStyle = "rgb(40,40,40)";
      }
      env.ctx.beginPath();
      env.ctx.moveTo(originScreen.x, originScreen.y);
      env.ctx.lineTo(endScreen.x, endScreen.y);
      env.ctx.stroke();
    }

    env.ctx.fillStyle = "rgba(255, 60, 40, 1)";
    env.ctx.beginPath();
    env.ctx.arc(originScreen.x, originScreen.y, 3, 0, Math.PI * 2);
    env.ctx.fill();
    env.ctx.restore();
  }

  /** @param {ViewerEnv} env */
  function drawGrid(env) {
    if (!env.currentScene().graphMode) return;
    const bounds = getViewBounds(env);
    if (hasPolarPlot(env)) {
      drawPolarGrid(env, bounds);
      return;
    }
    const spanY = bounds.maxY - bounds.minY;
    const yMinorStep = env.savedViewportMode ? 1 : chooseGridStep(spanY, 14);
    const yMajorStep = env.savedViewportMode ? 2 : chooseGridStep(spanY, 7);
    const minYIndex = Math.floor(bounds.minY / yMinorStep);
    const maxYIndex = Math.ceil(bounds.maxY / yMinorStep);

    env.ctx.save();
    env.ctx.lineWidth = 1;
    env.ctx.font = "12px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.fillStyle = "rgb(20,20,20)";
    const xAxisY = bounds.minY <= 0 && 0 <= bounds.maxY
      ? toScreen(env, { x: bounds.minX, y: 0 }).y
      : env.sourceScene.height - 18;
    const yAxisX = bounds.minX <= 0 && 0 <= bounds.maxX
      ? toScreen(env, { x: 0, y: bounds.minY }).x
      : env.sourceScene.width / 2;
    if (env.trigMode) {
      const xMinorStep = Math.PI / 2;
      const startIndex = Math.ceil(bounds.minX / xMinorStep);
      const endIndex = Math.floor(bounds.maxX / xMinorStep);
      for (let stepIndex = startIndex; stepIndex <= endIndex; stepIndex += 1) {
        const x = stepIndex * xMinorStep;
        const screen = toScreen(env, { x, y: bounds.minY });
        const major = stepIndex % 2 === 0;
        env.ctx.strokeStyle = Math.abs(x) < 1e-9
          ? "rgb(40,40,40)"
          : major ? "rgb(190,190,190)" : "rgb(220,220,220)";
        env.ctx.beginPath();
        env.ctx.moveTo(screen.x, 0);
        env.ctx.lineTo(screen.x, env.sourceScene.height);
        env.ctx.stroke();
        if (bounds.minY <= 0 && 0 <= bounds.maxY) {
          env.ctx.strokeStyle = "rgb(40,40,40)";
          env.ctx.beginPath();
          env.ctx.moveTo(screen.x, xAxisY - (major ? 6 : 4));
          env.ctx.lineTo(screen.x, xAxisY + (major ? 6 : 4));
          env.ctx.stroke();
        }
        if (major && stepIndex !== 0) {
          const label = env.formatPiLabel(stepIndex);
          const width = env.ctx.measureText(label).width;
          env.ctx.fillText(label, screen.x - width / 2, Math.min(env.sourceScene.height - 4, xAxisY + 16));
        }
      }
    } else {
      const spanX = bounds.maxX - bounds.minX;
      const xMinorStep = env.savedViewportMode ? 1 : chooseGridStep(spanX, 14);
      const xMajorStep = env.savedViewportMode ? 2 : chooseGridStep(spanX, 7);
      const minXIndex = Math.floor(bounds.minX / xMinorStep);
      const maxXIndex = Math.ceil(bounds.maxX / xMinorStep);
      for (let xIndex = minXIndex; xIndex <= maxXIndex; xIndex += 1) {
        const x = xIndex * xMinorStep;
        const screen = toScreen(env, { x, y: bounds.minY });
        const major = Math.abs((x / xMajorStep) - Math.round(x / xMajorStep)) < 1e-6;
        env.ctx.strokeStyle = Math.abs(x) < 1e-6
          ? "rgb(40,40,40)"
          : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
        env.ctx.beginPath();
        env.ctx.moveTo(screen.x, 0);
        env.ctx.lineTo(screen.x, env.sourceScene.height);
        env.ctx.stroke();
        if (bounds.minY <= 0 && 0 <= bounds.maxY) {
          env.ctx.strokeStyle = "rgb(40,40,40)";
          env.ctx.beginPath();
          env.ctx.moveTo(screen.x, xAxisY - (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2));
          env.ctx.lineTo(screen.x, xAxisY + (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2));
          env.ctx.stroke();
        }
        if (major && Math.abs(x) >= 1e-6) {
          const label = env.formatAxisNumber(x);
          const width = env.ctx.measureText(label).width;
          env.ctx.fillText(label, screen.x - width / 2, Math.min(env.sourceScene.height - 4, xAxisY + 16));
        }
      }
    }
    for (let yIndex = minYIndex; yIndex <= maxYIndex; yIndex += 1) {
      const y = yIndex * yMinorStep;
      const major = Math.abs((y / yMajorStep) - Math.round(y / yMajorStep)) < 1e-6;
      const screen = toScreen(env, { x: bounds.minX, y });
      env.ctx.strokeStyle = Math.abs(y) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      env.ctx.beginPath();
      env.ctx.moveTo(0, screen.y);
      env.ctx.lineTo(env.sourceScene.width, screen.y);
      env.ctx.stroke();
      if (bounds.minX <= 0 && 0 <= bounds.maxX) {
        env.ctx.strokeStyle = "rgb(40,40,40)";
        env.ctx.beginPath();
        env.ctx.moveTo(yAxisX - (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2), screen.y);
        env.ctx.lineTo(yAxisX + (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2), screen.y);
        env.ctx.stroke();
      }
      if (major && Math.abs(y) >= 1e-6) {
        const label = env.formatAxisNumber(y);
        const width = env.ctx.measureText(label).width;
        env.ctx.fillText(label, yAxisX - width - 8, screen.y - 6);
      }
    }
    if (env.currentScene().origin) {
      const origin = toScreen(env, resolvePoint(env, env.currentScene().origin));
      env.ctx.fillStyle = "rgba(255, 60, 40, 1)";
      env.ctx.beginPath();
      env.ctx.arc(origin.x, origin.y, 3, 0, Math.PI * 2);
      env.ctx.fill();
    }
    env.ctx.restore();
  }

  modules.scene = {
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
    pointOnCircleArc,
    projectToCircleArc,
    pointOnThreePointArc,
    projectToThreePointArc,
    sampleArcBoundaryPoints,
    sampleCoordinateTracePoints,
    lineLineIntersection,
    lineCircleIntersection,
    circleCircleIntersection,
    drawGrid,
  };
})();
