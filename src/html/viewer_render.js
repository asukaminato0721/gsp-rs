(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function arcGeometryFromPoints(start, mid, end) {
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
    const forwardSpan = normalizeAngleDelta(startAngle, endAngle);
    const forwardMid = normalizeAngleDelta(startAngle, midAngle);

    return {
      center,
      radius,
      startAngle,
      endAngle,
      counterClockwise: forwardMid > forwardSpan + 1e-9,
    };
  }

  function normalizeAngleDelta(from, to) {
    const tau = Math.PI * 2;
    return ((to - from) % tau + tau) % tau;
  }

  function midpointOnCircleWorld(start, end, center, counterclockwise, yUp) {
    const ySign = yUp ? 1 : -1;
    const startAngle = Math.atan2((start.y - center.y) * ySign, start.x - center.x);
    const endAngle = Math.atan2((end.y - center.y) * ySign, end.x - center.x);
    const radius = (Math.hypot(start.x - center.x, start.y - center.y) + Math.hypot(end.x - center.x, end.y - center.y)) / 2;
    if (radius <= 1e-9) return null;
    const span = counterclockwise
      ? normalizeAngleDelta(startAngle, endAngle)
      : -normalizeAngleDelta(endAngle, startAngle);
    const midpointAngle = startAngle + span * 0.5;
    return {
      x: center.x + radius * Math.cos(midpointAngle),
      y: center.y + ySign * radius * Math.sin(midpointAngle),
    };
  }

  function clipParametricLineToRect(start, end, width, height, rayOnly) {
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
    return [hits[0].point, hits[hits.length - 1].point];
  }

  function labelMetrics(env, text) {
    const lines = text.split("\n");
    const width = lines.reduce((best, line) => Math.max(best, env.ctx.measureText(line).width), 0);
    return {
      lines,
      width,
      height: lines.length * 22,
    };
  }

  function labelBounds(env, label) {
    const screen = label.screenSpace
      ? { x: label.anchor.x, y: label.anchor.y }
      : env.toScreen(env.resolvePoint(label.anchor));
    const metrics = labelMetrics(env, label.text);
    if (label.centeredOnAnchor) {
      return {
        screen,
        lines: metrics.lines,
        width: metrics.width,
        height: metrics.height,
        left: screen.x - metrics.width / 2,
        top: screen.y - metrics.height / 2,
      };
    }
    return {
      screen,
      lines: metrics.lines,
      width: metrics.width,
      height: metrics.height,
      left: screen.x + 2,
      top: screen.y - 14,
    };
  }

  function findHitPoint(env, screenX, screenY) {
    let bestIndex = null;
    let bestDistanceSquared = env.pointHitRadius * env.pointHitRadius;
    env.currentScene().points.forEach((point, index) => {
      if (point.visible === false) {
        return;
      }
      const screen = env.toScreen(env.resolveScenePoint(index));
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

  function findHitLabel(env, screenX, screenY) {
    env.ctx.save();
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.textBaseline = "top";
    for (let index = env.currentScene().labels.length - 1; index >= 0; index -= 1) {
      const label = env.currentScene().labels[index];
      if (label.visible === false) {
        continue;
      }
      const bounds = labelBounds(env, label);
      if (
        screenX >= bounds.left &&
        screenX <= bounds.left + bounds.width + 8 &&
        screenY >= bounds.top &&
        screenY <= bounds.top + bounds.height
      ) {
        env.ctx.restore();
        return index;
      }
    }
    env.ctx.restore();
    return null;
  }

  function pointInPolygon(point, polygon) {
    let inside = false;
    for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i, i += 1) {
      const xi = polygon[i].x;
      const yi = polygon[i].y;
      const xj = polygon[j].x;
      const yj = polygon[j].y;
      const intersects = ((yi > point.y) !== (yj > point.y))
        && (point.x < ((xj - xi) * (point.y - yi)) / ((yj - yi) || 1e-9) + xi);
      if (intersects) inside = !inside;
    }
    return inside;
  }

  function isFreePolygon(env, polygon) {
    if (polygon.binding) return false;
    if (polygon.points.length < 3) return false;
    return polygon.points.every((handle) => {
      if (typeof handle?.pointIndex !== "number") return false;
      const point = env.currentScene().points[handle.pointIndex];
      return point && !point.constraint && !point.binding;
    });
  }

  function findHitPolygon(env, screenX, screenY) {
    for (let index = env.currentScene().polygons.length - 1; index >= 0; index -= 1) {
      const polygon = env.currentScene().polygons[index];
      if (polygon.visible === false || !isFreePolygon(env, polygon)) continue;
      const screenPoints = polygon.points.map((handle) => env.toScreen(env.resolvePoint(handle)));
      if (screenPoints.length < 3) continue;
      if (pointInPolygon({ x: screenX, y: screenY }, screenPoints)) {
        return index;
      }
    }
    return null;
  }

  function drawPolygons(env) {
    for (const polygon of env.currentScene().polygons) {
      if (polygon.visible === false) continue;
      if (polygon.points.length < 3) continue;
      env.ctx.beginPath();
      polygon.points.forEach((handle, index) => {
        const screen = env.toScreen(env.resolvePoint(handle));
        if (index === 0) {
          env.ctx.moveTo(screen.x, screen.y);
        } else {
          env.ctx.lineTo(screen.x, screen.y);
        }
      });
      env.ctx.closePath();
      env.ctx.fillStyle = env.rgba(polygon.color);
      env.ctx.strokeStyle = env.rgba(polygon.outlineColor);
      env.ctx.lineWidth = 1.5;
      env.ctx.fill();
      env.ctx.stroke();
    }
  }

  function drawLines(env) {
    for (const line of env.currentScene().lines) {
      if (line.visible === false) continue;
      let screenPoints = null;
      const resolveHostLinePoints = (binding) => {
        if (typeof binding?.lineIndex === "number") {
          return env.resolveLinePoints(binding.lineIndex);
        }
        if (
          typeof binding?.lineStartIndex === "number"
          && typeof binding?.lineEndIndex === "number"
        ) {
          return [
            env.resolveScenePoint(binding.lineStartIndex),
            env.resolveScenePoint(binding.lineEndIndex),
          ];
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
          ? env.toScreen(env.resolveScenePoint(line.binding.throughIndex))
          : line.binding.kind === "angle-bisector-ray"
            ? env.toScreen(env.resolveScenePoint(line.binding.vertexIndex))
          : env.toScreen(env.resolveScenePoint(line.binding.startIndex));
        const end = line.binding.kind === "perpendicular-line"
          ? (() => {
              const through = env.resolveScenePoint(line.binding.throughIndex);
              const hostLine = resolveHostLinePoints(line.binding);
              if (!hostLine) return null;
              const [lineStart, lineEnd] = hostLine;
              const dx = lineEnd.x - lineStart.x;
              const dy = lineEnd.y - lineStart.y;
              const len = Math.hypot(dx, dy);
              if (len <= 1e-9) return null;
              return env.toScreen({
                x: through.x - dy / len,
                y: through.y + dx / len,
              });
            })()
          : line.binding.kind === "parallel-line"
            ? (() => {
                const through = env.resolveScenePoint(line.binding.throughIndex);
                const hostLine = resolveHostLinePoints(line.binding);
                if (!hostLine) return null;
                const [lineStart, lineEnd] = hostLine;
                const dx = lineEnd.x - lineStart.x;
                const dy = lineEnd.y - lineStart.y;
                const len = Math.hypot(dx, dy);
                if (len <= 1e-9) return null;
                return env.toScreen({
                  x: through.x + dx / len,
                  y: through.y + dy / len,
                });
              })()
          : line.binding.kind === "angle-bisector-ray"
            ? (() => {
                const startPoint = env.resolveScenePoint(line.binding.startIndex);
                const vertex = env.resolveScenePoint(line.binding.vertexIndex);
                const endPoint = env.resolveScenePoint(line.binding.endIndex);
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
                return env.toScreen({
                  x: vertex.x + direction.x,
                  y: vertex.y + direction.y,
                });
              })()
          : env.toScreen(env.resolveScenePoint(line.binding.endIndex));
        if (!end) continue;
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
          : line.points.map((handle) => env.resolvePoint(handle));
        if (points && points.length >= 2) {
          screenPoints = points.map((point) => env.toScreen(point));
        }
      }
      if (!screenPoints || screenPoints.length < 2) continue;
      env.ctx.beginPath();
      screenPoints.forEach((screen, index) => {
        if (index === 0) {
          env.ctx.moveTo(screen.x, screen.y);
        } else {
          env.ctx.lineTo(screen.x, screen.y);
        }
      });
      env.ctx.strokeStyle = env.rgba(line.color);
      env.ctx.lineWidth = 2;
      env.ctx.setLineDash(line.dashed ? [8, 8] : []);
      env.ctx.stroke();
    }
    env.ctx.setLineDash([]);
  }

  function drawCircles(env) {
    for (const circle of env.currentScene().circles) {
      if (circle.visible === false) continue;
      const centerWorld = env.resolvePoint(circle.center);
      const radiusPointWorld = env.resolvePoint(circle.radiusPoint);
      const center = env.toScreen(centerWorld);
      const radius = Math.hypot(
        radiusPointWorld.x - centerWorld.x,
        radiusPointWorld.y - centerWorld.y,
      ) * center.scale;
      env.ctx.beginPath();
      env.ctx.arc(center.x, center.y, radius, 0, Math.PI * 2);
      env.ctx.strokeStyle = env.rgba(circle.color);
      env.ctx.lineWidth = 2;
      env.ctx.setLineDash(circle.dashed ? [8, 8] : []);
      env.ctx.stroke();
    }
    env.ctx.setLineDash([]);
  }

  function drawArcs(env) {
    for (const arc of env.currentScene().arcs || []) {
      if (arc.visible === false || !Array.isArray(arc.points) || arc.points.length !== 3) continue;
      let screenPoints;
      if (arc.center) {
        const startWorld = env.resolvePoint(arc.points[0]);
        const endWorld = env.resolvePoint(arc.points[2]);
        const centerWorld = env.resolvePoint(arc.center);
        const midpointWorld = midpointOnCircleWorld(
          startWorld,
          endWorld,
          centerWorld,
          arc.counterclockwise !== false,
          !!env.sourceScene.yUp,
        );
        if (!midpointWorld) continue;
        screenPoints = [
          env.toScreen(startWorld),
          env.toScreen(midpointWorld),
          env.toScreen(endWorld),
        ];
      } else {
        screenPoints = arc.points.map((handle) => env.toScreen(env.resolvePoint(handle)));
      }
      const geometry = arcGeometryFromPoints(screenPoints[0], screenPoints[1], screenPoints[2]);
      if (!geometry) continue;
      env.ctx.beginPath();
      env.ctx.arc(
        geometry.center.x,
        geometry.center.y,
        geometry.radius,
        geometry.startAngle,
        geometry.endAngle,
        geometry.counterClockwise,
      );
      env.ctx.strokeStyle = env.rgba(arc.color);
      env.ctx.lineWidth = 2;
      env.ctx.stroke();
    }
  }

  function drawPoints(env) {
    env.currentScene().points.forEach((point, index) => {
      if (point.visible === false) {
        return;
      }
      const screen = env.toScreen(env.resolveScenePoint(index));
      env.ctx.beginPath();
      env.ctx.arc(screen.x, screen.y, index === env.hoverPointIndex.val ? 6 : 4, 0, Math.PI * 2);
      env.ctx.fillStyle = index === env.hoverPointIndex.val ? "rgba(255, 120, 20, 1)" : "rgba(255, 60, 40, 1)";
      env.ctx.fill();
    });
  }

  function drawLabels(env) {
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    for (const label of env.currentScene().labels) {
      if (label.visible === false) continue;
      const bounds = labelBounds(env, label);
      env.ctx.fillStyle = env.rgba(label.color);
      if (label.centeredOnAnchor) {
        env.ctx.textAlign = "center";
        env.ctx.textBaseline = "middle";
        const midOffset = (bounds.lines.length - 1) / 2;
        bounds.lines.forEach((line, index) => {
          env.ctx.fillText(line, bounds.screen.x, bounds.screen.y + (index - midOffset) * 22);
        });
      } else {
        env.ctx.textAlign = "left";
        env.ctx.textBaseline = "top";
        bounds.lines.forEach((line, index) => {
          env.ctx.fillText(line, bounds.left + 4, bounds.top + index * 22);
        });
      }
    }
    env.ctx.textAlign = "left";
    env.ctx.textBaseline = "alphabetic";
  }

  function draw(env) {
    env.ctx.clearRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.ctx.fillStyle = "rgb(250,250,248)";
    env.ctx.fillRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.drawGrid();
    drawPolygons(env);
    drawLines(env);
    drawCircles(env);
    drawArcs(env);
    drawPoints(env);
    drawLabels(env);
  }

  modules.render = {
    labelMetrics,
    labelBounds,
    findHitPoint,
    findHitLabel,
    findHitPolygon,
    drawPolygons,
    drawLines,
    drawCircles,
    drawArcs,
    drawPoints,
    drawLabels,
    draw,
  };
})();
