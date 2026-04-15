// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {number} from
   * @param {number} to
   */
  function normalizeAngleDelta(from, to) {
    const tau = Math.PI * 2;
    return ((to - from) % tau + tau) % tau;
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {number} t
   */
  function lerpPoint(start, end, t) {
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }

  /**
   * @param {Point} point
   * @param {Point} center
   * @param {number} radians
   * @returns {Point}
   */
  function rotateAround(point, center, radians) {
    const cos = Math.cos(radians);
    const sin = Math.sin(radians);
    const dx = point.x - center.x;
    const dy = point.y - center.y;
    return {
      x: center.x + dx * cos + dy * sin,
      y: center.y - dx * sin + dy * cos,
    };
  }

  /**
   * @param {Point} point
   * @param {Point} center
   * @param {number} factor
   * @returns {Point}
   */
  function scaleAround(point, center, factor) {
    return {
      x: center.x + (point.x - center.x) * factor,
      y: center.y + (point.y - center.y) * factor,
    };
  }

  /**
   * @param {Point} point
   * @param {Point} lineStart
   * @param {Point} lineEnd
   * @returns {Point}
   */
  function reflectAcrossLine(point, lineStart, lineEnd) {
    const dx = lineEnd.x - lineStart.x;
    const dy = lineEnd.y - lineStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) return point;
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

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {{ minX: number; maxX: number; minY: number; maxY: number }} bounds
   * @param {boolean} rayOnly
   * @returns {Point[] | null}
   */
  function clipParametricLineToBounds(start, end, bounds, rayOnly) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return null;

    /** @type {Array<{ t: number; point: Point }>} */
    const hits = [];
    /**
     * @param {number} t
     * @param {Point} point
     */
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

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {{ minX: number; maxX: number; minY: number; maxY: number }} bounds
   */
  function clipLineToBounds(start, end, bounds) {
    return clipParametricLineToBounds(start, end, bounds, false);
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {{ minX: number; maxX: number; minY: number; maxY: number }} bounds
   */
  function clipRayToBounds(start, end, bounds) {
    return clipParametricLineToBounds(start, end, bounds, true);
  }

  /**
   * @param {Point} start
   * @param {Point} vertex
   * @param {Point} end
   * @returns {Point | null}
   */
  function angleBisectorDirection(start, vertex, end) {
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
  };
})();
