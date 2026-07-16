(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  );

  
  function pathFromPoints(points: Point[], close: boolean = false) {
    if (!points || points.length === 0) return "";
    const commands = points.map((point, index: number) => `${index === 0 ? "M" : "L"} ${point.x} ${point.y}`);
    if (close) {
      commands.push("Z");
    }
    return commands.join(" ");
  }

  
  function arcPath(center: Point, radius: number, startAngle: number, endAngle: number, counterClockwise: boolean) {
    if (!Number.isFinite(radius) || radius <= 1e-9) return "";
    const tau = Math.PI * 2;
    const start = {
      x: center.x + radius * Math.cos(startAngle),
      y: center.y + radius * Math.sin(startAngle),
    };
    const end = {
      x: center.x + radius * Math.cos(endAngle),
      y: center.y + radius * Math.sin(endAngle),
    };
    const forwardDelta = ((endAngle - startAngle) % tau + tau) % tau;
    const delta = counterClockwise ? (tau - forwardDelta) % tau : forwardDelta;
    const largeArc = delta > Math.PI ? 1 : 0;
    const sweep = counterClockwise ? 0 : 1;
    return `M ${start.x} ${start.y} A ${radius} ${radius} 0 ${largeArc} ${sweep} ${end.x} ${end.y}`;
  }

  
  function appendSceneElement(env: ViewerEnv, tag: string, attrs: Record<string, string | number | boolean | null | undefined>, text: string | null = null, debugTarget: DebugTarget | null = null) {
    const element = env.createSvgElement(tag, attrs);
    if (text !== null) {
      element.textContent = text;
    }
    env.registerDebugElement?.(element, debugTarget);
    env.sceneLayer.append(element);
    return element;
  }

  
  function appendPointPath(env: ViewerEnv, points: Point[], options: { stroke: string, strokeWidth?: number, fill?: string, dashed?: boolean, close?: boolean, lineCap?: string, lineJoin?: string, debugTarget?: DebugTarget | null }) {
    if (!points || points.length < 2) return null;
    return appendSceneElement(env, "path", {
      d: pathFromPoints(points, !!options.close),
      fill: options.fill ?? "none",
      stroke: options.stroke,
      "stroke-width": options.strokeWidth ?? 1,
      "stroke-dasharray": options.dashed ? "8 8" : null,
      "stroke-linecap": options.lineCap ?? "round",
      "stroke-linejoin": options.lineJoin ?? "round",
    }, null, options.debugTarget ?? null);
  }

  
  function clipParametricLineToRect(start: Point, end: Point, width: number, height: number, rayOnly: boolean) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return null;

    
    const hits = [];
    
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
    const firstHit = hits[0];
    const lastHit = hits[hits.length - 1];
    return firstHit && lastHit ? [firstHit.point, lastHit.point] : null;
  }

  
  function findHitPoint(env: ViewerEnv, screenX: number, screenY: number) {
    let bestIndex = null;
    let bestDistanceSquared = env.pointHitRadius * env.pointHitRadius;
    env.currentScene().points.forEach((point, index: number) => {
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

  
  function drawLines(env: ViewerEnv) {
    const drawPolyline = (
       worldPoints,
       color,
       dashed,
       strokeWidth,
       close= false,
       debugTarget= null,
    ) => {
      const screenPoints = worldPoints.map(( point) => env.toScreen(point));
      if (screenPoints.length < 2) return;
      appendPointPath(env, screenPoints, {
        stroke: env.rgba(color),
        strokeWidth,
        dashed,
        close,
        debugTarget,
      });
    };
    const drawAngleMarker = ( line,  lineIndex: number) => {
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
          ? (() => {
              const sideBase = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
              const side = sideBase + layerIndex * 5;
              if (side <= 1e-9) return null;
              return [
                { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
                { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
                { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
              ];
            })()
          : null;
        if (points?.length) {
          drawPolyline(points, line.color, line.dashed, line.strokeWidth, false, { category: "lines", index: lineIndex });
          continue;
        }
        const radius = Math.min(Math.max(shortestLen * 0.12, 10), 28) + layerIndex * 5;
        const clampedRadius = Math.min(radius, shortestLen * (0.42 + layerIndex * 0.06));
        if (clampedRadius <= 1e-9) continue;
        const delta = Math.atan2(cross, dot);
        if (Math.abs(delta) <= 1e-6) continue;
        const startAngle = Math.atan2(first.y, first.x);
        const samples = 9;
        const polyline = Array.from({ length: samples }, (_, index: number) => {
          const t = index / (samples - 1);
          const angle = startAngle + delta * t;
          return {
            x: vertex.x + clampedRadius * Math.cos(angle),
            y: vertex.y + clampedRadius * Math.sin(angle),
          };
        });
        drawPolyline(polyline, line.color, line.dashed, line.strokeWidth, false, { category: "lines", index: lineIndex });
      }
    };
    const drawSegmentMarker = ( line,  lineIndex: number) => {
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
        ], line.color, line.dashed, line.strokeWidth, false, { category: "lines", index: lineIndex });
      }
    };
    const pointsEqual = ( left,  right) =>
      Math.abs(left.x - right.x) < 1e-6 && Math.abs(left.y - right.y) < 1e-6;
    const extendedRayStart = ( startPoint,  endPoint) => {
      const dx = endPoint.x - startPoint.x;
      const dy = endPoint.y - startPoint.y;
      const lenSq = dx * dx + dy * dy;
      if (lenSq <= 1e-9) return startPoint;
      let bestPoint = startPoint;
      let bestT = 0;
      for (const candidate of env.currentScene().lines) {
        if (candidate.visible === false || candidate.binding?.kind !== "segment") continue;
        const a = env.resolveScenePoint(candidate.binding.startIndex);
        const b = env.resolveScenePoint(candidate.binding.endIndex);
        if (!a || !b) continue;
        let other = null;
        if (pointsEqual(a, startPoint)) other = b;
        else if (pointsEqual(b, startPoint)) other = a;
        if (!other) continue;
        const cross = (other.x - startPoint.x) * dy - (other.y - startPoint.y) * dx;
        if (Math.abs(cross) > 1e-6) continue;
        const t = ((other.x - startPoint.x) * dx + (other.y - startPoint.y) * dy) / lenSq;
        if (t > bestT + 1e-9) {
          bestT = t;
          bestPoint = other;
        }
      }
      return bestPoint;
    };
    const extendedRayEnd = (
       originalStart,
       originalEnd,
       shiftedStart,
    ) => ({
      x: shiftedStart.x + (originalEnd.x - originalStart.x),
      y: shiftedStart.y + (originalEnd.y - originalStart.y),
    });
    const linePriority = ( line) => (
      line.binding?.kind === "line"
        || line.binding?.kind === "ray"
        || line.binding?.kind === "angle-bisector-ray"
    ) ? 0 : 1;
    const orderedLines = env.currentScene().lines
      .map((line, index: number) => ({ line, index }))
      .sort((left, right) => linePriority(left.line) - linePriority(right.line) || left.index - right.index)
    for (const { line, index } of orderedLines) {
      if (line.visible === false) continue;
      if (line.binding?.kind === "graph-helper-line") continue;
      if (line.binding?.kind === "angle-marker") {
        drawAngleMarker(line, index);
        continue;
      }
      if (line.binding?.kind === "segment-marker") {
        drawSegmentMarker(line, index);
        continue;
      }
      let screenPoints = null;
      
      if (
        line.binding?.kind === "line"
        || line.binding?.kind === "ray"
        || line.binding?.kind === "angle-bisector-ray"
      ) {
        const start = line.binding.kind === "angle-bisector-ray"
            ? (() => {
                const vertex = env.resolveScenePoint(line.binding.vertexIndex);
                return vertex ? env.toScreen(vertex) : null;
              })()
            : (() => {
                const startPoint = env.resolveScenePoint(line.binding.startIndex);
                if (!startPoint) return null;
                if (line.binding.kind === "ray") {
                  const endPoint = env.resolveScenePoint(line.binding.endIndex);
                  if (!endPoint) return null;
                  const shiftedStart = extendedRayStart(startPoint, endPoint);
                  return env.toScreen(shiftedStart);
                }
                return env.toScreen(startPoint);
              })();
        const end = line.binding.kind === "angle-bisector-ray"
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
                  if (line.binding.kind === "ray") {
                    const startPoint = env.resolveScenePoint(line.binding.startIndex);
                    if (!startPoint || !endPoint) return null;
                    const shiftedStart = extendedRayStart(startPoint, endPoint);
                    return env.toScreen(extendedRayEnd(startPoint, endPoint, shiftedStart));
                  }
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
        const segments = Array.isArray(line.segments) ? line.segments : null;
        if (segments) {
          for (const segment of segments) {
            if (!segment || segment.length < 2) continue;
            appendPointPath(env, segment.map(( point) => env.toScreen(point)), {
              stroke: env.rgba(line.color),
              strokeWidth: line.strokeWidth,
              dashed: !!line.dashed,
              debugTarget: { category: "lines", index },
            });
          }
          continue;
        } else {
          const points = env.resolveLinePoints
            ? env.resolveLinePoints(line)
            : line.points.map(( handle) => env.resolvePoint(handle));
          if (points && points.length >= 2) {
            screenPoints = points.map(( point) => env.toScreen(point));
          }
        }
      }
      if (!screenPoints || screenPoints.length < 2) continue;
      appendPointPath(env, screenPoints, {
        stroke: env.rgba(line.color),
        strokeWidth: line.strokeWidth,
        dashed: !!line.dashed,
        debugTarget: { category: "lines", index },
      });
    }
  }

  
  function drawPoints(env: ViewerEnv) {
    env.currentScene().points.forEach((point, index: number) => {
      if (point.visible === false) {
        return;
      }
      const resolved = env.resolveScenePoint(index);
      if (!resolved) return;
      const screen = env.toScreen(resolved);
      appendSceneElement(env, "circle", {
        cx: screen.x,
        cy: screen.y,
        r: index === env.hoverPointIndex.val ? 6 : 4,
        fill: index === env.hoverPointIndex.val
          ? "rgba(255, 120, 20, 1)"
          : env.rgba(point.color || [255, 60, 40, 255]),
        stroke: "rgba(0, 0, 0, 1)",
        "stroke-width": 1.25,
      }, null, { category: "points", index });
    });
  }

  
  function draw(env: ViewerEnv) {
    env.clearSvgChildren(env.sceneLayer);
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
    findHitPoint,
    drawLines,
    drawPoints,
    draw,
    pathFromPoints,
    arcPath,
    appendSceneElement,
    appendPointPath,
  } as ViewerRenderModule;
})();
