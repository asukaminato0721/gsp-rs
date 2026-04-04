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
    const ccwSpan = normalizeAngleDelta(startAngle, endAngle);
    const ccwMid = normalizeAngleDelta(startAngle, midAngle);
    const clampedT = Math.max(0, Math.min(1, t));
    const angle = ccwMid <= ccwSpan + 1e-9
      ? startAngle + ccwSpan * clampedT
      : startAngle - normalizeAngleDelta(endAngle, startAngle) * clampedT;
    return {
      x: center.x + radius * Math.cos(angle),
      y: center.y + radius * Math.sin(angle),
    };
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
      return { x: origin.x + constraint.dx, y: origin.y + constraint.dy };
    }
    if (constraint.kind === "segment") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      return lerpPoint(start, end, constraint.t);
    }
    if (constraint.kind === "polyline") {
      const count = constraint.points.length;
      if (count < 2) return null;
      const segmentIndex = Math.max(0, Math.min(count - 2, constraint.segmentIndex));
      const start = constraint.points[segmentIndex];
      const end = constraint.points[segmentIndex + 1];
      return lerpPoint(start, end, constraint.t);
    }
    if (constraint.kind === "polygon-boundary") {
      const count = constraint.vertexIndices.length;
      if (count < 2) return null;
      const start = resolveFn(constraint.vertexIndices[((constraint.edgeIndex % count) + count) % count]);
      const end = resolveFn(constraint.vertexIndices[(constraint.edgeIndex + 1 + count) % count]);
      return lerpPoint(start, end, constraint.t);
    }
    if (constraint.kind === "circle") {
      const center = resolveFn(constraint.centerIndex);
      const radiusPoint = resolveFn(constraint.radiusIndex);
      const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
      return {
        x: center.x + radius * constraint.unitX,
        y: center.y + radius * constraint.unitY,
      };
    }
    if (constraint.kind === "arc") {
      const start = resolveFn(constraint.startIndex);
      const mid = resolveFn(constraint.midIndex);
      const end = resolveFn(constraint.endIndex);
      return pointOnThreePointArc(start, mid, end, constraint.t);
    }
    return null;
  }

  /** @param {ViewerEnv} env */
  function resolveScenePoint(env, index) {
    const point = env.currentScene().points[index];
    if (!point) return { x: 0, y: 0 };
    const resolved = resolveConstrainedPoint(env, point.constraint, (i) => resolveScenePoint(env, i));
    return resolved || point;
  }

  /** @param {ViewerEnv} env */
  function resolvePoint(env, handle) {
    if (typeof handle.pointIndex === "number") {
      const point = resolveScenePoint(env, handle.pointIndex);
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
  function resolveLinePoints(env, lineOrIndex) {
    const line = typeof lineOrIndex === "number" ? env.currentScene().lines[lineOrIndex] : lineOrIndex;
    if (!line) return null;
    if (line.binding?.kind === "segment") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      return [start, end];
    }
    if (line.binding?.kind === "angle-bisector-ray") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const vertex = resolveScenePoint(env, line.binding.vertexIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
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
      const lineStart = resolveScenePoint(env, line.binding.lineStartIndex);
      const lineEnd = resolveScenePoint(env, line.binding.lineEndIndex);
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
      const lineStart = resolveScenePoint(env, line.binding.lineStartIndex);
      const lineEnd = resolveScenePoint(env, line.binding.lineEndIndex);
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
      return clipParametricLineToBounds(start, end, getViewBounds(env), false);
    }
    if (line.binding?.kind === "ray") {
      const start = resolveScenePoint(env, line.binding.startIndex);
      const end = resolveScenePoint(env, line.binding.endIndex);
      return clipParametricLineToBounds(start, end, getViewBounds(env), true);
    }
    return line.points.map((handle) => resolvePoint(env, handle));
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

  /** @param {ViewerEnv} env */
  function drawGrid(env) {
    if (!env.currentScene().graphMode) return;
    const bounds = getViewBounds(env);
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
      const xLabelStep = spanX > 20 ? 5 : 2;
      const minX = Math.floor(bounds.minX);
      const maxX = Math.ceil(bounds.maxX);
      for (let x = minX; x <= maxX; x += 1) {
        const screen = toScreen(env, { x, y: bounds.minY });
        env.ctx.strokeStyle = x === 0 ? "rgb(40,40,40)" : "rgb(200,200,200)";
        env.ctx.beginPath();
        env.ctx.moveTo(screen.x, 0);
        env.ctx.lineTo(screen.x, env.sourceScene.height);
        env.ctx.stroke();
        if (bounds.minY <= 0 && 0 <= bounds.maxY) {
          env.ctx.strokeStyle = "rgb(40,40,40)";
          env.ctx.beginPath();
          env.ctx.moveTo(screen.x, xAxisY - (x === 0 ? 6 : 4));
          env.ctx.lineTo(screen.x, xAxisY + (x === 0 ? 6 : 4));
          env.ctx.stroke();
        }
        if (x !== 0 && x % xLabelStep === 0) {
          const label = String(x);
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
    pointOnThreePointArc,
    projectToThreePointArc,
    drawGrid,
  };
})();
