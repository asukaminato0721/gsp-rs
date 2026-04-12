// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  /** @typedef {{ minX: number; maxX: number; minY: number; maxY: number; spanX?: number; spanY?: number }} ViewBounds */
  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { pointIndex: number }>}
   */
  function hasPointIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { lineIndex: number }>}
   */
  function hasLineIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "lineIndex" in handle && typeof handle.lineIndex === "number";
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
   * @param {Point} start
   * @param {Point} vertex
   * @param {Point} end
   */
  function measuredRotationRadians(start, vertex, end) {
    const firstX = start.x - vertex.x;
    const firstY = vertex.y - start.y;
    const secondX = end.x - vertex.x;
    const secondY = vertex.y - end.y;
    const firstLen = Math.hypot(firstX, firstY);
    const secondLen = Math.hypot(secondX, secondY);
    if (firstLen <= 1e-9 || secondLen <= 1e-9) return null;
    return Math.atan2(firstX * secondY - firstY * secondX, firstX * secondX + firstY * secondY);
  }

  /**
   * @param {Point} point
   * @param {Point} center
   * @param {number} factor
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
   * @param {ViewBounds} bounds
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
   * @param {ViewBounds} bounds
   */
  function clipLineToBounds(start, end, bounds) {
    return clipParametricLineToBounds(start, end, bounds, false);
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {ViewBounds} bounds
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

  /**
   * @param {Point} vertex
   * @param {Point} first
   * @param {Point} second
   * @param {number} shortestLen
   */
  function resolveRightAngleMarkerPoints(vertex, first, second, shortestLen) {
    const side = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
    if (side <= 1e-9) return null;
    return [
      { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
      { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
      { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
    ];
  }

  /**
   * @param {Point} vertex
   * @param {Point} first
   * @param {Point} second
   * @param {number} shortestLen
   * @param {number} cross
   * @param {number} dot
   * @param {number} markerClass
   */
  function resolveArcAngleMarkerPoints(vertex, first, second, shortestLen, cross, dot, markerClass) {
    const classScale = 1 + 0.18 * Math.max(0, (markerClass || 1) - 1);
    const radius = Math.min(Math.max(shortestLen * 0.12, 10), 28) * classScale;
    const clampedRadius = Math.min(radius, shortestLen * 0.42);
    if (clampedRadius <= 1e-9) return null;
    const delta = Math.atan2(cross, dot);
    if (Math.abs(delta) <= 1e-6) return null;
    const startAngle = Math.atan2(first.y, first.x);
    const samples = 9;
    return Array.from({ length: samples }, (_, index) => {
      const t = index / (samples - 1);
      const angle = startAngle + delta * t;
      return {
        x: vertex.x + clampedRadius * Math.cos(angle),
        y: vertex.y + clampedRadius * Math.sin(angle),
      };
    });
  }

  /**
   * @param {Point} start
   * @param {Point} vertex
   * @param {Point} end
   * @param {number} markerClass
   */
  function resolveAngleMarkerPoints(start, vertex, end, markerClass) {
    const firstDx = start.x - vertex.x;
    const firstDy = start.y - vertex.y;
    const secondDx = end.x - vertex.x;
    const secondDy = end.y - vertex.y;
    const firstLen = Math.hypot(firstDx, firstDy);
    const secondLen = Math.hypot(secondDx, secondDy);
    const shortestLen = Math.min(firstLen, secondLen);
    if (firstLen <= 1e-9 || secondLen <= 1e-9 || shortestLen <= 1e-9) return null;
    const first = { x: firstDx / firstLen, y: firstDy / firstLen };
    const second = { x: secondDx / secondLen, y: secondDy / secondLen };
    const dot = Math.max(-1, Math.min(1, first.x * second.x + first.y * second.y));
    const cross = first.x * second.y - first.y * second.x;
    if (Math.abs(dot) <= 0.12) {
      return resolveRightAngleMarkerPoints(vertex, first, second, shortestLen);
    }
    return resolveArcAngleMarkerPoints(vertex, first, second, shortestLen, cross, dot, markerClass);
  }

  /**
   * @param {string} op
   * @param {number} x
   * @returns {number | null}
   */
  function evaluateUnary(op, x) {
    switch (op) {
      case "sin": return Math.sin(x);
      case "cos": return Math.cos(x);
      case "tan": return Math.tan(x);
      case "abs": return Math.abs(x);
      case "sqrt": return x >= 0 ? Math.sqrt(x) : null;
      case "ln": return x > 0 ? Math.log(x) : null;
      case "log10": return x > 0 ? Math.log10(x) : null;
      case "sign": return x > 0 ? 1 : (x < 0 ? -1 : 0);
      case "round": return Math.round(x);
      case "trunc": return Math.trunc(x);
      default: return null;
    }
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {number} x
   * @param {Map<string, number>} parameters
   * @returns {number | null}
   */
  function evaluateExpr(expr, x, parameters) {
    if (expr.kind === "constant") return expr.value;
    if (expr.kind === "identity") return x;
    if (expr.kind !== "parsed") return null;
    return evaluateExprAst(expr.expr, x, parameters);
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {number} x
   * @param {Map<string, number>} parameters
   * @returns {number | null}
   */
  function evaluateExprAst(expr, x, parameters) {
    if (!expr || typeof expr !== "object") return null;
    switch (expr.kind) {
      case "variable":
        return x;
      case "constant":
        return expr.value;
      case "parameter":
        return parameters.get(expr.name) ?? expr.value;
      case "pi-angle":
        return 180;
      case "unary": {
        const inner = evaluateExprAst(expr.expr, x, parameters);
        return inner === null ? null : evaluateUnary(expr.op, inner);
      }
      case "binary": {
        const lhs = evaluateExprAst(expr.lhs, x, parameters);
        const rhs = evaluateExprAst(expr.rhs, x, parameters);
        if (lhs === null || rhs === null) return null;
        const value = expr.op === "add"
          ? lhs + rhs
          : expr.op === "sub"
            ? lhs - rhs
          : expr.op === "mul"
            ? lhs * rhs
          : expr.op === "div"
            ? (Math.abs(rhs) >= 1e-9 ? lhs / rhs : null)
          : Math.pow(lhs, rhs);
        return value === null || !Number.isFinite(value) ? null : value;
      }
      default:
        return null;
    }
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {(value: number) => string} formatAxisNumber
   * @param {string} [variableLabel]
   */
  function formatExpr(expr, formatAxisNumber, variableLabel = "x") {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return variableLabel;
    if (expr.kind === "parsed") {
      return formatExprAst(expr.expr, formatAxisNumber, variableLabel, 0);
    }
    return "?";
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {(value: number) => string} formatAxisNumber
   * @param {string} [variableLabel]
   * @param {number} [parentPrec]
   * @returns {string}
   */
  function formatExprAst(expr, formatAxisNumber, variableLabel = "x", parentPrec = 0) {
    if (!expr || typeof expr !== "object") return "?";
    switch (expr.kind) {
      case "variable":
        return variableLabel;
      case "constant":
        return formatAxisNumber(expr.value);
      case "parameter":
        return expr.name;
      case "pi-angle":
        return "180";
      case "unary": {
        /** @type {string} */
        const inner = formatExprAst(expr.expr, formatAxisNumber, variableLabel, 4);
        if (expr.op === "abs") return `|${inner}|`;
        if (expr.op === "sqrt") {
          return expr.expr?.kind === "binary" ? `√(${inner})` : `√${inner}`;
        }
        return `${expr.op}(${inner})`;
      }
      case "binary": {
        const { prec, rightAssoc } = binaryPrecedence(expr.op);
        /** @type {string} */
        const left = formatExprAst(expr.lhs, formatAxisNumber, variableLabel, prec);
        /** @type {string} */
        const right = formatExprAst(
          expr.rhs,
          formatAxisNumber,
          variableLabel,
          prec + (rightAssoc ? 0 : 1),
        );
        /** @type {string} */
        const text = `${left}${binaryOpText(expr.op)}${right}`;
        return prec < parentPrec ? `(${text})` : text;
      }
      default:
        return "?";
    }
  }

  /** @param {string} op */
  function binaryPrecedence(op) {
    switch (op) {
      case "add":
      case "sub":
        return { prec: 1, rightAssoc: false };
      case "mul":
      case "div":
        return { prec: 2, rightAssoc: false };
      case "pow":
        return { prec: 3, rightAssoc: true };
      default:
        return { prec: 0, rightAssoc: false };
    }
  }

  /** @param {string} op */
  function binaryOpText(op) {
    switch (op) {
      case "add": return " + ";
      case "sub": return " - ";
      case "mul": return "*";
      case "div": return " / ";
      case "pow": return "^";
      default: return " ? ";
    }
  }

  /** @param {ViewerEnv} env */
  /** @param {ViewerEnv} env */
  function parameterMap(env) {
    return new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]));
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {Set<string>} names
   */
  function collectExprParameterNames(expr, names) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parsed") {
      collectExprAstParameterNames(expr.expr, names);
    }
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {Set<string>} names
   */
  function collectExprAstParameterNames(expr, names) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parameter" && typeof expr.name === "string") {
      names.add(expr.name);
      return;
    }
    if (expr.kind === "unary") {
      collectExprAstParameterNames(expr.expr, names);
      return;
    }
    if (expr.kind === "binary") {
      collectExprAstParameterNames(expr.lhs, names);
      collectExprAstParameterNames(expr.rhs, names);
    }
  }

  /**
   * @param {FunctionJson} functionDef
   * @param {Map<string, number>} parameters
   */
  function sampleDynamicFunction(functionDef, parameters) {
    const points = [];
    const last = Math.max(1, functionDef.domain.sampleCount - 1);
    for (let index = 0; index < functionDef.domain.sampleCount; index += 1) {
      const t = index / last;
      const x = functionDef.domain.xMin + (functionDef.domain.xMax - functionDef.domain.xMin) * t;
      const y = evaluateExpr(functionDef.expr, x, parameters);
      if (y === null) continue;
      if (functionDef.domain.plotMode === "polar") {
        points.push({
          x: y * Math.cos(x),
          y: y * Math.sin(x),
        });
      } else {
        points.push({ x, y });
      }
    }
    return points;
  }

  /** @param {number} value */
  function wrapUnitInterval(value) {
    return ((value % 1) + 1) % 1;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function circleParameterFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (constraint?.kind !== "circle") {
      return null;
    }
    const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
    const tau = Math.PI * 2;
    return ((pointAngle % tau) + tau) % tau / tau;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function polygonBoundaryParameterFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (!constraint || constraint.kind !== "polygon-boundary" || constraint.vertexIndices.length < 2) {
      return null;
    }

    const count = constraint.vertexIndices.length;
    let perimeter = 0;
    let traveled = 0;
    for (let index = 0; index < count; index += 1) {
      const start = scene.points[constraint.vertexIndices[index]];
      const end = scene.points[constraint.vertexIndices[(index + 1) % count]];
      if (!start || !end) {
        return null;
      }
      const length = Math.hypot(end.x - start.x, end.y - start.y);
      perimeter += length;
      if (index < constraint.edgeIndex) {
        traveled += length;
      } else if (index === constraint.edgeIndex) {
        traveled += length * Math.max(0, Math.min(1, constraint.t));
      }
    }

    return perimeter > 1e-9 ? traveled / perimeter : null;
  }

  /**
   * @param {Point[]} vertices
   * @param {number} parameter
   * @returns {Point | null}
   */
  function pointOnPolygonBoundary(vertices, parameter) {
    if (!vertices || vertices.length < 2) {
      return null;
    }
    const wrapped = ((parameter % 1) + 1) % 1;
    const lengths = [];
    let perimeter = 0;
    for (let index = 0; index < vertices.length; index += 1) {
      const start = vertices[index];
      const end = vertices[(index + 1) % vertices.length];
      const length = Math.hypot(end.x - start.x, end.y - start.y);
      lengths.push(length);
      perimeter += length;
    }
    if (perimeter <= 1e-9) {
      return null;
    }
    const target = wrapped * perimeter;
    let traveled = 0;
    for (let edgeIndex = 0; edgeIndex < lengths.length; edgeIndex += 1) {
      const length = lengths[edgeIndex];
      if (traveled + length >= target || edgeIndex === lengths.length - 1) {
        const start = vertices[edgeIndex];
        const end = vertices[(edgeIndex + 1) % vertices.length];
        const localT = length <= 1e-9 ? 0 : Math.max(0, Math.min(1, (target - traveled) / length));
        return {
          x: start.x + (end.x - start.x) * localT,
          y: start.y + (end.y - start.y) * localT,
        };
      }
      traveled += length;
    }
    return null;
  }

  /** @type {Record<string, PointConstraintParameterReader>} */
  const POINT_CONSTRAINT_PARAMETER_READERS = {
    segment: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    polyline: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    "polygon-boundary": polygonBoundaryParameterFromPoint,
    circle: circleParameterFromPoint,
    "circle-arc": (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    arc: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
  };

  /** @type {Record<string, PointConstraintParameterApplier>} */
  const POINT_CONSTRAINT_PARAMETER_APPLIERS = {
    segment(point, _scene, wrapped) {
      point.constraint.t = wrapped;
    },
    polyline(point, _scene, wrapped) {
      point.constraint.t = wrapped;
    },
    "polygon-boundary"(point, scene, wrapped) {
      const count = point.constraint.vertexIndices.length;
      if (count < 2) return;
      const lengths = [];
      let perimeter = 0;
      for (let i = 0; i < count; i += 1) {
        const start = scene.points[point.constraint.vertexIndices[i]];
        const end = scene.points[point.constraint.vertexIndices[(i + 1) % count]];
        if (!start || !end) return;
        const length = Math.hypot(end.x - start.x, end.y - start.y);
        lengths.push(length);
        perimeter += length;
      }
      if (perimeter <= 1e-9) return;
      const target = wrapped * perimeter;
      let traveled = 0;
      for (let edgeIndex = 0; edgeIndex < lengths.length; edgeIndex += 1) {
        const length = lengths[edgeIndex];
        if (traveled + length >= target || edgeIndex === lengths.length - 1) {
          point.constraint.edgeIndex = edgeIndex;
          point.constraint.t = length <= 1e-9 ? 0 : Math.max(0, Math.min(1, (target - traveled) / length));
          return;
        }
        traveled += length;
      }
    },
    circle(point, _scene, wrapped) {
      const angle = Math.PI * 2 * wrapped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    },
    "circle-arc"(point, _scene, wrapped) {
      point.constraint.t = wrapped;
    },
    arc(point, _scene, wrapped) {
      point.constraint.t = wrapped;
    },
  };

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function parameterValueFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (!constraint) return null;
    const readParameter = POINT_CONSTRAINT_PARAMETER_READERS[constraint.kind];
    return readParameter ? readParameter(scene, pointIndex) : null;
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {ViewerSceneData} scene
   * @param {number} value
   */
  function applyNormalizedParameterToPoint(point, scene, value) {
    if (!point.constraint) return;
    const wrapped = wrapUnitInterval(value);
    const applyParameter = POINT_CONSTRAINT_PARAMETER_APPLIERS[point.constraint.kind];
    if (applyParameter) {
      applyParameter(point, scene, wrapped);
    }
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {ViewerSceneData} scene
   * @param {number} value
   * @param {number} xMin
   * @param {number} xMax
   */
  function applyTraceValueToPoint(point, scene, value, xMin, xMax) {
    if (!point?.constraint) return;
    if (point.constraint.kind === "circle") {
      point.constraint.unitX = Math.cos(value);
      point.constraint.unitY = -Math.sin(value);
      return;
    }
    const normalized = Math.abs(xMax - xMin) <= 1e-9
      ? 0
      : Math.max(0, Math.min(1, (value - xMin) / (xMax - xMin)));
    applyNormalizedParameterToPoint(point, scene, normalized);
  }

  /**
   * @param {{ depth: number; parameterName?: string | null }} family
   * @param {Map<string, number>} parameters
   */
  function pointIterationDepth(family, parameters) {
    const rawValue = family.parameterName ? parameters.get(family.parameterName) : family.depth;
    const fallback = Number.isFinite(family.depth) ? family.depth : 0;
    const depth = Number.isFinite(rawValue) ? rawValue : fallback;
    return Math.max(0, Math.round(depth));
  }

  /**
   * @param {Point[]} sourceTriangle
   * @param {Point[]} targetTriangle
   */
  function affineMapFromTriangles(sourceTriangle, targetTriangle) {
    const sourceOrigin = sourceTriangle[0];
    const su = {
      x: sourceTriangle[1].x - sourceOrigin.x,
      y: sourceTriangle[1].y - sourceOrigin.y,
    };
    const sv = {
      x: sourceTriangle[2].x - sourceOrigin.x,
      y: sourceTriangle[2].y - sourceOrigin.y,
    };
    const det = su.x * sv.y - su.y * sv.x;
    if (Math.abs(det) <= 1e-9) {
      return null;
    }
    const targetOrigin = targetTriangle[0];
    const tu = {
      x: targetTriangle[1].x - targetOrigin.x,
      y: targetTriangle[1].y - targetOrigin.y,
    };
    const tv = {
      x: targetTriangle[2].x - targetOrigin.x,
      y: targetTriangle[2].y - targetOrigin.y,
    };
    return (/** @type {Point} */ point) => {
      const relative = { x: point.x - sourceOrigin.x, y: point.y - sourceOrigin.y };
      const u = (relative.x * sv.y - relative.y * sv.x) / det;
      const v = (su.x * relative.y - su.y * relative.x) / det;
      return {
        x: targetOrigin.x + tu.x * u + tv.x * v,
        y: targetOrigin.y + tu.y * u + tv.y * v,
      };
    };
  }

  /**
   * @param {Point} sourceStart
   * @param {Point} sourceEnd
   * @param {Point} point
   */
  function segmentPointCoefficients(sourceStart, sourceEnd, point) {
    const dx = sourceEnd.x - sourceStart.x;
    const dy = sourceEnd.y - sourceStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) {
      return null;
    }
    const relativeX = point.x - sourceStart.x;
    const relativeY = point.y - sourceStart.y;
    return {
      alpha: (relativeX * dx + relativeY * dy) / lenSq,
      beta: (relativeX * -dy + relativeY * dx) / lenSq,
    };
  }

  /**
   * @param {Point} segmentStart
   * @param {Point} segmentEnd
   * @param {{ alpha: number; beta: number }} coeffs
   */
  function applySegmentCoefficients(segmentStart, segmentEnd, coeffs) {
    const dx = segmentEnd.x - segmentStart.x;
    const dy = segmentEnd.y - segmentStart.y;
    return {
      x: segmentStart.x + coeffs.alpha * dx - coeffs.beta * dy,
      y: segmentStart.y + coeffs.alpha * dy + coeffs.beta * dx,
    };
  }

  /** @param {number} value */
  function formatSequenceValue(value) {
    if (!Number.isFinite(value)) {
      return "-";
    }
    return Math.abs(value - Math.round(value)) < 0.005
      ? String(Math.round(value))
      : value.toFixed(2);
  }

  /**
   * @param {[number, number, number, number]} color
   * @param {number} amount
   */
  function darken(color, amount) {
    return [
      Math.max(0, color[0] - amount),
      Math.max(0, color[1] - amount),
      Math.max(0, color[2] - amount),
      color[3],
    ];
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {string} parameterName
   * @param {number} currentValue
   * @param {Map<string, number>} parameters
   */
  function evaluateRecursiveExpression(expr, parameterName, currentValue, parameters) {
    const nextParameters = new Map(parameters);
    nextParameters.set(parameterName, currentValue);
    return evaluateExpr(expr, 0, nextParameters);
  }

  /**
   * @param {string} exprLabel
   * @param {number} value
   * @param {(value: number) => string} formatNumber
   */
  function buildExpressionRichMarkup(exprLabel, value, formatNumber) {
    if (typeof exprLabel !== "string") {
      return null;
    }
    const renderPart = (/** @type {string} */ text) => text.split("*").join("\u00b7");
    const additiveFraction = exprLabel.match(/^(.*)\s\+\s(.*)\s\/\s(.*)$/);
    if (additiveFraction) {
      const [, prefix, numerator, denominator] = additiveFraction;
      return `<H<Tx${renderPart(prefix)} + ></<Tx${renderPart(numerator)}><Tx${renderPart(denominator)}>><Tx = ${formatNumber(value)}>>`;
    }
    const parts = exprLabel.split(" / ");
    if (parts.length !== 2) {
      return null;
    }
    return `<H</<Tx${renderPart(parts[0])}><Tx${renderPart(parts[1])}>><Tx = ${formatNumber(value)}>>`;
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIterationPoints(env, scene, parameters) {
    const families = env.sourceScene.pointIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => sum + (family.depth || 0), 0);
    const baseCount = Math.max(0, env.sourceScene.points.length - exportedDepth);
    scene.points = scene.points.slice(0, baseCount);

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (depth <= 0) {
        return;
      }
      if (family.kind === "offset") {
        let previousIndex = family.seedIndex;
        for (let step = 0; step < depth; step += 1) {
          const origin = scene.points[previousIndex];
          if (!origin) {
            break;
          }
          scene.points.push({
            x: origin.x + family.dx,
            y: origin.y + family.dy,
            visible: true,
            constraint: {
              kind: "offset",
              originIndex: previousIndex,
              dx: family.dx,
              dy: family.dy,
            },
            binding: null,
          });
          previousIndex = scene.points.length - 1;
        }
        return;
      }

      if (family.kind === "rotate-chain") {
        const center = scene.points[family.centerIndex];
        let previousIndex = family.seedIndex;
        if (!center) {
          return;
        }
        for (let step = 0; step < depth; step += 1) {
          const source = scene.points[previousIndex];
          if (!source) {
            break;
          }
          const rotated = rotateAround(source, center, family.angleDegrees * Math.PI / 180);
          scene.points.push({
            x: rotated.x,
            y: rotated.y,
            visible: true,
            constraint: null,
            binding: {
              kind: "rotate",
              sourceIndex: previousIndex,
              centerIndex: family.centerIndex,
              angleDegrees: family.angleDegrees,
            },
          });
          previousIndex = scene.points.length - 1;
        }
        return;
      }

      if (family.kind === "rotate") {
        const source = scene.points[family.sourceIndex];
        const center = scene.points[family.centerIndex];
        if (!source || !center) {
          return;
        }
        const angleDegrees = evaluateExpr(family.angleExpr, 0, parameters);
        if (!Number.isFinite(angleDegrees)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const rotated = rotateAround(source, center, (angleDegrees * step) * Math.PI / 180);
          scene.points.push({
            x: rotated.x,
            y: rotated.y,
            visible: true,
            constraint: null,
            binding: {
              kind: "rotate",
              sourceIndex: family.sourceIndex,
              centerIndex: family.centerIndex,
              angleDegrees: angleDegrees * step,
            },
          });
        }
      }
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIteratedLines(env, scene, parameters) {
    const families = env.sourceScene.lineIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      const depth = family.depth || 0;
      if (family.kind === "branching") {
        const branchCount = Array.isArray(family.targetSegments) ? family.targetSegments.length : 0;
        let total = 0;
        let width = branchCount;
        for (let step = 0; step < depth; step += 1) {
          total += width;
          width *= branchCount;
        }
        return sum + total;
      }
      if (family.kind === "affine") {
        return sum + depth;
      }
      if (family.bidirectional) {
        if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
          return sum + (2 * depth * (depth + 1));
        }
        return sum + (2 * depth);
      }
      if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
        return sum + (((depth + 1) * (depth + 2)) / 2 - 1);
      }
      return sum + depth;
    }, 0);
    const baseCount = Math.max(0, env.sourceScene.lines.length - exportedDepth);
    scene.lines = scene.lines.slice(0, baseCount);

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (depth <= 0) {
        return;
      }
      const start = env.resolveScenePoint(family.startIndex);
      const end = env.resolveScenePoint(family.endIndex);
      if (!start || !end) {
        return;
      }
      const resolveHandle = (/** @type {PointHandle} */ handle) => {
        if (hasPointIndexHandle(handle)) {
          return env.resolveScenePoint(handle.pointIndex);
        }
        if (hasLineIndexHandle(handle)) {
          const line = scene.lines[handle.lineIndex];
          if (!line?.points || line.points.length < 2) return null;
          const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
          const t = typeof handle.t === "number" ? handle.t : 0.5;
          const p0 = line.points[segmentIndex];
          const p1 = line.points[segmentIndex + 1];
          return {
            x: p0.x + (p1.x - p0.x) * t,
            y: p0.y + (p1.y - p0.y) * t,
          };
        }
        return /** @type {Point} */ (handle);
      };
      if (family.kind === "branching") {
        const targetSegments = (family.targetSegments || []).map((segment) => [
          resolveHandle(segment[0]),
          resolveHandle(segment[1]),
        ]);
        if (targetSegments.some((segment) => segment.some((point) => !point))) {
          return;
        }
        const coeffs = targetSegments
          .map((segment) => {
            const startCoeffs = segmentPointCoefficients(start, end, segment[0]);
            const endCoeffs = segmentPointCoefficients(start, end, segment[1]);
            if (!startCoeffs || !endCoeffs) {
              return null;
            }
            return { startCoeffs, endCoeffs };
          })
          .filter(Boolean);
        if (coeffs.length === 0) {
          return;
        }
        /** @type {{ start: Point; end: Point }[]} */
        let frontier = [{ start: { ...start }, end: { ...end } }];
        for (let step = 0; step < depth; step += 1) {
          /** @type {{ start: Point; end: Point }[]} */
          const next = [];
          frontier.forEach((segment) => {
            coeffs.forEach((coeff) => {
              const childStart = applySegmentCoefficients(segment.start, segment.end, coeff.startCoeffs);
              const childEnd = applySegmentCoefficients(segment.start, segment.end, coeff.endCoeffs);
              scene.lines.push({
                points: [{ ...childStart }, { ...childEnd }],
                color: family.color,
                dashed: !!family.dashed,
                binding: null,
              });
              next.push({ start: childStart, end: childEnd });
            });
          });
          frontier = next;
        }
        return;
      }
      if (family.kind === "affine") {
        const sourceTriangle = family.sourceTriangleIndices.map((index) => env.resolveScenePoint(index));
        const targetTriangle = family.targetTriangle.map((handle) => resolveHandle(handle));
        if (sourceTriangle.some((point) => !point) || targetTriangle.some((point) => !point)) {
          return;
        }
        const mapPoint = affineMapFromTriangles(sourceTriangle, targetTriangle);
        if (!mapPoint) {
          return;
        }
        let currentStart = { ...start };
        let currentEnd = { ...end };
        for (let step = 0; step < depth; step += 1) {
          currentStart = mapPoint(currentStart);
          currentEnd = mapPoint(currentEnd);
          scene.lines.push({
            points: [{ ...currentStart }, { ...currentEnd }],
            color: family.color,
            dashed: !!family.dashed,
            binding: null,
          });
        }
        return;
      }
      if (family.kind !== "translate") return;
      const hasSecondary = Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy);
      const deltas = [];
      if (family.bidirectional && hasSecondary) {
        for (let primary = -depth; primary <= depth; primary += 1) {
          for (let secondary = -depth; secondary <= depth; secondary += 1) {
            if (primary === 0 && secondary === 0) {
              continue;
            }
            if (Math.abs(primary) + Math.abs(secondary) > depth) {
              continue;
            }
            deltas.push({
              dx: family.dx * primary + family.secondaryDx * secondary,
              dy: family.dy * primary + family.secondaryDy * secondary,
            });
          }
        }
      } else if (family.bidirectional) {
        for (let step = 1; step <= depth; step += 1) {
          deltas.push(
            { dx: family.dx * step, dy: family.dy * step },
            { dx: -family.dx * step, dy: -family.dy * step },
          );
        }
      } else if (hasSecondary) {
        for (let primary = 0; primary <= depth; primary += 1) {
          for (let secondary = 0; secondary <= depth - primary; secondary += 1) {
            if (primary === 0 && secondary === 0) {
              continue;
            }
            deltas.push({
              dx: family.dx * primary + family.secondaryDx * secondary,
              dy: family.dy * primary + family.secondaryDy * secondary,
            });
          }
        }
      } else {
        for (let step = 1; step <= depth; step += 1) {
          deltas.push({
            dx: family.dx * step,
            dy: family.dy * step,
          });
        }
      }
      deltas.forEach(({ dx, dy }) => {
        scene.lines.push({
          points: [
            { x: start.x + dx, y: start.y + dy },
            { x: end.x + dx, y: end.y + dy },
          ],
          color: family.color,
          dashed: !!family.dashed,
          binding: null,
        });
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIteratedPolygons(env, scene, parameters) {
    const families = env.sourceScene.polygonIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      const depth = family.depth || 0;
      if (family.bidirectional) {
        if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
          return sum + (1 + 2 * depth * (depth + 1));
        }
        return sum + (1 + 2 * depth);
      }
      if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
        return sum + (((depth + 1) * (depth + 2)) / 2);
      }
      return sum + (depth + 1);
    }, 0);
    const baseCount = Math.max(0, env.sourceScene.polygons.length - exportedDepth);
    scene.polygons = scene.polygons.slice(0, baseCount);

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (family.kind !== "translate" || family.vertexIndices.length < 3) {
        return;
      }
      const seedVertices = family.vertexIndices
        .map((index) => env.resolveScenePoint(index));
      if (seedVertices.some((point) => !point)) {
        return;
      }
      const hasSecondary = Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy);
      const deltas = [];
      if (family.bidirectional && hasSecondary) {
        for (let primary = -depth; primary <= depth; primary += 1) {
          for (let secondary = -depth; secondary <= depth; secondary += 1) {
            if (Math.abs(primary) + Math.abs(secondary) > depth) {
              continue;
            }
            deltas.push({
              dx: family.dx * primary + family.secondaryDx * secondary,
              dy: family.dy * primary + family.secondaryDy * secondary,
            });
          }
        }
      } else if (family.bidirectional) {
        deltas.push({ dx: 0, dy: 0 });
        for (let step = 1; step <= depth; step += 1) {
          deltas.push(
            { dx: family.dx * step, dy: family.dy * step },
            { dx: -family.dx * step, dy: -family.dy * step },
          );
        }
      } else if (hasSecondary) {
        for (let primary = 0; primary <= depth; primary += 1) {
          for (let secondary = 0; secondary <= depth - primary; secondary += 1) {
            deltas.push({
              dx: family.dx * primary + family.secondaryDx * secondary,
              dy: family.dy * primary + family.secondaryDy * secondary,
            });
          }
        }
      } else {
        for (let step = 0; step <= depth; step += 1) {
          deltas.push({
            dx: family.dx * step,
            dy: family.dy * step,
          });
        }
      }
      deltas.forEach(({ dx, dy }) => {
        scene.polygons.push({
          points: seedVertices.map((point) => ({ x: point.x + dx, y: point.y + dy })),
          color: family.color,
          outlineColor: darken(family.color, 80),
          binding: null,
        });
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIteratedLabels(env, scene, parameters) {
    const families = env.sourceScene.labelIterations || [];
    if (families.length === 0) {
      return;
    }
    const baseCount = env.sourceScene.labels.length;
    scene.labels = scene.labels.slice(0, baseCount);

    families.forEach((family) => {
      if (family.kind !== "point-expression") {
        return;
      }
      const seedLabel = scene.labels[family.seedLabelIndex];
      const seedAnchor = seedLabel?.anchor;
      if (!seedLabel || typeof seedAnchor?.pointIndex !== "number") {
        return;
      }
      const depth = pointIterationDepth({
        depth: family.depth,
        parameterName: family.depthParameterName,
      }, parameters);
      let currentValue = parameters.get(family.parameterName);
      if (!Number.isFinite(currentValue)) {
        return;
      }
      for (let step = 0; step <= depth; step += 1) {
        const value = evaluateRecursiveExpression(
          family.expr,
          family.parameterName,
          currentValue,
          parameters,
        );
        if (!Number.isFinite(value)) {
          break;
        }
        const pointIndex = family.pointSeedIndex + step;
        if (!scene.points[pointIndex]) {
          break;
        }
        if (step === 0) {
          seedLabel.text = formatSequenceValue(value);
          seedLabel.anchor = { ...seedAnchor, pointIndex };
        } else {
          scene.labels.push({
            ...seedLabel,
            text: formatSequenceValue(value),
            binding: null,
            anchor: { ...seedAnchor, pointIndex },
          });
        }
        currentValue = value;
      }
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIterationTables(env, scene, parameters) {
    const sourceTables = env.sourceScene.iterationTables || [];
    const currentTables = scene.iterationTables || [];
    scene.iterationTables = sourceTables.map((table, index) => {
      const current = currentTables[index];
      const depth = table.depthParameterName
        ? Math.max(0, Math.round(parameters.get(table.depthParameterName) ?? table.depth) - 1)
        : Math.max(0, Math.round(table.depth));
      let currentValue = parameters.get(table.parameterName);
      const rows = [];
      if (Number.isFinite(currentValue)) {
        for (let index = 0; index <= depth; index += 1) {
          const value = evaluateRecursiveExpression(
            table.expr,
            table.parameterName,
            currentValue,
            parameters,
          );
          if (!Number.isFinite(value)) {
            break;
          }
          rows.push({ index, value });
          currentValue = value;
        }
      }
      return {
        ...table,
        x: Number.isFinite(current?.x) ? current.x : table.x,
        y: Number.isFinite(current?.y) ? current.y : table.y,
        rows,
      };
    });
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {Point | null} source
   * @param {Map<string, number>} parameters
   */
  function updateCoordinateSourcePoint(point, source, parameters) {
    if (!source) return;
    const exprParameters = new Map(parameters);
    exprParameters.set(point.binding.name, parameters.get(point.binding.name));
    const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
    if (offset === null) return;
    if (point.binding.axis === "horizontal") {
      point.x = source.x + offset;
      point.y = source.y;
      return;
    }
    point.x = source.x;
    point.y = source.y + offset;
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {Point | null} source
   * @param {Map<string, number>} parameters
   */
  function updateCoordinateSource2dPoint(point, source, parameters) {
    if (!source) return;
    const exprParameters = new Map(parameters);
    exprParameters.set(point.binding.xName, parameters.get(point.binding.xName));
    exprParameters.set(point.binding.yName, parameters.get(point.binding.yName));
    const dx = evaluateExpr(point.binding.xExpr, 0, exprParameters);
    const dy = evaluateExpr(point.binding.yExpr, 0, exprParameters);
    if (dx !== null && dy !== null) {
      point.x = source.x + dx;
      point.y = source.y + dy;
    }
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {Map<string, number>} parameters
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   * @param {ViewerSceneData} parameterSourceScene
   */
  function updateCustomTransformPoint(point, parameters, resolvePointAt, parameterSourceScene) {
    const value = parameterValueFromPoint(parameterSourceScene, point.binding.sourceIndex);
    if (!Number.isFinite(value)) return;
    const exprParameters = new Map(parameters);
    const names = new Set();
    collectExprParameterNames(point.binding.distanceExpr, names);
    collectExprParameterNames(point.binding.angleExpr, names);
    names.forEach((name) => exprParameters.set(name, value));
    const distanceValue = evaluateExpr(point.binding.distanceExpr, value, exprParameters);
    const angleValue = evaluateExpr(point.binding.angleExpr, value, exprParameters);
    const origin = resolvePointAt(point.binding.originIndex);
    const axisEnd = resolvePointAt(point.binding.axisEndIndex);
    if (distanceValue === null || angleValue === null || !origin || !axisEnd) return;
    const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
    const radians = (baseAngle + angleValue * point.binding.angleDegreesScale) * Math.PI / 180;
    const distance = distanceValue * point.binding.distanceRawScale;
    point.x = origin.x + distance * Math.cos(radians);
    point.y = origin.y - distance * Math.sin(radians);
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   */
  function updateScaleByRatioPoint(point, resolvePointAt) {
    const source = resolvePointAt(point.binding.sourceIndex);
    const center = resolvePointAt(point.binding.centerIndex);
    const ratioOrigin = resolvePointAt(point.binding.ratioOriginIndex);
    const ratioDenominator = resolvePointAt(point.binding.ratioDenominatorIndex);
    const ratioNumerator = resolvePointAt(point.binding.ratioNumeratorIndex);
    if (!source || !center || !ratioOrigin || !ratioDenominator || !ratioNumerator) return;
    const denominator = Math.hypot(
      ratioDenominator.x - ratioOrigin.x,
      ratioDenominator.y - ratioOrigin.y,
    );
    if (denominator <= 1e-9) return;
    const numerator = Math.hypot(
      ratioNumerator.x - ratioOrigin.x,
      ratioNumerator.y - ratioOrigin.y,
    );
    const scaled = scaleAround(source, center, numerator / denominator);
    point.x = scaled.x;
    point.y = scaled.y;
  }

  /**
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   * @param {ViewBounds} bounds
   * @param {LineConstraintJson} constraint
   * @returns {Point[] | null}
   */
  function resolveLineConstraintPoints(resolvePointAt, bounds, constraint) {
    if (!constraint) return null;
    if (constraint.kind === "segment") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? [start, end] : null;
    }
    if (constraint.kind === "line") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? clipParametricLineToBounds(start, end, bounds, false) : null;
    }
    if (constraint.kind === "ray") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? clipParametricLineToBounds(start, end, bounds, true) : null;
    }
    if (constraint.kind === "perpendicular-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x - dy / len, y: through.y + dx / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "parallel-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x + dx / len, y: through.y + dy / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolvePointAt(constraint.startIndex);
      const vertex = resolvePointAt(constraint.vertexIndex);
      const end = resolvePointAt(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      if (!direction) return null;
      return clipParametricLineToBounds(
        vertex,
        { x: vertex.x + direction.x, y: vertex.y + direction.y },
        bounds,
        true,
      );
    }
    if (constraint.kind === "translated") {
      /** @type {Point[] | null} */
      const source = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      const vectorStart = resolvePointAt(constraint.vectorStartIndex);
      const vectorEnd = resolvePointAt(constraint.vectorEndIndex);
      if (!source || !vectorStart || !vectorEnd) return null;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      return source.map((/** @type {Point} */ point) => ({ x: point.x + dx, y: point.y + dy }));
    }
    return null;
  }

  /** @type {Record<string, PointBindingRefresher>} */
  const DERIVED_POINT_BINDING_REFRESHERS = {
    "derived-parameter"(env, scene, point) {
      const value = parameterValueFromPoint(scene, point.binding.sourceIndex);
      if (value !== null) {
        applyNormalizedParameterToPoint(point, scene, value);
      }
    },
    "derived-parameter-expr"(_env, scene, point, parameters) {
      const sourceValue = parameterValueFromPoint(scene, point.binding.sourceIndex);
      if (sourceValue === null) return;
      const exprParameters = new Map(parameters);
      exprParameters.set(point.binding.parameterName, sourceValue);
      const delta = evaluateExpr(point.binding.expr, 0, exprParameters);
      if (delta !== null) {
        applyNormalizedParameterToPoint(point, scene, sourceValue + delta);
      }
    },
    "coordinate-source"(env, _scene, point, parameters) {
      updateCoordinateSourcePoint(point, env.resolveScenePoint(point.binding.sourceIndex), parameters);
    },
    "coordinate-source-2d"(env, _scene, point, parameters) {
      updateCoordinateSource2dPoint(point, env.resolveScenePoint(point.binding.sourceIndex), parameters);
    },
    translate(env, _scene, point) {
      const source = env.resolveScenePoint(point.binding.sourceIndex);
      const vectorStart = env.resolveScenePoint(point.binding.vectorStartIndex);
      const vectorEnd = env.resolveScenePoint(point.binding.vectorEndIndex);
      if (!source || !vectorStart || !vectorEnd) return;
      point.x = source.x + (vectorEnd.x - vectorStart.x);
      point.y = source.y + (vectorEnd.y - vectorStart.y);
    },
    reflect(env, _scene, point) {
      const source = env.resolveScenePoint(point.binding.sourceIndex);
      const lineStart = env.resolveScenePoint(point.binding.lineStartIndex);
      const lineEnd = env.resolveScenePoint(point.binding.lineEndIndex);
      if (!source || !lineStart || !lineEnd) return;
      const reflected = reflectAcrossLine(source, lineStart, lineEnd);
      point.x = reflected.x;
      point.y = reflected.y;
    },
    "reflect-line-constraint"(env, _scene, point) {
      const source = env.resolveScenePoint(point.binding.sourceIndex);
      const line = resolveLineConstraintPoints(
        (/** @type {number} */ index) => env.resolveScenePoint(index),
        env.getViewBounds ? env.getViewBounds() : env.sourceScene.bounds,
        point.binding.line,
      );
      if (!source || !line) return;
      const reflected = reflectAcrossLine(source, line[0], line[1]);
      point.x = reflected.x;
      point.y = reflected.y;
    },
    rotate(env, _scene, point, parameters) {
      const source = env.resolveScenePoint(point.binding.sourceIndex);
      const center = env.resolveScenePoint(point.binding.centerIndex);
      if (!source || !center) return;
      const angleDegrees = point.binding.parameterName
        ? parameters.get(point.binding.parameterName)
        : point.binding.angleDegrees;
      if (!Number.isFinite(angleDegrees)) return;
      const rotated = rotateAround(source, center, angleDegrees * Math.PI / 180);
      point.x = rotated.x;
      point.y = rotated.y;
    },
    "scale-by-ratio"(env, _scene, point) {
      updateScaleByRatioPoint(point, (/** @type {number} */ index) => env.resolveScenePoint(index));
    },
    scale(env, _scene, point) {
      const source = env.resolveScenePoint(point.binding.sourceIndex);
      const center = env.resolveScenePoint(point.binding.centerIndex);
      if (!source || !center) return;
      const scaled = scaleAround(source, center, point.binding.factor);
      point.x = scaled.x;
      point.y = scaled.y;
    },
    "custom-transform"(_env, scene, point, parameters) {
      updateCustomTransformPoint(point, parameters, (/** @type {number} */ index) => scene.points[index], scene);
    },
  };

  /** @type {Record<string, DynamicLabelRefresher>} */
  const DYNAMIC_LABEL_REFRESHERS = {
    "parameter-value"(env, _scene, label, parameters) {
      const value = parameters.get(label.binding.name);
      if (value !== null && value !== undefined) {
        label.text = `${label.binding.name} = ${env.formatNumber(value)}`;
      }
    },
    "point-expression-value"(_env, _scene, label, parameters) {
      const currentValue = parameters.get(label.binding.parameterName);
      if (!Number.isFinite(currentValue)) return;
      const value = evaluateRecursiveExpression(
        label.binding.expr,
        label.binding.parameterName,
        currentValue,
        parameters,
      );
      if (value !== null) {
        label.text = formatSequenceValue(value);
      }
    },
    "expression-value"(env, _scene, label, parameters) {
      const value = evaluateExpr(label.binding.expr, 0, parameters);
      if (value !== null) {
        label.richMarkup = buildExpressionRichMarkup(
          label.binding.exprLabel,
          value,
          env.formatNumber,
        );
        label.text = label.binding.exprLabel === "360° / n"
          ? `360°\n——— = ${env.formatNumber(value)}°\n  n`
          : `${label.binding.exprLabel} = ${env.formatNumber(value)}`;
      } else {
        label.richMarkup = null;
      }
    },
    "polygon-boundary-parameter"(env, scene, label) {
      const value = polygonBoundaryParameterFromPoint(scene, label.binding.pointIndex);
      if (value !== null) {
        label.text = label.binding.polygonName
          ? `${label.binding.pointName}在${label.binding.polygonName}上的t值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
      }
    },
    "polygon-boundary-expression"(env, scene, label, parameters) {
      const parameterValue = polygonBoundaryParameterFromPoint(scene, label.binding.pointIndex);
      if (parameterValue === null) return;
      const exprParameters = new Map(parameters);
      exprParameters.set(label.binding.parameterName, parameterValue);
      const value = evaluateExpr(label.binding.expr, 0, exprParameters);
      if (value !== null) {
        label.richMarkup = buildExpressionRichMarkup(
          label.binding.exprLabel,
          value,
          env.formatNumber,
        );
        label.text = `${label.binding.exprLabel} = ${env.formatNumber(value)}`;
      } else {
        label.richMarkup = null;
      }
    },
    "segment-parameter"(env, scene, label) {
      const point = scene.points[label.binding.pointIndex];
      const value = point?.constraint?.kind === "segment" ? point.constraint.t : null;
      if (value !== null) {
        label.text = `${label.binding.pointName}在${label.binding.segmentName}上的t值 = ${env.formatNumber(value)}`;
      }
    },
    "circle-parameter"(env, scene, label) {
      const point = scene.points[label.binding.pointIndex];
      const constraint = point?.constraint;
      if (constraint?.kind !== "circle") return;
      const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
      const tau = Math.PI * 2;
      const value = ((pointAngle % tau) + tau) % tau / tau;
      label.text = `${label.binding.pointName}在⊙${label.binding.circleName}上的值 = ${env.formatNumber(value)}`;
    },
    "angle-marker-value"(_env, scene, label) {
      const start = scene.points[label.binding.startIndex];
      const vertex = scene.points[label.binding.vertexIndex];
      const end = scene.points[label.binding.endIndex];
      if (!start || !vertex || !end) return;
      const first = { x: start.x - vertex.x, y: start.y - vertex.y };
      const second = { x: end.x - vertex.x, y: end.y - vertex.y };
      const firstLen = Math.hypot(first.x, first.y);
      const secondLen = Math.hypot(second.x, second.y);
      if (firstLen <= 1e-9 || secondLen <= 1e-9) return;
      const cross = (first.x / firstLen) * (second.y / secondLen)
        - (first.y / firstLen) * (second.x / secondLen);
      const dot = (first.x / firstLen) * (second.x / secondLen)
        + (first.y / firstLen) * (second.y / secondLen);
      const value = Math.abs(Math.atan2(cross, dot)) * 180 / Math.PI;
      if (Number.isFinite(value)) {
        label.text = value.toFixed(label.binding.decimals);
      }
    },
    "custom-transform-value"(env, scene, label, parameters) {
      const value = parameterValueFromPoint(scene, label.binding.pointIndex);
      if (!Number.isFinite(value)) return;
      const exprParameters = new Map(parameters);
      const names = new Set();
      collectExprParameterNames(label.binding.expr, names);
      names.forEach((name) => exprParameters.set(name, value));
      const evaluated = evaluateExpr(label.binding.expr, value, exprParameters);
      if (evaluated !== null) {
        label.text = `${label.binding.exprLabel} = ${env.formatNumber(evaluated * label.binding.valueScale)}${label.binding.valueSuffix}`;
      }
    },
  };

  /** @type {Record<string, PointBindingRefresher>} */
  const SYNC_DYNAMIC_POINT_BINDING_UPDATERS = {
    coordinate(_env, draft, point, parameters) {
      const value = parameters.get(point.binding.name);
      if (!Number.isFinite(value)) return;
      point.x = value;
      const y = evaluateExpr(point.binding.expr, 0, parameters);
      if (y !== null) {
        point.y = y;
      }
    },
    "coordinate-source"(_env, draft, point, parameters) {
      updateCoordinateSourcePoint(point, draft.points[point.binding.sourceIndex], parameters);
    },
    "coordinate-source-2d"(_env, draft, point, parameters) {
      updateCoordinateSource2dPoint(point, draft.points[point.binding.sourceIndex], parameters);
    },
    "custom-transform"(_env, draft, point, parameters) {
      updateCustomTransformPoint(point, parameters, (index) => draft.points[index], draft);
    },
    "scale-by-ratio"(_env, draft, point) {
      updateScaleByRatioPoint(point, (index) => draft.points[index]);
    },
    "derived-parameter-expr"(_env, draft, point, parameters) {
      const sourceValue = parameterValueFromPoint(draft, point.binding.sourceIndex);
      if (!Number.isFinite(sourceValue)) return;
      const exprParameters = new Map(parameters);
      exprParameters.set(point.binding.parameterName, sourceValue);
      const delta = evaluateExpr(point.binding.expr, 0, exprParameters);
      if (delta !== null) {
        applyNormalizedParameterToPoint(point, draft, sourceValue + delta);
      }
    },
  };

  /**
   * @param {ViewerSceneData} scene
   * @param {LineBindingJson} binding
   * @returns {Point[] | null}
   */
  function resolveHostLinePoints(scene, binding) {
    const hostBinding = /** @type {{ lineStartIndex?: number | null; lineEndIndex?: number | null; lineIndex?: number | null }} */ (binding);
    if (
      typeof hostBinding?.lineStartIndex === "number"
      && typeof hostBinding?.lineEndIndex === "number"
    ) {
      const start = scene.points[hostBinding.lineStartIndex];
      const end = scene.points[hostBinding.lineEndIndex];
      return start && end ? [start, end] : null;
    }
    if (typeof hostBinding?.lineIndex === "number") {
      const hostLine = scene.lines[hostBinding.lineIndex];
      return hostLine?.points?.length >= 2 ? hostLine.points : null;
    }
    return null;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {RuntimeLineJson} line
   * @param {Map<string, number>} parameters
   * @returns {Point[] | null}
   */
  function sampleCustomTransformTraceLine(scene, line, parameters) {
    const point = scene.points[line.binding.pointIndex];
    const binding = point?.binding;
    if (binding?.kind !== "custom-transform") return null;
    const origin = scene.points[binding.originIndex];
    const axisEnd = scene.points[binding.axisEndIndex];
    const traceMax = parameterValueFromPoint(scene, binding.sourceIndex);
    if (!origin || !axisEnd || !Number.isFinite(traceMax)) return null;
    const sampled = [];
    const last = Math.max(1, line.binding.sampleCount - 1);
    const maxValue = Math.max(line.binding.xMin, Math.min(line.binding.xMax, traceMax));
    for (let index = 0; index < line.binding.sampleCount; index += 1) {
      const value = line.binding.xMin + (maxValue - line.binding.xMin) * (index / last);
      const exprParameters = new Map(parameters);
      const names = new Set();
      collectExprParameterNames(binding.distanceExpr, names);
      collectExprParameterNames(binding.angleExpr, names);
      names.forEach((name) => exprParameters.set(name, value));
      const distanceValue = evaluateExpr(binding.distanceExpr, value, exprParameters);
      const angleValue = evaluateExpr(binding.angleExpr, value, exprParameters);
      if (distanceValue === null || angleValue === null) continue;
      const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
      const radians = (baseAngle + angleValue * binding.angleDegreesScale) * Math.PI / 180;
      const distance = distanceValue * binding.distanceRawScale;
      sampled.push({
        x: origin.x + distance * Math.cos(radians),
        y: origin.y - distance * Math.sin(radians),
      });
    }
    return sampled.length >= 2 ? sampled : null;
  }

  /** @param {Point} point */
  function cloneTracePoint(point) {
    if (typeof structuredClone === "function") {
      return structuredClone(point);
    }
    return JSON.parse(JSON.stringify(point));
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {RuntimeLineJson} line
   * @param {Map<string, number>} parameters
   * @returns {Point[] | null}
   */
  function samplePointTraceLine(scene, line, parameters) {
    const driver = scene.points[line.binding.driverIndex];
    if (!driver?.constraint) return null;
    const sampleScene = {
      ...scene,
      lines: scene.lines,
      circles: scene.circles,
      /** @type {RuntimeScenePointJson[]} */
      points: [],
    };

    /**
     * @param {RuntimeScenePointJson[]} points
     * @param {number} index
     * @param {Set<number>} [visiting]
     * @returns {Point | null}
     */
    const resolveTracePoint = (points, index, visiting = new Set()) => {
      if (visiting.has(index)) return null;
      const point = points[index];
      if (!point) return null;
      visiting.add(index);

      let resolved = null;
      if (point.binding?.kind === "translate") {
        /** @type {Point | null} */
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        /** @type {Point | null} */
        const vectorStart = resolveTracePoint(points, point.binding.vectorStartIndex, visiting);
        /** @type {Point | null} */
        const vectorEnd = resolveTracePoint(points, point.binding.vectorEndIndex, visiting);
        if (source && vectorStart && vectorEnd) {
          resolved = {
            x: source.x + (vectorEnd.x - vectorStart.x),
            y: source.y + (vectorEnd.y - vectorStart.y),
          };
        }
      } else if (point.binding?.kind === "reflect") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const lineStart = resolveTracePoint(points, point.binding.lineStartIndex, visiting);
        const lineEnd = resolveTracePoint(points, point.binding.lineEndIndex, visiting);
        if (source && lineStart && lineEnd) {
          resolved = reflectAcrossLine(source, lineStart, lineEnd);
        }
      } else if (point.binding?.kind === "reflect-line-constraint") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const line = resolveLineConstraintPoints(
          (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          scene.bounds,
          point.binding.line,
        );
        if (source && line) {
          resolved = reflectAcrossLine(source, line[0], line[1]);
        }
      } else if (point.binding?.kind === "rotate") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const center = resolveTracePoint(points, point.binding.centerIndex, visiting);
        const angleDegrees = point.binding.parameterName
          ? parameters.get(point.binding.parameterName)
          : point.binding.angleDegrees;
        if (source && center && Number.isFinite(angleDegrees)) {
          resolved = rotateAround(source, center, angleDegrees * Math.PI / 180);
        }
      } else if (point.binding?.kind === "scale-by-ratio") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const center = resolveTracePoint(points, point.binding.centerIndex, visiting);
        const ratioOrigin = resolveTracePoint(points, point.binding.ratioOriginIndex, visiting);
        const ratioDenominator = resolveTracePoint(points, point.binding.ratioDenominatorIndex, visiting);
        const ratioNumerator = resolveTracePoint(points, point.binding.ratioNumeratorIndex, visiting);
        const denominator = ratioOrigin && ratioDenominator
          ? Math.hypot(ratioDenominator.x - ratioOrigin.x, ratioDenominator.y - ratioOrigin.y)
          : null;
        const numerator = ratioOrigin && ratioNumerator
          ? Math.hypot(ratioNumerator.x - ratioOrigin.x, ratioNumerator.y - ratioOrigin.y)
          : null;
        if (source && center && Number.isFinite(denominator) && denominator > 1e-9 && Number.isFinite(numerator)) {
          resolved = scaleAround(source, center, numerator / denominator);
        }
      } else if (point.binding?.kind === "scale") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const center = resolveTracePoint(points, point.binding.centerIndex, visiting);
        if (source && center) {
          resolved = scaleAround(source, center, point.binding.factor);
        }
      } else if (point.binding?.kind === "midpoint") {
        const start = resolveTracePoint(points, point.binding.startIndex, visiting);
        const end = resolveTracePoint(points, point.binding.endIndex, visiting);
        if (start && end) {
          resolved = lerpPoint(start, end, 0.5);
        }
      } else if (point.binding?.kind === "custom-transform") {
        const derived = { ...point };
        updateCustomTransformPoint(derived, parameters, (pointIndex) => resolveTracePoint(points, pointIndex, visiting), sampleScene);
        if (Number.isFinite(derived.x) && Number.isFinite(derived.y)) {
          resolved = { x: derived.x, y: derived.y };
        }
      }

      if (!resolved && point.constraint) {
        sampleScene.points = points;
        resolved = window.GspViewerModules.scene.resolveConstrainedPoint(
          {
            sourceScene: scene,
            currentScene: () => sampleScene,
            resolveScenePoint: (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          },
          point.constraint,
          (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          point,
        );
      }

      visiting.delete(index);
      if (resolved) return resolved;
      return point.constraint ? null : point;
    };

    const sampled = [];
    const last = Math.max(1, line.binding.sampleCount - 1);
    for (let index = 0; index < line.binding.sampleCount; index += 1) {
      const value = line.binding.xMin + (line.binding.xMax - line.binding.xMin) * (index / last);
      const points = scene.points.map(cloneTracePoint);
      sampleScene.points = points;
      applyTraceValueToPoint(
        points[line.binding.driverIndex],
        sampleScene,
        value,
        line.binding.xMin,
        line.binding.xMax,
      );
      const point = resolveTracePoint(points, line.binding.pointIndex);
      if (point) {
        sampled.push({ x: point.x, y: point.y });
      }
    }
    return sampled.length >= 2 ? sampled : null;
  }

  /** @type {Record<string, LineBindingRefresher>} */
  const LINE_BINDING_REFRESHERS = {
    segment({ scene }, line) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      if (start && end) {
        line.points = [{ x: start.x, y: start.y }, { x: end.x, y: end.y }];
      }
    },
    "angle-marker"({ scene }, line) {
      const start = scene.points[line.binding.startIndex];
      const vertex = scene.points[line.binding.vertexIndex];
      const end = scene.points[line.binding.endIndex];
      const points = start && vertex && end
        ? resolveAngleMarkerPoints(start, vertex, end, line.binding.markerClass)
        : null;
      if (points) {
        line.points = points;
      }
    },
    "angle-bisector-ray"({ scene, bounds }, line) {
      const start = scene.points[line.binding.startIndex];
      const vertex = scene.points[line.binding.vertexIndex];
      const end = scene.points[line.binding.endIndex];
      if (start && vertex && end) {
        const direction = angleBisectorDirection(start, vertex, end);
        const clipped = direction
          ? clipRayToBounds(
              vertex,
              { x: vertex.x + direction.x, y: vertex.y + direction.y },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    "perpendicular-line"({ scene, bounds }, line) {
      const through = scene.points[line.binding.throughIndex];
      const hostLine = resolveHostLinePoints(scene, line.binding);
      const lineStart = hostLine?.[0];
      const lineEnd = hostLine?.[1];
      if (through && lineStart && lineEnd) {
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        const clipped = len > 1e-9
          ? clipLineToBounds(
              through,
              { x: through.x - dy / len, y: through.y + dx / len },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    "parallel-line"({ scene, bounds }, line) {
      const through = scene.points[line.binding.throughIndex];
      const hostLine = resolveHostLinePoints(scene, line.binding);
      const lineStart = hostLine?.[0];
      const lineEnd = hostLine?.[1];
      if (through && lineStart && lineEnd) {
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        const clipped = len > 1e-9
          ? clipLineToBounds(
              through,
              { x: through.x + dx / len, y: through.y + dy / len },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    line({ scene, bounds }, line) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      const clipped = start && end ? clipLineToBounds(start, end, bounds) : null;
      if (clipped) line.points = clipped;
    },
    ray({ scene, bounds }, line) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      const clipped = start && end ? clipRayToBounds(start, end, bounds) : null;
      if (clipped) line.points = clipped;
    },
    "arc-boundary"({ env }, line) {
      const sampled = window.GspViewerModules.scene.sampleArcBoundaryPoints(env, line.binding);
      if (sampled) {
        line.points = sampled;
      }
    },
    "rotate-line"({ scene, parameters }, line) {
      const source = scene.lines[line.binding.sourceIndex];
      const center = scene.points[line.binding.centerIndex];
      if (source && center) {
        const angleDegrees = line.binding.parameterName
          ? parameters.get(line.binding.parameterName)
          : line.binding.angleDegrees;
        if (!Number.isFinite(angleDegrees)) return;
        const radians = angleDegrees * Math.PI / 180;
        line.points = source.points.map((/** @type {Point} */ point) => rotateAround(point, center, radians));
      }
    },
    "scale-line"({ scene }, line) {
      const source = scene.lines[line.binding.sourceIndex];
      const center = scene.points[line.binding.centerIndex];
      if (source && center) {
        line.points = source.points.map((/** @type {Point} */ point) => scaleAround(point, center, line.binding.factor));
      }
    },
    "reflect-line"({ scene }, line) {
      const source = scene.lines[line.binding.sourceIndex];
      const lineStart = scene.points[line.binding.lineStartIndex];
      const lineEnd = scene.points[line.binding.lineEndIndex];
      if (source && lineStart && lineEnd) {
        line.points = source.points.map((/** @type {Point} */ point) => reflectAcrossLine(point, lineStart, lineEnd));
      }
    },
    "custom-transform-trace"({ scene, parameters }, line) {
      const sampled = sampleCustomTransformTraceLine(scene, line, parameters);
      if (sampled) {
        line.points = sampled;
      }
    },
    "coordinate-trace"({ env }, line) {
      const sampled = window.GspViewerModules.scene.sampleCoordinateTracePoints(env, line.binding);
      if (sampled && sampled.length >= 2) {
        line.points = sampled;
      }
    },
    "point-trace"({ scene, parameters }, line) {
      const sampled = samplePointTraceLine(scene, line, parameters);
      if (sampled) {
        line.points = sampled;
      }
    },
  };

  /** @type {Record<string, CircleBindingRefresher>} */
  const CIRCLE_BINDING_REFRESHERS = {
    "point-radius-circle"({ env }, circle) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const radiusPoint = env.resolveScenePoint(circle.binding.radiusIndex);
      if (!center || !radiusPoint) return;
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: radiusPoint.x, y: radiusPoint.y };
    },
    "segment-radius-circle"({ env }, circle) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const lineStart = env.resolveScenePoint(circle.binding.lineStartIndex);
      const lineEnd = env.resolveScenePoint(circle.binding.lineEndIndex);
      if (!center || !lineStart || !lineEnd) return;
      const radius = Math.hypot(lineEnd.x - lineStart.x, lineEnd.y - lineStart.y);
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: center.x + radius, y: center.y };
    },
    "rotate-circle"({ scene, parameters, resolveHandle }, circle) {
      const source = scene.circles[circle.binding.sourceIndex];
      const center = scene.points[circle.binding.centerIndex];
      if (!source || !center) return;
      const sourceCenter = resolveHandle(source.center);
      const sourceRadius = resolveHandle(source.radiusPoint);
      const angleDegrees = circle.binding.parameterName
        ? parameters.get(circle.binding.parameterName)
        : circle.binding.angleDegrees;
      if (!Number.isFinite(angleDegrees)) return;
      const radians = angleDegrees * Math.PI / 180;
      circle.center = rotateAround(sourceCenter, center, radians);
      circle.radiusPoint = rotateAround(sourceRadius, center, radians);
    },
    "scale-circle"({ scene, resolveHandle }, circle) {
      const source = scene.circles[circle.binding.sourceIndex];
      const center = scene.points[circle.binding.centerIndex];
      if (!source || !center) return;
      const sourceCenter = resolveHandle(source.center);
      const sourceRadius = resolveHandle(source.radiusPoint);
      circle.center = scaleAround(sourceCenter, center, circle.binding.factor);
      circle.radiusPoint = scaleAround(sourceRadius, center, circle.binding.factor);
    },
    "reflect-circle"({ scene, resolveHandle }, circle) {
      const source = scene.circles[circle.binding.sourceIndex];
      const lineStart = scene.points[circle.binding.lineStartIndex];
      const lineEnd = scene.points[circle.binding.lineEndIndex];
      if (!source || !lineStart || !lineEnd) return;
      circle.center = reflectAcrossLine(resolveHandle(source.center), lineStart, lineEnd);
      circle.radiusPoint = reflectAcrossLine(resolveHandle(source.radiusPoint), lineStart, lineEnd);
    },
  };

  /** @type {Record<string, PolygonBindingRefresher>} */
  const POLYGON_BINDING_REFRESHERS = {
    "point-polygon"({ scene }, polygon) {
      const points = polygon.binding.vertexIndices
        .map((/** @type {number} */ index) => scene.points[index])
        .filter(Boolean);
      if (points.length === polygon.binding.vertexIndices.length) {
        polygon.points = points.map((/** @type {Point} */ point) => ({ x: point.x, y: point.y }));
      }
    },
    "arc-boundary-polygon"({ env }, polygon) {
      const sampled = window.GspViewerModules.scene.sampleArcBoundaryPoints(env, polygon.binding);
      if (sampled) {
        polygon.points = sampled;
      }
    },
    "translate-polygon"({ scene, resolveHandle }, polygon) {
      const source = scene.polygons[polygon.binding.sourceIndex];
      const vectorStart = scene.points[polygon.binding.vectorStartIndex];
      const vectorEnd = scene.points[polygon.binding.vectorEndIndex];
      if (!source || !vectorStart || !vectorEnd) return;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      polygon.points = source.points.map((/** @type {PointHandle} */ handle) => {
        const point = resolveHandle(handle);
        return { x: point.x + dx, y: point.y + dy };
      });
    },
    "rotate-polygon"({ scene, parameters, resolveHandle }, polygon) {
      const source = scene.polygons[polygon.binding.sourceIndex];
      const center = scene.points[polygon.binding.centerIndex];
      if (!source || !center) return;
      const angleDegrees = polygon.binding.parameterName
        ? parameters.get(polygon.binding.parameterName)
        : polygon.binding.angleDegrees;
      if (!Number.isFinite(angleDegrees)) return;
      const radians = angleDegrees * Math.PI / 180;
      polygon.points = source.points.map((/** @type {PointHandle} */ handle) => {
        const point = resolveHandle(handle);
        return rotateAround(point, center, radians);
      });
    },
    "scale-polygon"({ scene, resolveHandle }, polygon) {
      const source = scene.polygons[polygon.binding.sourceIndex];
      const center = scene.points[polygon.binding.centerIndex];
      if (!source || !center) return;
      polygon.points = source.points.map((/** @type {PointHandle} */ handle) => {
        const point = resolveHandle(handle);
        return scaleAround(point, center, polygon.binding.factor);
      });
    },
    "reflect-polygon"({ scene, resolveHandle }, polygon) {
      const source = scene.polygons[polygon.binding.sourceIndex];
      const lineStart = scene.points[polygon.binding.lineStartIndex];
      const lineEnd = scene.points[polygon.binding.lineEndIndex];
      if (!source || !lineStart || !lineEnd) return;
      polygon.points = source.points.map((/** @type {PointHandle} */ handle) => {
        const point = resolveHandle(handle);
        return reflectAcrossLine(point, lineStart, lineEnd);
      });
    },
  };

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function refreshDerivedPoints(env, scene) {
    const bounds = env.getViewBounds ? env.getViewBounds() : (scene.bounds || env.sourceScene.bounds);
    const parameters = parameterMap(env);
    const resolveHandle = (/** @type {PointHandle} */ handle) => {
      if (hasPointIndexHandle(handle)) {
        return env.resolveScenePoint(handle.pointIndex);
      }
      if (hasLineIndexHandle(handle)) {
        const line = scene.lines[handle.lineIndex];
        if (!line?.points || line.points.length < 2) return null;
        const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
        const t = typeof handle.t === "number" ? handle.t : 0.5;
        const p0 = line.points[segmentIndex];
        const p1 = line.points[segmentIndex + 1];
        return {
          x: p0.x + (p1.x - p0.x) * t,
          y: p0.y + (p1.y - p0.y) * t,
        };
      }
      return /** @type {Point} */ (handle);
    };

    scene.points.forEach((/** @type {RuntimeScenePointJson} */ point) => {
      const refreshBinding = point.binding ? DERIVED_POINT_BINDING_REFRESHERS[point.binding.kind] : null;
      if (refreshBinding) {
        refreshBinding(env, scene, point, parameters);
      }
    });

    scene.points.forEach((/** @type {RuntimeScenePointJson} */ point, /** @type {number} */ pointIndex) => {
      if (!point.constraint) {
        return;
      }
      const resolved = env.resolveScenePoint(pointIndex);
      if (!resolved) {
        return;
      }
      point.x = resolved.x;
      point.y = resolved.y;
    });

    const shapeContext = { env, scene, parameters, resolveHandle };
    scene.circles.forEach((/** @type {RuntimeCircleJson} */ circle) => {
      const refreshCircle = circle.binding ? CIRCLE_BINDING_REFRESHERS[circle.binding.kind] : null;
      if (refreshCircle) {
        refreshCircle(shapeContext, circle);
      }
    });

    const sourceCircleIterations = env.sourceScene.circleIterations || [];
    if (sourceCircleIterations.length > 0) {
      const generatedCount = sourceCircleIterations.reduce((sum, family) => sum + family.depth, 0);
      const baseCount = Math.max(0, env.sourceScene.circles.length - generatedCount);
      scene.circles = scene.circles.slice(0, baseCount);
      sourceCircleIterations.forEach((/** @type {RuntimeCircleIterationFamily} */ family) => {
        const source = scene.circles[family.sourceCircleIndex];
        if (!source) {
          return;
        }
        const vertices = family.vertexIndices
          .map((/** @type {number} */ index) => scene.points[index])
          .filter(Boolean);
        if (vertices.length !== family.vertexIndices.length) {
          return;
        }
        const liveSeedParameter =
          polygonBoundaryParameterFromPoint(scene, family.sourceCenterIndex);
        const liveNextParameter =
          polygonBoundaryParameterFromPoint(scene, family.sourceNextCenterIndex);
        const seedParameter = Number.isFinite(liveSeedParameter)
          ? liveSeedParameter
          : family.seedParameter;
        const stepParameter = Number.isFinite(liveSeedParameter) && Number.isFinite(liveNextParameter)
          ? ((liveNextParameter - liveSeedParameter) % 1 + 1) % 1
          : family.stepParameter;
        const depth = pointIterationDepth({
          depth: family.depth,
          parameterName: family.depthParameterName,
        }, parameters);
        const dx = source.radiusPoint.x - source.center.x;
        const dy = source.radiusPoint.y - source.center.y;
        for (let step = 1; step <= depth; step += 1) {
          const center = pointOnPolygonBoundary(
            vertices,
            seedParameter + stepParameter * step,
          );
          if (!center) {
            continue;
          }
          scene.circles.push({
            center,
            radiusPoint: {
              x: center.x + dx,
              y: center.y + dy,
            },
            color: source.color,
            fillColor: source.fillColor,
            dashed: source.dashed,
            visible: family.visible !== false,
            binding: null,
          });
        }
      });
    }

    scene.polygons.forEach((/** @type {RuntimePolygonJson} */ polygon) => {
      const refreshPolygon = polygon.binding ? POLYGON_BINDING_REFRESHERS[polygon.binding.kind] : null;
      if (refreshPolygon) {
        refreshPolygon(shapeContext, polygon);
      }
    });

    const preservedLines = [];
    const rotateFamilies = new Map();
    const lineContext = { env, scene, bounds, parameters };
    scene.lines.forEach((/** @type {RuntimeLineJson} */ line) => {
      const bindingKind = line.binding?.kind;
      if (!bindingKind) {
        preservedLines.push(line);
        return;
      }
      if (bindingKind !== "rotate-edge") {
        const refreshLine = LINE_BINDING_REFRESHERS[bindingKind];
        if (refreshLine) {
          refreshLine(lineContext, line);
        }
        preservedLines.push(line);
        return;
      }
      const key = `${line.binding.centerIndex}:${line.binding.vertexIndex}:${line.binding.parameterName}`;
      if (!rotateFamilies.has(key)) {
        rotateFamilies.set(key, {
          binding: line.binding,
          color: line.color,
          dashed: line.dashed,
        });
      }
    });
    for (const family of rotateFamilies.values()) {
      const center = scene.points[family.binding.centerIndex];
      const vertex = scene.points[family.binding.vertexIndex];
      const sidesValue = parameters.get(family.binding.parameterName);
      const sides = Math.max(1, Math.round(Number.isFinite(sidesValue) ? sidesValue : 1));
      if (!center || !vertex) continue;
      if (sides === 1) continue;
      const angleDegrees = evaluateExpr(family.binding.angleExpr, 0, parameters);
      if (!Number.isFinite(angleDegrees)) continue;
      const rotate = (/** @type {number} */ step) => rotateAround(vertex, center, (angleDegrees * step) * Math.PI / 180);
      if (sides === 2) {
        preservedLines.push({
          points: [rotate(0), rotate(1)],
          color: family.color,
          dashed: family.dashed,
          binding: {
            ...family.binding,
            angleDegrees,
            startStep: 0,
            endStep: 1,
          },
        });
        continue;
      }
      for (let step = 0; step < sides; step += 1) {
        preservedLines.push({
          points: [rotate(step), rotate((step + 1) % sides)],
          color: family.color,
          dashed: family.dashed,
          binding: {
            ...family.binding,
            angleDegrees,
            startStep: step,
            endStep: (step + 1) % sides,
          },
        });
      }
    }
    scene.lines = preservedLines;
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function refreshDynamicLabels(env, scene) {
    const parameters = parameterMap(env);
    scene.labels.forEach((/** @type {RuntimeLabelJson} */ label) => {
      if (!label.binding) return;
      const refreshLabel = DYNAMIC_LABEL_REFRESHERS[label.binding.kind];
      if (refreshLabel) {
        refreshLabel(env, scene, label, parameters);
      }
    });
  }

  /** @param {ViewerEnv} env */
  function syncDynamicScene(env) {
    env.updateScene((draft) => {
      const parameters = parameterMap(env);
      env.currentDynamics().parameters.forEach((/** @type {ParameterJson} */ parameter) => {
        if (typeof parameter.labelIndex === "number" && draft.labels[parameter.labelIndex]) {
          draft.labels[parameter.labelIndex].text =
            `${parameter.name} = ${parameter.value.toFixed(2)}${parameterValueSuffix(parameter)}`;
        }
      });
      draft.points.forEach((/** @type {RuntimeScenePointJson} */ point) => {
        if (point.binding?.kind !== "parameter" || !point.constraint) {
          const updatePoint = point.binding ? SYNC_DYNAMIC_POINT_BINDING_UPDATERS[point.binding.kind] : null;
          if (updatePoint) {
            updatePoint(env, draft, point, parameters);
          }
          return;
        }
        const value = parameters.get(point.binding.name);
        if (!Number.isFinite(value)) return;
        applyNormalizedParameterToPoint(point, draft, value);
      });
      env.currentDynamics().functions.forEach((/** @type {FunctionJson} */ functionDef) => {
        if (draft.labels[functionDef.labelIndex]) {
          const variableLabel = functionDef.domain.plotMode === "polar" ? "θ" : "x";
          const head = functionDef.domain.plotMode === "polar"
            ? (functionDef.derivative ? `r'(${variableLabel})` : "r")
            : (functionDef.derivative
              ? `${functionDef.name}'(${variableLabel})`
              : `${functionDef.name}(${variableLabel})`);
          draft.labels[functionDef.labelIndex].text = `${head} = ${formatExpr(functionDef.expr, env.formatAxisNumber, variableLabel)}`;
        }
        const sampled = sampleDynamicFunction(functionDef, parameters);
        if (typeof functionDef.lineIndex === "number" && draft.lines[functionDef.lineIndex]) {
          draft.lines[functionDef.lineIndex].points = sampled.map((point) => ({ ...point }));
        }
        functionDef.constrainedPointIndices.forEach((/** @type {number} */ pointIndex) => {
          const constraint = draft.points[pointIndex]?.constraint;
          if (constraint && constraint.kind === "polyline") {
            constraint.points = sampled.map((point) => ({ ...point }));
            constraint.segmentIndex = Math.min(constraint.segmentIndex, Math.max(0, sampled.length - 2));
          }
        });
      });
      refreshIterationGeometry(env, draft, parameters);
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function refreshIterationGeometry(env, scene, parameters) {
    rebuildIterationPoints(env, scene, parameters);
    rebuildIteratedLines(env, scene, parameters);
    rebuildIteratedPolygons(env, scene, parameters);
    rebuildIteratedLabels(env, scene, parameters);
    rebuildIterationTables(env, scene, parameters);
  }

  /** @param {ParameterJson} parameter */
  function parameterValueSuffix(parameter) {
    switch (parameter.unit) {
      case "degree":
        return "\u00B0";
      case "cm":
        return " cm";
      default:
        return "";
    }
  }

  /** @param {ViewerEnv} env */
  function buildParameterControls(env) {
    env.parameterControls.replaceChildren();
    const controls = env.currentDynamics().parameters.map((parameter, index) => env.labelTag(
      `${parameter.name} =`,
      env.inputTag({
        type: "number",
        step: parameter.name === "n" ? "1" : "0.1",
        min: parameter.name === "n" ? "0" : undefined,
        value: parameter.value.toFixed(2),
        oninput: (event) => {
          const target = /** @type {HTMLInputElement} */ (event.target);
          let value = Number.parseFloat(target.value);
          if (Number.isFinite(value)) {
            if (parameter.name === "n") {
              value = Math.max(0, Math.round(value));
            }
            env.updateDynamics((draft) => {
              draft.parameters[index].value = value;
            });
            syncDynamicScene(env);
          }
        },
      }),
      parameterValueSuffix(parameter),
    ));
    if (controls.length > 0) {
      env.van.add(env.parameterControls, ...controls);
    }
  }

  modules.dynamics = {
    buildParameterControls,
    evaluateExpr,
    formatExpr,
    parameterValueFromPoint,
    applyNormalizedParameterToPoint,
    refreshDerivedPoints,
    refreshDynamicLabels,
    refreshIterationGeometry,
    syncDynamicScene,
  };
})();
