// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const geometry = modules.geometry;
  const {
    normalizeAngleDelta,
    lerpPoint,
    rotateAround,
    scaleAround: scalePointAround,
    reflectAcrossLine: reflectPointAcrossLine,
    clipParametricLineToBounds,
    clipLineToBounds,
    clipRayToBounds,
    angleBisectorDirection,
  } = geometry;
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
   * @param {Point} point
   * @param {Point} start
   * @param {Point} end
   */
  function projectToSegment(point, start, end) {
    return projectToLineLike(point, start, end, "segment");
  }

  /**
   * @param {Point} point
   * @param {Point} start
   * @param {Point} end
   * @param {"segment" | "line" | "ray"} kind
   */
  function projectToLineLike(point, start, end, kind) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lengthSquared = dx * dx + dy * dy;
    if (lengthSquared <= 1e-9) {
      return null;
    }
    const rawT = ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared;
    const t = kind === "line"
      ? rawT
      : kind === "ray"
        ? Math.max(0, rawT)
        : Math.max(0, Math.min(1, rawT));
    const projected = lerpPoint(start, end, t);
    return {
      t,
      projected,
      distanceSquared: (point.x - projected.x) ** 2 + (point.y - projected.y) ** 2,
    };
  }

  /**
   * @param {ViewerEnv | null} env
   * @param {{ lineStartIndex?: number, lineEndIndex?: number, lineIndex?: number }} constraint
   * @param {(index: number) => Point | null} resolveFn
   * @returns {[Point | null, Point | null]}
   */
  function reflectionAxisPoints(env, constraint, resolveFn) {
    const scene = typeof env?.currentScene === "function"
      ? env.currentScene()
      : env?.sourceScene || null;
    /** @type {(binding: any) => [Point, Point] | null} */
    const resolveFromBinding = (binding) => {
      if (!binding) return null;
      if (typeof binding.lineStartIndex === "number" && typeof binding.lineEndIndex === "number") {
        const lineStart = resolveFn(binding.lineStartIndex);
        const lineEnd = resolveFn(binding.lineEndIndex);
        return lineStart && lineEnd ? [lineStart, lineEnd] : null;
      }
      if (typeof binding.lineIndex === "number") {
        const line = scene?.lines?.[binding.lineIndex];
        return line ? resolveFromLine(line) : null;
      }
      return null;
    };
    /** @type {(line: any) => [Point, Point] | null} */
    const resolveFromLine = (line) => {
      if (!line) return null;
      if (line.points?.length >= 2 && !line.binding) {
        return [line.points[0], line.points[line.points.length - 1]];
      }
      if (line.binding?.kind === "segment") {
        const start = resolveFn(line.binding.startIndex);
        const end = resolveFn(line.binding.endIndex);
        return start && end ? [start, end] : null;
      }
      if (line.binding?.kind === "perpendicular-line" || line.binding?.kind === "parallel-line") {
        const through = resolveFn(line.binding.throughIndex);
        const hostLine = resolveFromBinding(line.binding);
        if (!through || !hostLine) return null;
        const [lineStart, lineEnd] = hostLine;
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        if (len <= 1e-9) return null;
        return line.binding.kind === "perpendicular-line"
          ? [through, { x: through.x - dy / len, y: through.y + dx / len }]
          : [through, { x: through.x + dx / len, y: through.y + dy / len }];
      }
      if (line.binding?.kind === "angle-bisector-ray") {
        const start = resolveFn(line.binding.startIndex);
        const vertex = resolveFn(line.binding.vertexIndex);
        const end = resolveFn(line.binding.endIndex);
        if (!start || !vertex || !end) return null;
        const direction = angleBisectorDirection(start, vertex, end);
        return direction ? [vertex, { x: vertex.x + direction.x, y: vertex.y + direction.y }] : null;
      }
      if (line.points?.length >= 2) {
        return [line.points[0], line.points[line.points.length - 1]];
      }
      return null;
    };
    if (typeof constraint.lineIndex === "number") {
      const line = scene?.lines?.[constraint.lineIndex];
      const resolved = resolveFromLine(line);
      if (resolved) return resolved;
    }
    const lineStart = typeof constraint.lineStartIndex === "number"
      ? resolveFn(constraint.lineStartIndex)
      : null;
    const lineEnd = typeof constraint.lineEndIndex === "number"
      ? resolveFn(constraint.lineEndIndex)
      : null;
    return [lineStart, lineEnd];
  }

  /**
   * @param {ViewerEnv | null} env
   * @param {CircularConstraintJson | null} constraint
   * @param {(index: number) => Point | null} resolveFn
   * @returns {{ kind: "circle"; center: Point; radius: number } | null}
   */
  function circleFromConstraint(env, constraint, resolveFn) {
    if (!constraint) return null;
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
    if (constraint.kind === "derived" && constraint.transform.kind === "translate-delta") {
      const source = circleFromConstraint(env, constraint.source, resolveFn);
      if (!source) return null;
      return {
        kind: "circle",
        center: {
          x: source.center.x + constraint.transform.dx,
          y: source.center.y + constraint.transform.dy,
        },
        radius: source.radius,
      };
    }
    if (constraint.kind === "derived" && constraint.transform.kind === "reflect") {
      const source = circleFromConstraint(env, constraint.source, resolveFn);
      const [lineStart, lineEnd] = reflectionAxisPoints(env, constraint.transform, resolveFn);
      if (!source || !lineStart || !lineEnd) return null;
      const reflectedCenter = reflectPointAcrossLine(source.center, lineStart, lineEnd);
      if (!reflectedCenter) return null;
      return {
        kind: "circle",
        center: reflectedCenter,
        radius: source.radius,
      };
    }
    if (constraint.kind === "derived" && constraint.transform.kind === "scale") {
      const source = circleFromConstraint(env, constraint.source, resolveFn);
      const center = resolveFn(constraint.transform.centerIndex);
      if (!source || !center) return null;
      return {
        kind: "circle",
        center: scalePointAround(source.center, center, constraint.transform.factor),
        radius: source.radius * Math.abs(constraint.transform.factor),
      };
    }
    return null;
  }

  /**
   * @param {Point} _point
   * @param {{ kind: string } | null} constraint
   */
  function pointLiesOnCircularConstraint(_point, constraint) {
    return !!constraint && constraint.kind === "circle";
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
    if (constraint.kind === "circular-constraint") {
      const circle = circleFromConstraint(env, constraint.circle, resolveFn);
      if (!circle) return null;
      return {
        x: circle.center.x + circle.radius * constraint.unitX,
        y: circle.center.y - circle.radius * constraint.unitY,
      };
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
      if (extra) {
        const resolved = extra(env, line);
        if (resolved) {
          return resolved;
        }
      }
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
   * @param {ViewerEnv} env
   * @param {Element} parent
   * @param {Record<string, string | number | boolean | null | undefined>} attrs
   */
  function appendGridElement(env, parent, attrs) {
    const tag = /** @type {string} */ (attrs.tag);
    const nextAttrs = { ...attrs };
    delete nextAttrs.tag;
    const element = env.createSvgElement(tag, nextAttrs);
    parent.append(element);
    return element;
  }

  /**
   * @param {ViewerEnv} env
   * @param {Element} parent
   * @param {number} x1
   * @param {number} y1
   * @param {number} x2
   * @param {number} y2
   * @param {string} color
   */
  function appendGridLine(env, parent, x1, y1, x2, y2, color) {
    appendGridElement(env, parent, {
      tag: "line",
      x1,
      y1,
      x2,
      y2,
      stroke: color,
      "stroke-width": 1,
      "shape-rendering": "crispEdges",
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {Element} parent
   * @param {number} x
   * @param {number} y
   * @param {string} text
   * @param {"start" | "middle" | "end"} anchor
   */
  function appendGridText(env, parent, x, y, text, anchor) {
    const label = appendGridElement(env, parent, {
      tag: "text",
      x,
      y,
      fill: "rgb(20,20,20)",
      "font-size": 12,
      "font-family": "\"Noto Sans\", \"Segoe UI\", sans-serif",
      "text-anchor": anchor,
      "dominant-baseline": "middle",
    });
    label.textContent = text;
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
    env.clearSvgChildren(env.gridLayer);
    if (!env.currentScene().graphMode) return;
    const gridLayer = env.gridLayer;
    const snapStroke = (/** @type {number} */ value) => Math.round(value) + 0.5;
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
      const stroke = Math.abs(x) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      appendGridLine(
        env,
        gridLayer,
        snapStroke(screen.x),
        0,
        snapStroke(screen.x),
        env.sourceScene.height,
        stroke,
      );
      if (bounds.minY <= 0 && 0 <= bounds.maxY) {
        appendGridLine(
          env,
          gridLayer,
          snapStroke(screen.x),
          snapStroke(xAxisY - (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.x),
          snapStroke(xAxisY + (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2)),
          "rgb(40,40,40)",
        );
      }
      if (major && Math.abs(x) >= 1e-6) {
        const label = env.formatAxisNumber(x);
        appendGridText(
          env,
          gridLayer,
          Math.round(screen.x),
          Math.round(Math.min(env.sourceScene.height - 4, xAxisY + 16)),
          label,
          "middle",
        );
      }
    }

    for (let yIndex = minYIndex; yIndex <= maxYIndex; yIndex += 1) {
      const y = yIndex * yMinorStep;
      const major = Math.abs((y / yMajorStep) - Math.round(y / yMajorStep)) < 1e-6;
      const screen = toScreen(env, { x: bounds.minX, y });
      const stroke = Math.abs(y) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      appendGridLine(
        env,
        gridLayer,
        0,
        snapStroke(screen.y),
        env.sourceScene.width,
        snapStroke(screen.y),
        stroke,
      );
      if (bounds.minX <= 0 && 0 <= bounds.maxX) {
        appendGridLine(
          env,
          gridLayer,
          snapStroke(yAxisX - (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.y),
          snapStroke(yAxisX + (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.y),
          "rgb(40,40,40)",
        );
      }
      if (major && Math.abs(y) >= 1e-6) {
        const label = env.formatAxisNumber(y);
        appendGridText(
          env,
          gridLayer,
          Math.round(yAxisX - env.measureText(label, 12) - 8),
          Math.round(screen.y - 6),
          label,
          "start",
        );
      }
    }

    if (env.currentScene().origin) {
      const origin = toScreen(env, resolvePoint(env, env.currentScene().origin));
      appendGridElement(env, gridLayer, {
        tag: "circle",
        cx: origin.x,
        cy: origin.y,
        r: 3,
        fill: "rgba(255, 60, 40, 1)",
      });
    }
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
    _circleFromConstraint: circleFromConstraint,
    _pointLiesOnCircularConstraint: pointLiesOnCircularConstraint,
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
    projectToLineLike,
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
