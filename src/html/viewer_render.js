(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  const imageCache = new Map();

  function loadImage(src, env) {
    let entry = imageCache.get(src);
    if (entry) return entry;
    const img = new Image();
    entry = { img, loaded: false };
    img.onload = () => {
      entry.loaded = true;
      if (env?.ctx) {
        requestAnimationFrame(() => draw(env));
      }
    };
    img.src = src;
    imageCache.set(src, entry);
    return entry;
  }

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
    const worldAnchor = label.screenSpace
      ? { x: label.anchor.x, y: label.anchor.y }
      : env.resolvePoint(label.anchor);
    if (!worldAnchor) return null;
    const screen = label.screenSpace
      ? worldAnchor
      : env.toScreen(worldAnchor);
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

  function labelHotspotRects(env, label) {
    if (!label.hotspots?.length) {
      return [];
    }
    env.ctx.save();
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.textBaseline = "top";
    const bounds = labelBounds(env, label);
    if (!bounds) {
      env.ctx.restore();
      return [];
    }
    const rects = label.hotspots
      .map((hotspot) => {
        const line = bounds.lines[hotspot.line];
        if (typeof line !== "string") {
          return null;
        }
        const glyphs = Array.from(line);
        const start = Math.max(0, Math.min(glyphs.length, hotspot.start));
        const end = Math.max(start, Math.min(glyphs.length, hotspot.end));
        const prefix = glyphs.slice(0, start).join("");
        const text = glyphs.slice(start, end).join("");
        if (!text) {
          return null;
        }
        return {
          line: hotspot.line,
          start,
          end,
          text: hotspot.text || text,
          left: bounds.left + 4 + env.ctx.measureText(prefix).width,
          top: bounds.top + hotspot.line * 22,
          width: Math.max(1, env.ctx.measureText(text).width),
          height: 22,
          action: hotspot.action,
        };
      })
      .filter(Boolean);
    env.ctx.restore();
    return rects;
  }

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
      if (!bounds) continue;
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
      const worldPoints = polygon.points.map((handle) => env.resolvePoint(handle));
      if (worldPoints.some((point) => !point)) continue;
      const screenPoints = worldPoints.map((point) => env.toScreen(point));
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
      const worldPoints = polygon.points.map((handle) => env.resolvePoint(handle));
      if (worldPoints.some((point) => !point)) continue;
      env.ctx.beginPath();
      worldPoints.forEach((point, index) => {
        const screen = env.toScreen(point);
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

  function drawImages(env) {
    for (const image of env.currentScene().images || []) {
      const entry = loadImage(image.src, env);
      if (!entry.loaded) continue;

      const topLeft = image.screenSpace
        ? image.topLeft
        : env.toScreen(image.topLeft);
      const bottomRight = image.screenSpace
        ? image.bottomRight
        : env.toScreen(image.bottomRight);
      if (!topLeft || !bottomRight) continue;

      const left = Math.min(topLeft.x, bottomRight.x);
      const top = Math.min(topLeft.y, bottomRight.y);
      const width = Math.abs(bottomRight.x - topLeft.x);
      const height = Math.abs(bottomRight.y - topLeft.y);
      if (width <= 1e-6 || height <= 1e-6) continue;

      env.ctx.drawImage(entry.img, left, top, width, height);
    }
  }

  function drawLines(env) {
    const resolveRightAngleMarkerPoints = (vertex, first, second, shortestLen, layerIndex, layerCount) => {
      const sideBase = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
      const side = sideBase + layerIndex * 5;
      if (side <= 1e-9) return null;
      return [
        { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
        { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
        { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
      ];
    };
    const resolveArcAngleMarkerPoints = (vertex, first, shortestLen, cross, dot, layerIndex, layerCount) => {
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
    const drawPolyline = (worldPoints, color, dashed) => {
      const screenPoints = worldPoints.map((point) => env.toScreen(point));
      if (screenPoints.length < 2) return;
      env.ctx.beginPath();
      screenPoints.forEach((screen, index) => {
        if (index === 0) env.ctx.moveTo(screen.x, screen.y);
        else env.ctx.lineTo(screen.x, screen.y);
      });
      env.ctx.strokeStyle = env.rgba(color);
      env.ctx.lineWidth = 2;
      env.ctx.setLineDash(dashed ? [8, 8] : []);
      env.ctx.stroke();
    };
    const drawAngleMarker = (line) => {
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
          ? resolveRightAngleMarkerPoints(vertex, first, second, shortestLen, layerIndex, layerCount)
          : resolveArcAngleMarkerPoints(vertex, first, shortestLen, cross, dot, layerIndex, layerCount);
        if (points) drawPolyline(points, line.color, line.dashed);
      }
    };
    const drawSegmentMarker = (line) => {
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
          {
            x: slashCenter.x - normal.x * halfLen,
            y: slashCenter.y - normal.y * halfLen,
          },
          {
            x: slashCenter.x + normal.x * halfLen,
            y: slashCenter.y + normal.y * halfLen,
          },
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
      const resolveHostLinePoints = (binding) => {
        if (
          typeof binding?.lineStartIndex === "number"
          && typeof binding?.lineEndIndex === "number"
        ) {
          return [
            env.resolveScenePoint(binding.lineStartIndex),
            env.resolveScenePoint(binding.lineEndIndex),
          ];
        }
        if (typeof binding?.lineIndex === "number") {
          return env.resolveLinePoints(binding.lineIndex);
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
              return env.toScreen({
                x: through.x - dy / len,
                y: through.y + dx / len,
              });
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
                return env.toScreen({
                  x: vertex.x + direction.x,
                  y: vertex.y + direction.y,
                });
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
      if (!centerWorld || !radiusPointWorld) continue;
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
        if (!startWorld || !endWorld || !centerWorld) continue;
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
        const worldPoints = arc.points.map((handle) => env.resolvePoint(handle));
        if (worldPoints.some((point) => !point)) continue;
        screenPoints = worldPoints.map((point) => env.toScreen(point));
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

  function drawLabels(env) {
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    for (const label of env.currentScene().labels) {
      if (label.visible === false || (label.richMarkup && !label.hotspots?.length)) continue;
      const bounds = labelBounds(env, label);
      if (!bounds) continue;
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

  function drawHotspotFlashes(env) {
    const flashes = env.currentHotspotFlashes ? env.currentHotspotFlashes() : [];
    if (!flashes?.length) {
      return;
    }

    const strokePolyline = (points) => {
      if (!points || points.length < 2) return;
      env.ctx.beginPath();
      points.forEach((point, index) => {
        const screen = env.toScreen(point);
        if (index === 0) env.ctx.moveTo(screen.x, screen.y);
        else env.ctx.lineTo(screen.x, screen.y);
      });
      env.ctx.stroke();
    };

    env.ctx.save();
    env.ctx.strokeStyle = "rgba(255, 176, 32, 0.95)";
    env.ctx.fillStyle = "rgba(255, 210, 80, 0.22)";
    env.ctx.lineWidth = 5;
    env.ctx.lineJoin = "round";
    env.ctx.lineCap = "round";

    flashes.forEach((flash) => {
      const action = flash.action || {};
      switch (action.kind) {
        case "point": {
          const point = env.resolveScenePoint(action.pointIndex);
          if (!point) break;
          const screen = env.toScreen(point);
          env.ctx.beginPath();
          env.ctx.arc(screen.x, screen.y, 9, 0, Math.PI * 2);
          env.ctx.fill();
          env.ctx.stroke();
          break;
        }
        case "segment": {
          const start = env.resolveScenePoint(action.startPointIndex);
          const end = env.resolveScenePoint(action.endPointIndex);
          if (!start || !end) break;
          strokePolyline([start, end]);
          break;
        }
        case "angle-marker": {
          const line = env.currentScene().lines.find((candidate) =>
            candidate.binding?.kind === "angle-marker"
            && candidate.binding.startIndex === action.startPointIndex
            && candidate.binding.vertexIndex === action.vertexPointIndex
            && candidate.binding.endIndex === action.endPointIndex
          );
          if (!line) break;
          const points = env.resolveLinePoints(line);
          strokePolyline(points || []);
          break;
        }
        case "circle": {
          const circle = env.currentScene().circles[action.circleIndex];
          if (!circle) break;
          const center = env.resolvePoint(circle.center);
          const radiusPoint = env.resolvePoint(circle.radiusPoint);
          if (!center || !radiusPoint) break;
          const screenCenter = env.toScreen(center);
          const screenRadiusPoint = env.toScreen(radiusPoint);
          env.ctx.beginPath();
          env.ctx.arc(
            screenCenter.x,
            screenCenter.y,
            Math.hypot(screenRadiusPoint.x - screenCenter.x, screenRadiusPoint.y - screenCenter.y),
            0,
            Math.PI * 2,
          );
          env.ctx.stroke();
          break;
        }
        case "polygon": {
          const polygon = env.currentScene().polygons[action.polygonIndex];
          if (!polygon || polygon.points.length < 3) break;
          const points = polygon.points.map((handle) => env.resolvePoint(handle));
          if (points.some((point) => !point)) break;
          env.ctx.beginPath();
          points.forEach((point, index) => {
            const screen = env.toScreen(point);
            if (index === 0) env.ctx.moveTo(screen.x, screen.y);
            else env.ctx.lineTo(screen.x, screen.y);
          });
          env.ctx.closePath();
          env.ctx.fill();
          env.ctx.stroke();
          break;
        }
        default:
          break;
      }
    });

    env.ctx.restore();
  }

  function draw(env) {
    env.ctx.clearRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.ctx.fillStyle = "rgb(250,250,248)";
    env.ctx.fillRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.drawGrid();
    drawImages(env);
    drawPolygons(env);
    drawLines(env);
    drawCircles(env);
    drawArcs(env);
    drawPoints(env);
    drawLabels(env);
    drawHotspotFlashes(env);
  }

  modules.render = {
    labelMetrics,
    labelBounds,
    labelHotspotRects,
    findHitPoint,
    findHitLabel,
    findHitPolygon,
    drawImages,
    drawPolygons,
    drawLines,
    drawCircles,
    drawArcs,
    drawPoints,
    drawLabels,
    draw,
  };
})();
