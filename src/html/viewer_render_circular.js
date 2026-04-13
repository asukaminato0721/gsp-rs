// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   */
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
    const forwardSpan = ((endAngle - startAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2);
    const forwardMid = ((midAngle - startAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2);

    return {
      center,
      radius,
      startAngle,
      endAngle,
      counterClockwise: forwardMid > forwardSpan + 1e-9,
    };
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {Point} center
   * @param {boolean} counterclockwise
   * @param {boolean} yUp
   */
  function midpointOnCircleWorld(start, end, center, counterclockwise, yUp) {
    const ySign = yUp ? 1 : -1;
    const startAngle = Math.atan2((start.y - center.y) * ySign, start.x - center.x);
    const endAngle = Math.atan2((end.y - center.y) * ySign, end.x - center.x);
    const radius = (Math.hypot(start.x - center.x, start.y - center.y) + Math.hypot(end.x - center.x, end.y - center.y)) / 2;
    if (radius <= 1e-9) return null;
    const span = counterclockwise
      ? ((endAngle - startAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2)
      : -(((startAngle - endAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2));
    const midpointAngle = startAngle + span * 0.5;
    return {
      x: center.x + radius * Math.cos(midpointAngle),
      y: center.y + ySign * radius * Math.sin(midpointAngle),
    };
  }

  /** @param {ViewerEnv} env */
  modules.render.drawCircles = function drawCircles(env) {
    env.currentScene().circles.forEach((circle, index) => {
      if (circle.visible === false) return;
      const centerWorld = env.resolvePoint(circle.center);
      const radiusPointWorld = env.resolvePoint(circle.radiusPoint);
      if (!centerWorld || !radiusPointWorld) return;
      const center = env.toScreen(centerWorld);
      const radius = Math.hypot(
        radiusPointWorld.x - centerWorld.x,
        radiusPointWorld.y - centerWorld.y,
      ) * center.scale;
      if (radius <= 1e-9) return;
      modules.render.appendSceneElement(env, "circle", {
        cx: center.x,
        cy: center.y,
        r: radius,
        fill: circle.fillColor ? env.rgba(circle.fillColor) : "none",
        stroke: env.rgba(circle.color),
        "stroke-width": 2,
        "stroke-dasharray": circle.dashed ? "8 8" : null,
      }, null, { category: "circles", index });
    });
  };

  /** @param {ViewerEnv} env */
  modules.render.drawArcs = function drawArcs(env) {
    (env.currentScene().arcs || []).forEach((arc, index) => {
      if (arc.visible === false || !Array.isArray(arc.points) || arc.points.length !== 3) return;
      let screenPoints;
      if (arc.center) {
        const startWorld = env.resolvePoint(arc.points[0]);
        const endWorld = env.resolvePoint(arc.points[2]);
        const centerWorld = env.resolvePoint(arc.center);
        if (!startWorld || !endWorld || !centerWorld) return;
        const midpointWorld = midpointOnCircleWorld(
          startWorld,
          endWorld,
          centerWorld,
          arc.counterclockwise !== false,
          !!env.sourceScene.yUp,
        );
        if (!midpointWorld) return;
        screenPoints = [
          env.toScreen(startWorld),
          env.toScreen(midpointWorld),
          env.toScreen(endWorld),
        ];
      } else {
        const worldPoints = arc.points.map((/** @type {PointHandle} */ handle) => env.resolvePoint(handle));
        if (worldPoints.some((/** @type {Point | null} */ point) => !point)) return;
        screenPoints = worldPoints.map((/** @type {Point} */ point) => env.toScreen(point));
      }
      const geometry = arcGeometryFromPoints(screenPoints[0], screenPoints[1], screenPoints[2]);
      if (!geometry) return;
      modules.render.appendSceneElement(env, "path", {
        d: modules.render.arcPath(
          geometry.center,
          geometry.radius,
          geometry.startAngle,
          geometry.endAngle,
          geometry.counterClockwise,
        ),
        fill: "none",
        stroke: env.rgba(arc.color),
        "stroke-width": 2,
      }, null, { category: "arcs", index });
    });
  };
})();
