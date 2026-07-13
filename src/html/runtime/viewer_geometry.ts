(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  
  function normalizeAngleDelta(from: number, to: number) {
    return window.GspRuntimeCore.normalizeAngleDelta(from, to);
  }

  
  function lerpPoint(start: Point, end: Point, t: number) {
    return window.GspRuntimeCore.lerpPoint(start, end, t);
  }

  
  function rotateAround(point: Point, center: Point, radians: number) {
    return window.GspRuntimeCore.rotateAround(point, center, radians);
  }

  
  function scaleAround(point: Point, center: Point, factor: number) {
    return window.GspRuntimeCore.scaleAround(point, center, factor);
  }

  
  function reflectAcrossLine(point: Point, lineStart: Point, lineEnd: Point) {
    return window.GspRuntimeCore.reflectAcrossLine(point, lineStart, lineEnd);
  }

  
  function clipParametricLineToBounds(start: Point, end: Point, bounds: { minX: number; maxX: number; minY: number; maxY: number }, rayOnly: boolean) {
    return rayOnly
      ? window.GspRuntimeCore.clipRayToBounds(start, end, bounds)
      : window.GspRuntimeCore.clipLineToBounds(start, end, bounds);
  }

  
  function clipLineToBounds(start: Point, end: Point, bounds: { minX: number; maxX: number; minY: number; maxY: number }) {
    return clipParametricLineToBounds(start, end, bounds, false);
  }

  
  function clipRayToBounds(start: Point, end: Point, bounds: { minX: number; maxX: number; minY: number; maxY: number }) {
    return clipParametricLineToBounds(start, end, bounds, true);
  }

  
  function angleBisectorDirection(start: Point, vertex: Point, end: Point) {
    return window.GspRuntimeCore.angleBisectorDirection(start, vertex, end);
  }

  
  function measuredRotationRadians(start: Point, vertex: Point, end: Point) {
    return window.GspRuntimeCore.measuredRotationRadians(start, vertex, end);
  }

  
  function scaleByThreePointRatio(
    source: Point,
    center: Point,
    ratioOrigin: Point,
    ratioDenominator: Point,
    ratioNumerator: Point,
    signed: boolean = true,
    clampToUnit: boolean = false,
  ) {
    return window.GspRuntimeCore.scaleByThreePointRatio(
      source,
      center,
      ratioOrigin,
      ratioDenominator,
      ratioNumerator,
      signed,
      clampToUnit,
    );
  }

  modules.geometry = {
    normalizeAngleDelta,
    lerpPoint,
    rotateAround,
    scaleAround,
    reflectAcrossLine,
    clipParametricLineToBounds,
    clipLineToBounds,
    clipRayToBounds,
    angleBisectorDirection,
    measuredRotationRadians,
    scaleByThreePointRatio,
  };
})();
