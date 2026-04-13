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
   * @param {Point} point
   * @param {Point[]} polygon
   */
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

  /**
   * @param {ViewerEnv} env
   * @param {ScenePolygonJson} polygon
   */
  function isFreePolygon(env, polygon) {
    if (polygon.binding) return false;
    if (polygon.points.length < 3) return false;
    return polygon.points.every((/** @type {PointHandle} */ handle) => {
      if (!hasPointIndexHandle(handle)) return false;
      const point = env.currentScene().points[handle.pointIndex];
      return point && !point.constraint && !point.binding;
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   */
  modules.render.findHitPolygon = function findHitPolygon(env, screenX, screenY) {
    for (let index = env.currentScene().polygons.length - 1; index >= 0; index -= 1) {
      const polygon = env.currentScene().polygons[index];
      if (polygon.visible === false || !isFreePolygon(env, polygon)) continue;
      const worldPoints = polygon.points.map((/** @type {PointHandle} */ handle) => env.resolvePoint(handle));
      if (worldPoints.some((/** @type {Point | null} */ point) => !point)) continue;
      const screenPoints = worldPoints.map((/** @type {Point} */ point) => env.toScreen(point));
      if (screenPoints.length < 3) continue;
      if (pointInPolygon({ x: screenX, y: screenY }, screenPoints)) {
        return index;
      }
    }
    return null;
  };

  /** @param {ViewerEnv} env */
  modules.render.drawPolygons = function drawPolygons(env) {
    for (const polygon of env.currentScene().polygons) {
      if (polygon.visible === false) continue;
      if (polygon.points.length < 3) continue;
      const worldPoints = polygon.points.map((/** @type {PointHandle} */ handle) => env.resolvePoint(handle));
      if (worldPoints.some((/** @type {Point | null} */ point) => !point)) continue;
      const screenPoints = worldPoints.map((/** @type {Point} */ point) => env.toScreen(point));
      modules.render.appendPointPath(env, screenPoints, {
        stroke: env.rgba(polygon.outlineColor),
        strokeWidth: 1.5,
        fill: env.rgba(polygon.color),
        close: true,
      });
    }
  };
})();
