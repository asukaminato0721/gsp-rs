// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /** @param {ViewerEnv} env */
  modules.render.drawHotspotFlashes = function drawHotspotFlashes(env) {
    const flashes = env.currentHotspotFlashes ? env.currentHotspotFlashes() : [];
    if (!flashes?.length) {
      return;
    }

    const strokePolyline = (/** @type {Point[]} */ points, /** @type {boolean} */ close = false) => {
      if (!points || points.length < 2) return;
      const screenPoints = points.map((/** @type {Point} */ point) => env.toScreen(point));
      modules.render.appendPointPath(env, screenPoints, {
        stroke: "rgba(255, 176, 32, 0.95)",
        strokeWidth: 5,
        fill: close ? "rgba(255, 210, 80, 0.22)" : "none",
        close,
      });
    };

    flashes.forEach((flash) => {
      const action = flash.action;
      if (!action) return;
      switch (action.kind) {
        case "point": {
          const point = env.resolveScenePoint(action.pointIndex);
          if (!point) break;
          const screen = env.toScreen(point);
          modules.render.appendSceneElement(env, "circle", {
            cx: screen.x,
            cy: screen.y,
            r: 9,
            fill: "rgba(255, 210, 80, 0.22)",
            stroke: "rgba(255, 176, 32, 0.95)",
            "stroke-width": 5,
          });
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
          modules.render.appendSceneElement(env, "circle", {
            cx: screenCenter.x,
            cy: screenCenter.y,
            r: Math.hypot(screenRadiusPoint.x - screenCenter.x, screenRadiusPoint.y - screenCenter.y),
            fill: "none",
            stroke: "rgba(255, 176, 32, 0.95)",
            "stroke-width": 5,
          });
          break;
        }
        case "polygon": {
          const polygon = env.currentScene().polygons[action.polygonIndex];
          if (!polygon || polygon.points.length < 3) break;
          const points = polygon.points.map((/** @type {PointHandle} */ handle) => env.resolvePoint(handle));
          if (points.some((/** @type {Point | null} */ point) => !point)) break;
          strokePolyline(/** @type {Point[]} */ (points), true);
          break;
        }
        default:
          break;
      }
    });
  };
})();
