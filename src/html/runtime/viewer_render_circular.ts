(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  ) as Partial<ViewerModules> & { render: ViewerRenderModule };

  
  function arcGeometryFromPoints(start: Point, mid: Point, end: Point) {
    const geometry = window.GspRuntimeCore.threePointArcGeometry(start, mid, end);
    if (!geometry) return null;
    return {
      center: geometry.center,
      radius: geometry.radius,
      startAngle: geometry.startAngle,
      endAngle: geometry.endAngle,
      counterClockwise: geometry.ccwMid > geometry.ccwSpan + 1e-9,
    };
  }

  
  function midpointOnCircleWorld(start: Point, end: Point, center: Point, counterclockwise: boolean, yUp: boolean) {
    const controls = counterclockwise
      ? window.GspRuntimeCore.circleArcControlPoints(center, start, end, yUp)
      : window.GspRuntimeCore.circleArcControlPoints(center, end, start, yUp);
    return controls?.[1] ?? null;
  }

  
  modules.render.drawCircles = function drawCircles(env: ViewerEnv) {
    env.currentScene().circles.forEach((circle, index: number) => {
      const fillVisible = circle.fillColor && circle.fillVisible !== false;
      const strokeVisible = circle.visible !== false;
      if (!fillVisible && !strokeVisible) return;
      if (!circle.center || !circle.radiusPoint) return;
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
        fill: fillVisible ? env.rgba(circle.fillColor) : "none",
        stroke: strokeVisible ? env.rgba(circle.color) : "none",
        "stroke-width": 2,
        "stroke-dasharray": strokeVisible && circle.dashed ? "8 8" : null,
      }, null, { category: "circles", index });
    });
  };

  
  modules.render.drawArcs = function drawArcs(env: ViewerEnv) {
    (env.currentScene().arcs || []).forEach((arc, index: number) => {
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
        const worldPoints = arc.points.map(( handle) => env.resolvePoint(handle));
        if (worldPoints.some(( point) => !point)) return;
        screenPoints = worldPoints.filter((point): point is Point => point !== null).map(( point) => env.toScreen(point));
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
