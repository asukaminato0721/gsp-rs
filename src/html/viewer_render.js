(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

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
    env.currentScene().points.forEach((_, index) => {
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
      if (line.points.length < 2) continue;
      env.ctx.beginPath();
      line.points.forEach((handle, index) => {
        const screen = env.toScreen(env.resolvePoint(handle));
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
    env.currentScene().points.forEach((_, index) => {
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
