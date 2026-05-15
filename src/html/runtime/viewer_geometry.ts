(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  
  function normalizeAngleDelta(from: number, to: number) {
    const tau = Math.PI * 2;
    return ((to - from) % tau + tau) % tau;
  }

  
  function lerpPoint(start: Point, end: Point, t: number) {
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }

  
  function rotateAround(point: Point, center: Point, radians: number) {
    const cos = Math.cos(radians);
    const sin = Math.sin(radians);
    const dx = point.x - center.x;
    const dy = point.y - center.y;
    return {
      x: center.x + dx * cos + dy * sin,
      y: center.y - dx * sin + dy * cos,
    };
  }

  
  function scaleAround(point: Point, center: Point, factor: number) {
    return {
      x: center.x + (point.x - center.x) * factor,
      y: center.y + (point.y - center.y) * factor,
    };
  }

  
  function reflectAcrossLine(point: Point, lineStart: Point, lineEnd: Point) {
    const dx = lineEnd.x - lineStart.x;
    const dy = lineEnd.y - lineStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) return null;
    const t = ((point.x - lineStart.x) * dx + (point.y - lineStart.y) * dy) / lenSq;
    const projection = {
      x: lineStart.x + t * dx,
      y: lineStart.y + t * dy,
    };
    return {
      x: projection.x * 2 - point.x,
      y: projection.y * 2 - point.y,
    };
  }

  
  function clipParametricLineToBounds(start: Point, end: Point, bounds: { minX: number; maxX: number; minY: number; maxY: number }, rayOnly: boolean) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return null;

    
    const hits = [];
    
    const pushHit = (t, point) => {
      if (!Number.isFinite(t)) return;
      if (rayOnly && t < -1e-9) return;
      if (
        point.x < bounds.minX - 1e-6 || point.x > bounds.maxX + 1e-6 ||
        point.y < bounds.minY - 1e-6 || point.y > bounds.maxY + 1e-6
      ) return;
      if (hits.some((hit) =>
        Math.abs(hit.t - t) < 1e-6 ||
        (Math.abs(hit.point.x - point.x) < 1e-6 && Math.abs(hit.point.y - point.y) < 1e-6)
      )) return;
      hits.push({ t, point });
    };

    if (Math.abs(dx) > 1e-9) {
      for (const x of [bounds.minX, bounds.maxX]) {
        const t = (x - start.x) / dx;
        pushHit(t, { x, y: start.y + dy * t });
      }
    }
    if (Math.abs(dy) > 1e-9) {
      for (const y of [bounds.minY, bounds.maxY]) {
        const t = (y - start.y) / dy;
        pushHit(t, { x: start.x + dx * t, y });
      }
    }
    if (
      rayOnly &&
      start.x >= bounds.minX - 1e-6 && start.x <= bounds.maxX + 1e-6 &&
      start.y >= bounds.minY - 1e-6 && start.y <= bounds.maxY + 1e-6
    ) {
      pushHit(0, { ...start });
    }
    if (hits.length < 2) return null;
    hits.sort((a, b) => a.t - b.t);
    return [hits[0].point, hits[hits.length - 1].point];
  }

  
  function clipLineToBounds(start: Point, end: Point, bounds: { minX: number; maxX: number; minY: number; maxY: number }) {
    return clipParametricLineToBounds(start, end, bounds, false);
  }

  
  function clipRayToBounds(start: Point, end: Point, bounds: { minX: number; maxX: number; minY: number; maxY: number }) {
    return clipParametricLineToBounds(start, end, bounds, true);
  }

  
  function angleBisectorDirection(start: Point, vertex: Point, end: Point) {
    const startDx = start.x - vertex.x;
    const startDy = start.y - vertex.y;
    const startLen = Math.hypot(startDx, startDy);
    const endDx = end.x - vertex.x;
    const endDy = end.y - vertex.y;
    const endLen = Math.hypot(endDx, endDy);
    if (startLen <= 1e-9 || endLen <= 1e-9) return null;

    const sumX = startDx / startLen + endDx / endLen;
    const sumY = startDy / startLen + endDy / endLen;
    const sumLen = Math.hypot(sumX, sumY);
    if (sumLen > 1e-9) {
      return { x: sumX / sumLen, y: sumY / sumLen };
    }
    return { x: -startDy / startLen, y: startDx / startLen };
  }

  
  function measuredRotationRadians(start: Point, vertex: Point, end: Point) {
    const firstX = start.x - vertex.x;
    const firstY = vertex.y - start.y;
    const secondX = end.x - vertex.x;
    const secondY = vertex.y - end.y;
    const firstLen = Math.hypot(firstX, firstY);
    const secondLen = Math.hypot(secondX, secondY);
    if (firstLen <= 1e-9 || secondLen <= 1e-9) return null;
    return Math.atan2(firstX * secondY - firstY * secondX, firstX * secondX + firstY * secondY);
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
    const denominatorDx = ratioDenominator.x - ratioOrigin.x;
    const denominatorDy = ratioDenominator.y - ratioOrigin.y;
    const numeratorDx = ratioNumerator.x - ratioOrigin.x;
    const numeratorDy = ratioNumerator.y - ratioOrigin.y;
    const denominator = Math.hypot(denominatorDx, denominatorDy);
    if (denominator <= 1e-9) return null;
    const rawNumerator = Math.hypot(numeratorDx, numeratorDy);
    const numerator = clampToUnit ? Math.min(rawNumerator, denominator) : rawNumerator;
    const direction = signed && denominatorDx * numeratorDx + denominatorDy * numeratorDy < 0
      ? -1
      : 1;
    return scaleAround(source, center, direction * numerator / denominator);
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
