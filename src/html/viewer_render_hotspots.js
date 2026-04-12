// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /** @param {ViewerEnv} env */
  modules.render.drawHotspotFlashes = function drawHotspotFlashes(env) {
    const flashes = env.currentHotspotFlashes ? env.currentHotspotFlashes() : [];
    if (!flashes?.length) {
      return;
    }

    const strokePolyline = (/** @type {Point[]} */ points) => {
      if (!points || points.length < 2) return;
      env.ctx.beginPath();
      points.forEach((/** @type {Point} */ point, /** @type {number} */ index) => {
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
      const action = flash.action;
      if (!action) return;
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
          const points = polygon.points.map((/** @type {PointHandle} */ handle) => env.resolvePoint(handle));
          if (points.some((/** @type {Point | null} */ point) => !point)) break;
          env.ctx.beginPath();
          points.forEach((/** @type {Point} */ point, /** @type {number} */ index) => {
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
  };
})();
