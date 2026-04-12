// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { pointIndex: number }>}
   */
  function hasPointIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  /**
   * @param {LineBindingJson} binding
   * @returns {binding is Extract<LineBindingJson, { lineStartIndex: number | null; lineEndIndex: number | null }>}
   */
  function hasExplicitHostLine(binding) {
    return !!binding
      && typeof binding === "object"
      && "lineStartIndex" in binding
      && "lineEndIndex" in binding
      && typeof binding.lineStartIndex === "number"
      && typeof binding.lineEndIndex === "number";
  }

  /**
   * @param {LineBindingJson} binding
   * @returns {boolean}
   */
  function hasHostLineIndex(binding) {
    return !!binding
      && typeof binding === "object"
      && "lineIndex" in binding
      && typeof binding.lineIndex === "number";
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {number} width
   * @param {number} height
   * @param {boolean} rayOnly
   */
  function clipParametricLineToRect(start, end, width, height, rayOnly) {
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
        point.x < -1e-6 || point.x > width + 1e-6 ||
        point.y < -1e-6 || point.y > height + 1e-6
      ) return;
      if (hits.some((hit) =>
        Math.abs(hit.t - t) < 1e-6 ||
        (Math.abs(hit.point.x - point.x) < 1e-6 && Math.abs(hit.point.y - point.y) < 1e-6)
      )) return;
      hits.push({ t, point });
    };

    if (Math.abs(dx) > 1e-9) {
      for (const x of [0, width]) {
        const t = (x - start.x) / dx;
        pushHit(t, { x, y: start.y + dy * t });
      }
    }
    if (Math.abs(dy) > 1e-9) {
      for (const y of [0, height]) {
        const t = (y - start.y) / dy;
        pushHit(t, { x: start.x + dx * t, y });
      }
    }
    if (
      rayOnly &&
      start.x >= -1e-6 && start.x <= width + 1e-6 &&
      start.y >= -1e-6 && start.y <= height + 1e-6
    ) {
      pushHit(0, { ...start });
    }
    if (hits.length < 2) return null;
    hits.sort((a, b) => a.t - b.t);
    return [hits[0].point, hits[hits.length - 1].point];
  }

  /**
   * @param {ViewerEnv} env
   * @param {string} _text
   * @returns {{ lines: string[]; width: number; height: number }}
   */
  function labelMetrics(env, _text) {
    return { lines: [], width: 0, height: env ? 0 : 0 };
  }

  /**
   * @param {ViewerEnv} _env
   * @param {RuntimeLabelJson} _label
   * @returns {null}
   */
  function labelBounds(_env, _label) {
    return null;
  }

  /**
   * @param {ViewerEnv} _env
   * @param {RuntimeIterationTableJson} _table
   * @returns {null}
   */
  function iterationTableBounds(_env, _table) {
    return null;
  }

  /**
   * @param {ViewerEnv} _env
   * @param {RuntimeLabelJson} _label
   * @returns {Array<never>}
   */
  function labelHotspotRects(_env, _label) {
    return [];
  }

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   * @returns {number | null}
   */
  function findHitPoint(env, screenX, screenY) {
    let bestIndex = null;
    let bestDistanceSquared = env.pointHitRadius * env.pointHitRadius;
    env.currentScene().points.forEach((point, index) => {
      if (point.visible === false || point.draggable === false) {
        return;
      }
      const resolved = env.resolveScenePoint(index);
      if (!resolved) return;
      const screen = env.toScreen(resolved);
      const dx = screen.x - screenX;
      const dy = screen.y - screenY;
      const distanceSquared = dx * dx + dy * dy;
      if (distanceSquared <= bestDistanceSquared) {
        bestDistanceSquared = distanceSquared;
        bestIndex = index;
      }
    });
    return bestIndex;
  }

  /** @returns {null} */
  function findHitLabel() {
    return null;
  }

  /** @returns {null} */
  function findHitIterationTable() {
    return null;
  }

  /** @returns {null} */
  function findHitPolygon() {
    return null;
  }

  /** @param {ViewerEnv} _env */
  function drawImages(_env) {}

  /** @param {ViewerEnv} _env */
  function drawPolygons(_env) {}

  /** @param {ViewerEnv} env */
  function drawLines(env) {
    const resolveRightAngleMarkerPoints = (
      /** @type {Point} */ vertex,
      /** @type {Point} */ first,
      /** @type {Point} */ second,
      /** @type {number} */ shortestLen,
      /** @type {number} */ layerIndex,
    ) => {
      const sideBase = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
      const side = sideBase + layerIndex * 5;
      if (side <= 1e-9) return null;
      return [
        { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
        { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
        { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
      ];
    };
    const resolveArcAngleMarkerPoints = (
      /** @type {Point} */ vertex,
      /** @type {Point} */ first,
      /** @type {number} */ shortestLen,
      /** @type {number} */ cross,
      /** @type {number} */ dot,
      /** @type {number} */ layerIndex,
    ) => {
      const radius = Math.min(Math.max(shortestLen * 0.12, 10), 28) + layerIndex * 5;
      const clampedRadius = Math.min(radius, shortestLen * (0.42 + layerIndex * 0.06));
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
    };
    const drawPolyline = (
      /** @type {Point[]} */ worldPoints,
      /** @type {[number, number, number, number]} */ color,
      /** @type {boolean} */ dashed,
    ) => {
      const screenPoints = worldPoints.map((/** @type {Point} */ point) => env.toScreen(point));
      if (screenPoints.length < 2) return;
      env.ctx.beginPath();
      screenPoints.forEach((/** @type {Point & { scale: number }} */ screen, /** @type {number} */ index) => {
        if (index === 0) env.ctx.moveTo(screen.x, screen.y);
        else env.ctx.lineTo(screen.x, screen.y);
      });
      env.ctx.strokeStyle = env.rgba(color);
      env.ctx.lineWidth = 2;
      env.ctx.setLineDash(dashed ? [8, 8] : []);
      env.ctx.stroke();
    };
    const drawAngleMarker = (/** @type {RuntimeLineJson} */ line) => {
      const start = env.resolveScenePoint(line.binding.startIndex);
      const vertex = env.resolveScenePoint(line.binding.vertexIndex);
      const end = env.resolveScenePoint(line.binding.endIndex);
      if (!start || !vertex || !end) return;
      const firstDx = start.x - vertex.x;
      const firstDy = start.y - vertex.y;
      const secondDx = end.x - vertex.x;
      const secondDy = end.y - vertex.y;
      const firstLen = Math.hypot(firstDx, firstDy);
      const secondLen = Math.hypot(secondDx, secondDy);
      const shortestLen = Math.min(firstLen, secondLen);
      if (firstLen <= 1e-9 || secondLen <= 1e-9 || shortestLen <= 1e-9) return;
      const first = { x: firstDx / firstLen, y: firstDy / firstLen };
      const second = { x: secondDx / secondLen, y: secondDy / secondLen };
      const dot = Math.max(-1, Math.min(1, first.x * second.x + first.y * second.y));
      const cross = first.x * second.y - first.y * second.x;
      const layerCount = Math.max(1, line.binding.markerClass || 1);
      for (let layerIndex = 0; layerIndex < layerCount; layerIndex += 1) {
        const points = Math.abs(dot) <= 0.12
          ? resolveRightAngleMarkerPoints(vertex, first, second, shortestLen, layerIndex)
          : resolveArcAngleMarkerPoints(vertex, first, shortestLen, cross, dot, layerIndex);
        if (points) drawPolyline(points, line.color, line.dashed);
      }
    };
    const drawSegmentMarker = (/** @type {RuntimeLineJson} */ line) => {
      const start = env.resolveScenePoint(line.binding.startIndex);
      const end = env.resolveScenePoint(line.binding.endIndex);
      if (!start || !end) return;
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return;
      const tangent = { x: dx / len, y: dy / len };
      const normal = { x: -tangent.y, y: tangent.x };
      const centerT = Math.max(0, Math.min(1, line.binding.t));
      const center = { x: start.x + dx * centerT, y: start.y + dy * centerT };
      const halfLen = Math.min(Math.max(len * 0.06, 5), 10);
      const spacing = Math.min(Math.max(len * 0.05, 6), 11);
      const layerCount = Math.max(1, line.binding.markerClass || 1);
      const offsetBase = -(layerCount - 1) / 2;
      for (let layerIndex = 0; layerIndex < layerCount; layerIndex += 1) {
        const offset = (offsetBase + layerIndex) * spacing;
        const slashCenter = {
          x: center.x + tangent.x * offset,
          y: center.y + tangent.y * offset,
        };
        drawPolyline([
          { x: slashCenter.x - normal.x * halfLen, y: slashCenter.y - normal.y * halfLen },
          { x: slashCenter.x + normal.x * halfLen, y: slashCenter.y + normal.y * halfLen },
        ], line.color, line.dashed);
      }
    };
    for (const line of env.currentScene().lines) {
      if (line.visible === false) continue;
      if (line.binding?.kind === "graph-helper-line") continue;
      if (line.binding?.kind === "angle-marker") {
        drawAngleMarker(line);
        continue;
      }
      if (line.binding?.kind === "segment-marker") {
        drawSegmentMarker(line);
        continue;
      }
      let screenPoints = null;
      const resolveHostLinePoints = (/** @type {LineBindingJson} */ binding) => {
        if (hasExplicitHostLine(binding)) {
          return [env.resolveScenePoint(binding.lineStartIndex), env.resolveScenePoint(binding.lineEndIndex)];
        }
        if (hasHostLineIndex(binding)) {
          const indexedBinding = /** @type {{ lineIndex?: number }} */ (binding);
          if (typeof indexedBinding.lineIndex === "number") {
            return env.resolveLinePoints(indexedBinding.lineIndex);
          }
        }
        return null;
      };
      if (
        line.binding?.kind === "line"
        || line.binding?.kind === "ray"
        || line.binding?.kind === "angle-bisector-ray"
        || line.binding?.kind === "perpendicular-line"
        || line.binding?.kind === "parallel-line"
      ) {
        const start = line.binding.kind === "perpendicular-line" || line.binding.kind === "parallel-line"
          ? (() => {
              const through = env.resolveScenePoint(line.binding.throughIndex);
              return through ? env.toScreen(through) : null;
            })()
          : line.binding.kind === "angle-bisector-ray"
            ? (() => {
                const vertex = env.resolveScenePoint(line.binding.vertexIndex);
                return vertex ? env.toScreen(vertex) : null;
              })()
          : (() => {
              const startPoint = env.resolveScenePoint(line.binding.startIndex);
              return startPoint ? env.toScreen(startPoint) : null;
            })();
        const end = line.binding.kind === "perpendicular-line"
          ? (() => {
              const through = env.resolveScenePoint(line.binding.throughIndex);
              if (!through) return null;
              const hostLine = resolveHostLinePoints(line.binding);
              if (!hostLine) return null;
              const [lineStart, lineEnd] = hostLine;
              const dx = lineEnd.x - lineStart.x;
              const dy = lineEnd.y - lineStart.y;
              const len = Math.hypot(dx, dy);
              if (len <= 1e-9) return null;
              return env.toScreen({ x: through.x - dy / len, y: through.y + dx / len });
            })()
          : line.binding.kind === "parallel-line"
            ? (() => {
                const through = env.resolveScenePoint(line.binding.throughIndex);
                if (!through) return null;
                const hostLine = resolveHostLinePoints(line.binding);
                if (!hostLine) return null;
                const [lineStart, lineEnd] = hostLine;
                const dx = lineEnd.x - lineStart.x;
                const dy = lineEnd.y - lineStart.y;
                const len = Math.hypot(dx, dy);
                if (len <= 1e-9) return null;
                return env.toScreen({ x: through.x + dx / len, y: through.y + dy / len });
              })()
          : line.binding.kind === "angle-bisector-ray"
            ? (() => {
                const startPoint = env.resolveScenePoint(line.binding.startIndex);
                const vertex = env.resolveScenePoint(line.binding.vertexIndex);
                const endPoint = env.resolveScenePoint(line.binding.endIndex);
                if (!startPoint || !vertex || !endPoint) return null;
                const startDx = startPoint.x - vertex.x;
                const startDy = startPoint.y - vertex.y;
                const startLen = Math.hypot(startDx, startDy);
                const endDx = endPoint.x - vertex.x;
                const endDy = endPoint.y - vertex.y;
                const endLen = Math.hypot(endDx, endDy);
                if (startLen <= 1e-9 || endLen <= 1e-9) return null;
                const sumX = startDx / startLen + endDx / endLen;
                const sumY = startDy / startLen + endDy / endLen;
                const sumLen = Math.hypot(sumX, sumY);
                const direction = sumLen > 1e-9
                  ? { x: sumX / sumLen, y: sumY / sumLen }
                  : { x: -startDy / startLen, y: startDx / startLen };
                return env.toScreen({ x: vertex.x + direction.x, y: vertex.y + direction.y });
              })()
          : (() => {
              const endPoint = env.resolveScenePoint(line.binding.endIndex);
              return endPoint ? env.toScreen(endPoint) : null;
            })();
        if (!start || !end) continue;
        screenPoints = clipParametricLineToRect(
          start,
          end,
          env.sourceScene.width,
          env.sourceScene.height,
          line.binding.kind === "ray" || line.binding.kind === "angle-bisector-ray",
        );
      } else {
        const points = env.resolveLinePoints
          ? env.resolveLinePoints(line)
          : line.points.map((/** @type {PointHandle} */ handle) => env.resolvePoint(handle));
        if (points && points.length >= 2) {
          screenPoints = points.map((/** @type {Point} */ point) => env.toScreen(point));
        }
      }
      if (!screenPoints || screenPoints.length < 2) continue;
      env.ctx.beginPath();
      screenPoints.forEach((/** @type {Point & { scale: number }} */ screen, /** @type {number} */ index) => {
        if (index === 0) env.ctx.moveTo(screen.x, screen.y);
        else env.ctx.lineTo(screen.x, screen.y);
      });
      env.ctx.strokeStyle = env.rgba(line.color);
      env.ctx.lineWidth = 2;
      env.ctx.setLineDash(line.dashed ? [8, 8] : []);
      env.ctx.stroke();
    }
    env.ctx.setLineDash([]);
  }

  /** @param {ViewerEnv} _env */
  function drawCircles(_env) {}

  /** @param {ViewerEnv} _env */
  function drawArcs(_env) {}

  /** @param {ViewerEnv} env */
  function drawPoints(env) {
    env.currentScene().points.forEach((point, index) => {
      if (point.visible === false) {
        return;
      }
      const resolved = env.resolveScenePoint(index);
      if (!resolved) return;
      const screen = env.toScreen(resolved);
      env.ctx.beginPath();
      env.ctx.arc(screen.x, screen.y, index === env.hoverPointIndex.val ? 6 : 4, 0, Math.PI * 2);
      env.ctx.fillStyle = index === env.hoverPointIndex.val
        ? "rgba(255, 120, 20, 1)"
        : env.rgba(point.color || [255, 60, 40, 255]);
      env.ctx.fill();
    });
  }

  /** @param {ViewerEnv} _env */
  function drawLabels(_env) {}

  /** @param {ViewerEnv} _env */
  function drawIterationTables(_env) {}

  /** @param {ViewerEnv} _env */
  function drawHotspotFlashes(_env) {}

  /** @param {ViewerEnv} env */
  function draw(env) {
    env.ctx.clearRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.ctx.fillStyle = "rgb(250,250,248)";
    env.ctx.fillRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.drawGrid();
    modules.render.drawImages(env);
    modules.render.drawPolygons(env);
    drawLines(env);
    modules.render.drawCircles(env);
    modules.render.drawArcs(env);
    drawPoints(env);
    modules.render.drawLabels(env);
    modules.render.drawIterationTables(env);
    modules.render.drawHotspotFlashes(env);
  }

  modules.render = {
    labelMetrics,
    labelBounds,
    iterationTableBounds,
    labelHotspotRects,
    findHitPoint,
    findHitLabel,
    findHitIterationTable,
    findHitPolygon,
    drawImages,
    drawPolygons,
    drawLines,
    drawCircles,
    drawArcs,
    drawPoints,
    drawLabels,
    drawIterationTables,
    drawHotspotFlashes,
    draw,
  };
})();
