// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  /** @type {Record<string, (env: ViewerEnv | null, constraint: RuntimePointConstraintJson, resolveFn: (index: number) => Point | null, reference?: RuntimeScenePointJson | Point | null) => Point | null>} */
  const extraPointConstraintResolvers = {};
  /** @type {Record<string, (env: ViewerEnv, line: RuntimeLineJson) => Point[] | null>} */
  const extraLineBindingResolvers = {};

  /**
   * @param {string} kind
   * @param {(env: ViewerEnv | null, constraint: RuntimePointConstraintJson, resolveFn: (index: number) => Point | null, reference?: RuntimeScenePointJson | Point | null) => Point | null} resolver
   */
  function registerPointConstraintResolver(kind, resolver) {
    extraPointConstraintResolvers[kind] = resolver;
  }

  /**
   * @param {string} kind
   * @param {(env: ViewerEnv, line: RuntimeLineJson) => Point[] | null} resolver
   */
  function registerLineBindingResolver(kind, resolver) {
    extraLineBindingResolvers[kind] = resolver;
  }

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { pointIndex: number }>}
   */
  function hasPointIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { lineIndex: number }>}
   */
  function hasLineIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "lineIndex" in handle && typeof handle.lineIndex === "number";
  }

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
   * @param {Point} end
   * @param {number} t
   */
  function lerpPoint(start, end, t) {
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }

  /**
   * @param {Point} point
   * @param {Point} start
   * @param {Point} end
   */
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

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {{ minX: number; maxX: number; minY: number; maxY: number }} bounds
   * @param {boolean} rayOnly
   * @returns {Point[] | null}
   */
  function clipParametricLineToBounds(start, end, bounds, rayOnly) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return null;

    /** @type {Array<{ t: number; point: Point }>} */
    const hits = [];
    /**
     * @param {number} t
     * @param {Point} point
     */
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

  /**
   * @param {Point} start
   * @param {Point} vertex
   * @param {Point} end
   * @returns {Point | null}
   */
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
   * @param {RuntimePointConstraintJson | CircularConstraintJson | LineConstraintJson | null} constraint
   * @param {(index: number) => Point | null} resolveFn
   * @param {RuntimeScenePointJson | Point | null | undefined} reference
   * @returns {Point | null}
   */
  function resolveConstrainedPoint(env, constraint, resolveFn, reference) {
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
    if (constraint.kind === "polygon-boundary") {
      const count = constraint.vertexIndices.length;
      if (count < 2) return null;
      const start = resolveFn(constraint.vertexIndices[((constraint.edgeIndex % count) + count) % count]);
      const end = resolveFn(constraint.vertexIndices[(constraint.edgeIndex + 1 + count) % count]);
      return start && end ? lerpPoint(start, end, constraint.t) : null;
    }
    const extra = extraPointConstraintResolvers[constraint.kind];
    return extra ? extra(env, constraint, resolveFn, reference) : null;
  }

  /**
   * @param {ViewerEnv} env
   * @param {number} index
   * @returns {Point | null}
   */
  function resolveScenePoint(env, index) {
    const point = env.currentScene().points[index];
    if (!point) return null;
    if (!point.constraint) return point;
    const resolved = resolveConstrainedPoint(env, point.constraint, (i) => resolveScenePoint(env, i), point);
    if (resolved) return resolved;
    return null;
  }

  /**
   * @param {ViewerEnv} env
   * @param {PointHandle} handle
   * @returns {Point | null}
   */
  function resolvePoint(env, handle) {
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
    return /** @type {Point} */ (handle);
  }

  /**
   * @param {ViewerEnv} env
   * @param {PointHandle} handle
   * @returns {Point | null}
   */
  function resolveAnchorBase(env, handle) {
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
    return /** @type {Point} */ (handle);
  }

  /**
   * @param {ViewerEnv} env
   * @param {LineBindingJson} binding
   * @returns {Point[] | null}
   */
  function resolveHostLinePoints(env, binding) {
    const hostBinding = /** @type {{ lineStartIndex?: number; lineEndIndex?: number; lineIndex?: number }} */ (binding);
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

  /**
   * @param {Point} vertex
   * @param {Point} first
   * @param {Point} second
   * @param {number} shortestLen
   */
  function resolveRightAngleMarkerPoints(vertex, first, second, shortestLen) {
    const side = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
    if (side <= 1e-9) return null;
    return [
      { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
      { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
      { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
    ];
  }

  /**
   * @param {Point} vertex
   * @param {Point} first
   * @param {Point} second
   * @param {number} shortestLen
   * @param {number} cross
   * @param {number} dot
   * @param {number} markerClass
   */
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

  /**
   * @param {Point} start
   * @param {Point} vertex
   * @param {Point} end
   * @param {number} markerClass
   */
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

  /**
   * @param {ViewerEnv} env
   * @param {SceneLineJson | number | null | undefined} lineOrIndex
   * @returns {Point[] | null}
   */
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
      return extra ? extra(env, line) : null;
    }
    const points = line.points.map((/** @type {PointHandle} */ handle) => resolvePoint(env, handle));
    return points.every(Boolean) ? points : null;
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} point
   */
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

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   */
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

  /**
   * @param {ViewerEnv} env
   * @param {MouseEvent | PointerEvent | WheelEvent} event
   */
  function getCanvasCoords(env, event) {
    const rect = env.canvas.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left) * (env.sourceScene.width / rect.width),
      y: (event.clientY - rect.top) * (env.sourceScene.height / rect.height),
    };
  }

  /**
   * @param {number} span
   * @param {number} targetLines
   */
  function chooseGridStep(span, targetLines) {
    const rough = Math.max(1e-6, span / Math.max(1, targetLines));
    const magnitude = 10 ** Math.floor(Math.log10(rough));
    const normalized = rough / magnitude;
    if (normalized <= 1) return magnitude;
    if (normalized <= 2) return magnitude * 2;
    if (normalized <= 5) return magnitude * 5;
    return magnitude * 10;
  }

  /** @param {ViewerEnv} env */
  function drawGrid(env) {
    if (!env.currentScene().graphMode) return;
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

  /** @returns {null} */
  function pointOnCircleArc() {
    return null;
  }

  /** @returns {null} */
  function projectToCircleArc() {
    return null;
  }

  /** @returns {null} */
  function pointOnThreePointArc() {
    return null;
  }

  /** @returns {null} */
  function projectToThreePointArc() {
    return null;
  }

  /** @returns {null} */
  function sampleArcBoundaryPoints() {
    return null;
  }

  /** @returns {null} */
  function sampleCoordinateTracePoints() {
    return null;
  }

  /** @returns {null} */
  function lineLineIntersection() {
    return null;
  }

  /** @returns {null} */
  function lineCircleIntersection() {
    return null;
  }

  /** @returns {null} */
  function circleCircleIntersection() {
    return null;
  }

  modules.scene = {
    registerPointConstraintResolver,
    registerLineBindingResolver,
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
