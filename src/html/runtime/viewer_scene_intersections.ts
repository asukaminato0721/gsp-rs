(function() {
  const modules = (window.GspViewerModules || (window.GspViewerModules = {})) as Partial<ViewerModules> & {
    scene: ViewerSceneModule;
  };
  const scene = modules.scene;

  function lineLineIntersection(
    leftStart: Point,
    leftEnd: Point,
    leftKind: RuntimeLineKind,
    rightStart: Point,
    rightEnd: Point,
    rightKind: RuntimeLineKind,
  ) {
    return window.GspRuntimeCore.lineLineIntersection(
      leftStart,
      leftEnd,
      leftKind,
      rightStart,
      rightEnd,
      rightKind,
    );
  }

  function lineCircleIntersection(
    lineStart: Point,
    lineEnd: Point,
    lineKind: RuntimeLineKind,
    center: Point,
    radiusPoint: Point,
    variant: number,
  ) {
    const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
    return window.GspRuntimeCore.lineCircleIntersectionCandidate(
      lineStart,
      lineEnd,
      lineKind,
      center,
      radius,
      variant,
    );
  }

  function circleCircleIntersection(
    leftCenter: Point,
    leftRadiusPoint: Point,
    rightCenter: Point,
    rightRadiusPoint: Point,
    variant: number,
    reference?: Point | RuntimeScenePointJson | null,
  ) {
    const candidates = window.GspRuntimeCore.circleCircleIntersections(
      leftCenter,
      Math.hypot(leftRadiusPoint.x - leftCenter.x, leftRadiusPoint.y - leftCenter.y),
      rightCenter,
      Math.hypot(rightRadiusPoint.x - rightCenter.x, rightRadiusPoint.y - rightCenter.y),
    );
    return window.GspRuntimeCore.choosePointCandidate(candidates, reference ?? null, variant);
  }

  scene.lineLineIntersection = lineLineIntersection;
  scene.lineCircleIntersection = lineCircleIntersection;
  scene.circleCircleIntersection = circleCircleIntersection;
})();
