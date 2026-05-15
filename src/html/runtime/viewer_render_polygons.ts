(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  );

  
  function hasPointIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { pointIndex: number }> {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  
  function pointInPolygon(point: Point, polygon: Point[]) {
    let inside = false;
    for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i, i += 1) {
      const current = polygon[i];
      const previous = polygon[j];
      if (!current || !previous) continue;
      const xi = current.x;
      const yi = current.y;
      const xj = previous.x;
      const yj = previous.y;
      const intersects = ((yi > point.y) !== (yj > point.y))
        && (point.x < ((xj - xi) * (point.y - yi)) / ((yj - yi) || 1e-9) + xi);
      if (intersects) inside = !inside;
    }
    return inside;
  }

  
  function isFreePolygon(env: ViewerEnv, polygon: ScenePolygonJson) {
    if (polygon.binding) return false;
    if (polygon.points.length < 3) return false;
    return polygon.points.every(( handle) => {
      if (!hasPointIndexHandle(handle)) return false;
      const point = env.currentScene().points[handle.pointIndex];
      return point && !point.constraint && !point.binding;
    });
  }

  
  modules.render.findHitPolygon = function findHitPolygon(env: ViewerEnv, screenX: number, screenY: number) {
    for (let index = env.currentScene().polygons.length - 1; index >= 0; index -= 1) {
      const polygon = env.currentScene().polygons[index];
      if (polygon.visible === false || !isFreePolygon(env, polygon)) continue;
      const worldPoints = polygon.points.map(( handle) => env.resolvePoint(handle));
      if (worldPoints.some(( point) => !point)) continue;
      const resolvedPoints = worldPoints as Point[];
      const screenPoints = resolvedPoints.map(( point) => env.toScreen(point));
      if (screenPoints.length < 3) continue;
      if (pointInPolygon({ x: screenX, y: screenY }, screenPoints)) {
        return index;
      }
    }
    return null;
  };

  
  modules.render.drawPolygons = function drawPolygons(env: ViewerEnv) {
    env.currentScene().polygons.forEach((polygon, index: number) => {
      if (polygon.visible === false) return;
      if (polygon.points.length < 3) return;
      const worldPoints = polygon.points.map(( handle) => env.resolvePoint(handle));
      if (worldPoints.some(( point) => !point)) return;
      const resolvedPoints = worldPoints as Point[];
      const screenPoints = resolvedPoints.map(( point) => env.toScreen(point));
      modules.render.appendPointPath(env, screenPoints, {
        stroke: env.rgba(polygon.outlineColor),
        strokeWidth: 1.5,
        fill: env.rgba(polygon.color),
        close: true,
        debugTarget: { category: "polygons", index },
      });
    });
  };
})();
