(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

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
      if (line.binding?.kind === "line" || line.binding?.kind === "ray") {
        const start = env.toScreen(env.resolveScenePoint(line.binding.startIndex));
        const end = env.toScreen(env.resolveScenePoint(line.binding.endIndex));
        screenPoints = clipParametricLineToRect(
          start,
          end,
          env.sourceScene.width,
          env.sourceScene.height,
          line.binding.kind === "ray",
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
    env.ctx.textBaseline = "top";
    for (const label of env.currentScene().labels) {
      if (label.visible === false) continue;
      const bounds = labelBounds(env, label);
      env.ctx.fillStyle = env.rgba(label.color);
      bounds.lines.forEach((line, index) => {
        env.ctx.fillText(line, bounds.screen.x + 6, bounds.screen.y - 10 + index * 22);
      });
    }
  }

  function draw(env) {
    env.ctx.clearRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.ctx.fillStyle = "rgb(250,250,248)";
    env.ctx.fillRect(0, 0, env.sourceScene.width, env.sourceScene.height);
    env.drawGrid();
    drawPolygons(env);
    drawLines(env);
    drawCircles(env);
    drawPoints(env);
    drawLabels(env);
  }

  modules.render = {
    labelMetrics,
    labelBounds,
    findHitPoint,
    findHitLabel,
    drawPolygons,
    drawLines,
    drawCircles,
    drawPoints,
    drawLabels,
    draw,
  };
})();
