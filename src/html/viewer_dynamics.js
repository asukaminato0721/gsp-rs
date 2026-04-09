// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function lerpPoint(start, end, t) {
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }

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

  function scaleAround(point, center, factor) {
    return {
      x: center.x + (point.x - center.x) * factor,
      y: center.y + (point.y - center.y) * factor,
    };
  }

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

  function clipParametricLineToBounds(start, end, bounds, rayOnly) {
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

  function clipLineToBounds(start, end, bounds) {
    return clipParametricLineToBounds(start, end, bounds, false);
  }

  function clipRayToBounds(start, end, bounds) {
    return clipParametricLineToBounds(start, end, bounds, true);
  }

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

  function resolveRightAngleMarkerPoints(vertex, first, second, shortestLen) {
    const side = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
    if (side <= 1e-9) return null;
    return [
      { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
      { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
      { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
    ];
  }

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

  function evaluateExprTerm(term, x, parameters) {
    switch (term.kind) {
      case "variable": return x;
      case "constant": return term.value;
      case "parameter": return parameters.get(term.name) ?? term.value;
      case "unary_x": return evaluateUnary(term.op, x);
      case "product": {
        const left = evaluateExprTerm(term.left, x, parameters);
        const right = evaluateExprTerm(term.right, x, parameters);
        return left === null || right === null ? null : left * right;
      }
      case "power": {
        const base = evaluateExprTerm(term.base, x, parameters);
        const exponent = evaluateExprTerm(term.exponent, x, parameters);
        if (base === null || exponent === null) return null;
        const value = Math.pow(base, exponent);
        return Number.isFinite(value) ? value : null;
      }
      default: return null;
    }
  }

  function evaluateExpr(expr, x, parameters) {
    if (expr.kind === "constant") return expr.value;
    if (expr.kind === "identity") return x;
    if (expr.kind !== "parsed") return null;
    let value = evaluateExprTerm(expr.head, x, parameters);
    if (value === null) return null;
    for (const part of expr.tail) {
      const rhs = evaluateExprTerm(part.term, x, parameters);
      if (rhs === null) return null;
      value = part.op === "sub"
        ? value - rhs
        : part.op === "mul"
          ? value * rhs
          : part.op === "div"
            ? (Math.abs(rhs) >= 1e-9 ? value / rhs : null)
            : value + rhs;
      if (value === null) return null;
    }
    return Number.isFinite(value) ? value : null;
  }

  function formatExprTerm(term, formatAxisNumber, variableLabel = "x") {
    switch (term.kind) {
      case "variable": return variableLabel;
      case "constant": return formatAxisNumber(term.value);
      case "parameter": return term.name;
      case "unary_x": return `${term.op}(${variableLabel})`;
      case "product":
        return `${formatExprTerm(term.left, formatAxisNumber, variableLabel)}*${formatExprTerm(term.right, formatAxisNumber, variableLabel)}`;
      case "power":
        return `${formatExprTerm(term.base, formatAxisNumber, variableLabel)}^${formatExprTerm(term.exponent, formatAxisNumber, variableLabel)}`;
      default: return "?";
    }
  }

  function formatExpr(expr, formatAxisNumber, variableLabel = "x") {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return variableLabel;
    if (expr.kind === "parsed") {
      let text = formatExprTerm(expr.head, formatAxisNumber, variableLabel);
      for (const part of expr.tail) {
        text += part.op === "sub"
          ? " - "
          : part.op === "mul"
            ? " * "
          : part.op === "div"
              ? " / "
              : " + ";
        text += formatExprTerm(part.term, formatAxisNumber, variableLabel);
      }
      return text;
    }
    return "?";
  }

  /** @param {ViewerEnv} env */
  function parameterMap(env) {
    return new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]));
  }

  /**
   * @param {any} expr
   * @param {Set<string>} names
   */
  function collectExprParameterNames(expr, names) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parameter" && typeof expr.name === "string") {
      names.add(expr.name);
      return;
    }
    if (expr.kind === "parsed") {
      collectExprTermParameterNames(expr.head, names);
      for (const part of expr.tail || []) {
        collectExprTermParameterNames(part.term, names);
      }
    }
  }

  /**
   * @param {any} term
   * @param {Set<string>} names
   */
  function collectExprTermParameterNames(term, names) {
    if (!term || typeof term !== "object") return;
    if (term.kind === "parameter" && typeof term.name === "string") {
      names.add(term.name);
      return;
    }
    if (term.kind === "product" || term.kind === "power") {
      collectExprTermParameterNames(term.left, names);
      collectExprTermParameterNames(term.right, names);
    }
  }

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

  function wrapUnitInterval(value) {
    return ((value % 1) + 1) % 1;
  }

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

  function parameterValueFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (!constraint) return null;
    if (constraint.kind === "segment") {
      return constraint.t;
    }
    if (constraint.kind === "polyline") {
      return constraint.t;
    }
    if (constraint.kind === "polygon-boundary") {
      return polygonBoundaryParameterFromPoint(scene, pointIndex);
    }
    if (constraint.kind === "circle") {
      return circleParameterFromPoint(scene, pointIndex);
    }
    if (constraint.kind === "circle-arc") {
      return constraint.t;
    }
    if (constraint.kind === "arc") {
      return constraint.t;
    }
    return null;
  }

  function applyNormalizedParameterToPoint(point, scene, value) {
    if (!point.constraint) return;
    const wrapped = wrapUnitInterval(value);
    if (point.constraint.kind === "segment") {
      point.constraint.t = wrapped;
    } else if (point.constraint.kind === "polyline") {
      point.constraint.t = wrapped;
    } else if (point.constraint.kind === "polygon-boundary") {
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
          break;
        }
        traveled += length;
      }
    } else if (point.constraint.kind === "circle") {
      const angle = Math.PI * 2 * wrapped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    } else if (point.constraint.kind === "circle-arc") {
      point.constraint.t = wrapped;
    } else if (point.constraint.kind === "arc") {
      point.constraint.t = wrapped;
    }
  }

  function pointIterationDepth(family, parameters) {
    const rawValue = family.parameterName ? parameters.get(family.parameterName) : family.depth;
    const fallback = Number.isFinite(family.depth) ? family.depth : 0;
    const depth = Number.isFinite(rawValue) ? rawValue : fallback;
    return Math.max(0, Math.round(depth));
  }

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
    return (point) => {
      const relative = { x: point.x - sourceOrigin.x, y: point.y - sourceOrigin.y };
      const u = (relative.x * sv.y - relative.y * sv.x) / det;
      const v = (su.x * relative.y - su.y * relative.x) / det;
      return {
        x: targetOrigin.x + tu.x * u + tv.x * v,
        y: targetOrigin.y + tu.y * u + tv.y * v,
      };
    };
  }

  function formatSequenceValue(value) {
    if (!Number.isFinite(value)) {
      return "-";
    }
    return Math.abs(value - Math.round(value)) < 0.005
      ? String(Math.round(value))
      : value.toFixed(2);
  }

  function darken(color, amount) {
    return [
      Math.max(0, color[0] - amount),
      Math.max(0, color[1] - amount),
      Math.max(0, color[2] - amount),
      color[3],
    ];
  }

  function evaluateRecursiveExpression(expr, parameterName, currentValue, parameters) {
    const nextParameters = new Map(parameters);
    nextParameters.set(parameterName, currentValue);
    return evaluateExpr(expr, 0, nextParameters);
  }

  /** @param {ViewerEnv} env */
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

  /** @param {ViewerEnv} env */
  function rebuildIteratedLines(env, scene, parameters) {
    const families = env.sourceScene.lineIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      const depth = family.depth || 0;
      if (family.kind === "affine") {
        return sum + depth;
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
      if (family.kind === "affine") {
        const resolveHandle = (handle) => {
          if (typeof handle?.pointIndex === "number") {
            return env.resolveScenePoint(handle.pointIndex);
          }
          if (typeof handle?.lineIndex === "number") {
            const line = scene.lines[handle.lineIndex];
            if (!line?.points || line.points.length < 2) return handle;
            const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
            const t = typeof handle.t === "number" ? handle.t : 0.5;
            const p0 = line.points[segmentIndex];
            const p1 = line.points[segmentIndex + 1];
            return {
              x: p0.x + (p1.x - p0.x) * t,
              y: p0.y + (p1.y - p0.y) * t,
            };
          }
          return handle || { x: 0, y: 0 };
        };
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
      if (hasSecondary) {
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

  /** @param {ViewerEnv} env */
  function rebuildIteratedPolygons(env, scene, parameters) {
    const families = env.sourceScene.polygonIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      const depth = family.depth || 0;
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
      if (hasSecondary) {
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

  /** @param {ViewerEnv} env */
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

  /** @param {ViewerEnv} env */
  function refreshDerivedPoints(env, scene) {
    const bounds = env.getViewBounds ? env.getViewBounds() : (scene.bounds || env.sourceScene.bounds);
    const parameters = parameterMap(env);
    const resolveHandle = (handle) => {
      if (typeof handle?.pointIndex === "number") {
        return env.resolveScenePoint(handle.pointIndex);
      }
      return handle || { x: 0, y: 0 };
    };

    scene.points.forEach((point) => {
      if (point.binding?.kind === "derived-parameter") {
        const value = parameterValueFromPoint(scene, point.binding.sourceIndex);
        if (value !== null) {
          applyNormalizedParameterToPoint(point, scene, value);
        }
      } else if (point.binding?.kind === "coordinate-source") {
        const source = env.resolveScenePoint(point.binding.sourceIndex);
        if (!source) return;
        const exprParameters = new Map(parameters);
        exprParameters.set(point.binding.name, parameters.get(point.binding.name));
        const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
        if (offset !== null) {
          if (point.binding.axis === "horizontal") {
            point.x = source.x + offset;
            point.y = source.y;
          } else {
            point.x = source.x;
            point.y = source.y + offset;
          }
        }
      } else if (point.binding?.kind === "translate") {
        const source = env.resolveScenePoint(point.binding.sourceIndex);
        const vectorStart = env.resolveScenePoint(point.binding.vectorStartIndex);
        const vectorEnd = env.resolveScenePoint(point.binding.vectorEndIndex);
        if (!source || !vectorStart || !vectorEnd) return;
        point.x = source.x + (vectorEnd.x - vectorStart.x);
        point.y = source.y + (vectorEnd.y - vectorStart.y);
      } else if (point.binding?.kind === "reflect") {
        const source = env.resolveScenePoint(point.binding.sourceIndex);
        const lineStart = env.resolveScenePoint(point.binding.lineStartIndex);
        const lineEnd = env.resolveScenePoint(point.binding.lineEndIndex);
        if (!source || !lineStart || !lineEnd) return;
        const reflected = reflectAcrossLine(source, lineStart, lineEnd);
        point.x = reflected.x;
        point.y = reflected.y;
      } else if (point.binding?.kind === "rotate") {
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
      } else if (point.binding?.kind === "scale") {
        const source = env.resolveScenePoint(point.binding.sourceIndex);
        const center = env.resolveScenePoint(point.binding.centerIndex);
        if (!source || !center) return;
        const scaled = scaleAround(source, center, point.binding.factor);
        point.x = scaled.x;
        point.y = scaled.y;
      } else if (point.binding?.kind === "custom-transform") {
        const value = parameterValueFromPoint(scene, point.binding.sourceIndex);
        if (!Number.isFinite(value)) return;
        const exprParameters = new Map(parameters);
        const names = new Set();
        collectExprParameterNames(point.binding.distanceExpr, names);
        collectExprParameterNames(point.binding.angleExpr, names);
        names.forEach((name) => exprParameters.set(name, value));
        const distanceValue = evaluateExpr(point.binding.distanceExpr, value, exprParameters);
        const angleValue = evaluateExpr(point.binding.angleExpr, value, exprParameters);
        const origin = env.resolveScenePoint(point.binding.originIndex);
        const axisEnd = env.resolveScenePoint(point.binding.axisEndIndex);
        if (distanceValue === null || angleValue === null || !origin || !axisEnd) return;
        const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
        const radians = (baseAngle + angleValue * point.binding.angleDegreesScale) * Math.PI / 180;
        const distance = distanceValue * point.binding.distanceRawScale;
        point.x = origin.x + distance * Math.cos(radians);
        point.y = origin.y - distance * Math.sin(radians);
      }
    });

    scene.points.forEach((point, pointIndex) => {
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

    scene.circles.forEach((circle) => {
      if (circle.binding?.kind === "point-radius-circle") {
        const center = env.resolveScenePoint(circle.binding.centerIndex);
        const radiusPoint = env.resolveScenePoint(circle.binding.radiusIndex);
        if (!center || !radiusPoint) return;
        circle.center = { x: center.x, y: center.y };
        circle.radiusPoint = { x: radiusPoint.x, y: radiusPoint.y };
      } else if (circle.binding?.kind === "segment-radius-circle") {
        const center = env.resolveScenePoint(circle.binding.centerIndex);
        const lineStart = env.resolveScenePoint(circle.binding.lineStartIndex);
        const lineEnd = env.resolveScenePoint(circle.binding.lineEndIndex);
        if (!center || !lineStart || !lineEnd) return;
        const radius = Math.hypot(lineEnd.x - lineStart.x, lineEnd.y - lineStart.y);
        circle.center = { x: center.x, y: center.y };
        circle.radiusPoint = { x: center.x + radius, y: center.y };
      } else if (circle.binding?.kind === "rotate-circle") {
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
      } else if (circle.binding?.kind === "scale-circle") {
        const source = scene.circles[circle.binding.sourceIndex];
        const center = scene.points[circle.binding.centerIndex];
        if (!source || !center) return;
        const sourceCenter = resolveHandle(source.center);
        const sourceRadius = resolveHandle(source.radiusPoint);
        circle.center = scaleAround(sourceCenter, center, circle.binding.factor);
        circle.radiusPoint = scaleAround(sourceRadius, center, circle.binding.factor);
      } else if (circle.binding?.kind === "reflect-circle") {
        const source = scene.circles[circle.binding.sourceIndex];
        const lineStart = scene.points[circle.binding.lineStartIndex];
        const lineEnd = scene.points[circle.binding.lineEndIndex];
        if (!source || !lineStart || !lineEnd) return;
        circle.center = reflectAcrossLine(resolveHandle(source.center), lineStart, lineEnd);
        circle.radiusPoint = reflectAcrossLine(resolveHandle(source.radiusPoint), lineStart, lineEnd);
      }
    });

    scene.polygons.forEach((polygon) => {
      if (polygon.binding?.kind === "translate-polygon") {
        const source = scene.polygons[polygon.binding.sourceIndex];
        const vectorStart = scene.points[polygon.binding.vectorStartIndex];
        const vectorEnd = scene.points[polygon.binding.vectorEndIndex];
        if (!source || !vectorStart || !vectorEnd) return;
        const dx = vectorEnd.x - vectorStart.x;
        const dy = vectorEnd.y - vectorStart.y;
        polygon.points = source.points.map((handle) => {
          const point = resolveHandle(handle);
          return { x: point.x + dx, y: point.y + dy };
        });
      } else if (polygon.binding?.kind === "rotate-polygon") {
        const source = scene.polygons[polygon.binding.sourceIndex];
        const center = scene.points[polygon.binding.centerIndex];
        if (!source || !center) return;
        const angleDegrees = polygon.binding.parameterName
          ? parameters.get(polygon.binding.parameterName)
          : polygon.binding.angleDegrees;
        if (!Number.isFinite(angleDegrees)) return;
        const radians = angleDegrees * Math.PI / 180;
        polygon.points = source.points.map((handle) => {
          const point = resolveHandle(handle);
          return rotateAround(point, center, radians);
        });
      } else if (polygon.binding?.kind === "scale-polygon") {
        const source = scene.polygons[polygon.binding.sourceIndex];
        const center = scene.points[polygon.binding.centerIndex];
        if (!source || !center) return;
        polygon.points = source.points.map((handle) => {
          const point = resolveHandle(handle);
          return scaleAround(point, center, polygon.binding.factor);
        });
      } else if (polygon.binding?.kind === "reflect-polygon") {
        const source = scene.polygons[polygon.binding.sourceIndex];
        const lineStart = scene.points[polygon.binding.lineStartIndex];
        const lineEnd = scene.points[polygon.binding.lineEndIndex];
        if (!source || !lineStart || !lineEnd) return;
        polygon.points = source.points.map((handle) => {
          const point = resolveHandle(handle);
          return reflectAcrossLine(point, lineStart, lineEnd);
        });
      }
    });

    const preservedLines = [];
    const rotateFamilies = new Map();
    const resolveHostLinePoints = (binding) => {
      if (typeof binding?.lineIndex === "number") {
        const hostLine = scene.lines[binding.lineIndex];
        return hostLine?.points?.length >= 2 ? hostLine.points : null;
      }
      if (
        typeof binding?.lineStartIndex === "number"
        && typeof binding?.lineEndIndex === "number"
      ) {
        const start = scene.points[binding.lineStartIndex];
        const end = scene.points[binding.lineEndIndex];
        return start && end ? [start, end] : null;
      }
      return null;
    };
    scene.lines.forEach((line) => {
      if (line.binding?.kind === "segment") {
        const start = scene.points[line.binding.startIndex];
        const end = scene.points[line.binding.endIndex];
        if (start && end) {
          line.points = [{ x: start.x, y: start.y }, { x: end.x, y: end.y }];
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "angle-marker") {
        const start = scene.points[line.binding.startIndex];
        const vertex = scene.points[line.binding.vertexIndex];
        const end = scene.points[line.binding.endIndex];
        const points = start && vertex && end
          ? resolveAngleMarkerPoints(start, vertex, end, line.binding.markerClass)
          : null;
        if (points) {
          line.points = points;
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "angle-bisector-ray") {
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
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "perpendicular-line") {
        const through = scene.points[line.binding.throughIndex];
        const hostLine = resolveHostLinePoints(line.binding);
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
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "parallel-line") {
        const through = scene.points[line.binding.throughIndex];
        const hostLine = resolveHostLinePoints(line.binding);
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
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "line") {
        const start = scene.points[line.binding.startIndex];
        const end = scene.points[line.binding.endIndex];
        const clipped = start && end ? clipLineToBounds(start, end, bounds) : null;
        if (clipped) line.points = clipped;
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "ray") {
        const start = scene.points[line.binding.startIndex];
        const end = scene.points[line.binding.endIndex];
        const clipped = start && end ? clipRayToBounds(start, end, bounds) : null;
        if (clipped) line.points = clipped;
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "arc-boundary") {
        const sampled = window.GspViewerModules.scene.sampleArcBoundaryPoints(env, line.binding);
        if (sampled) {
          line.points = sampled;
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "rotate-line") {
        const source = scene.lines[line.binding.sourceIndex];
        const center = scene.points[line.binding.centerIndex];
        if (source && center) {
          const angleDegrees = line.binding.parameterName
            ? parameters.get(line.binding.parameterName)
            : line.binding.angleDegrees;
          if (!Number.isFinite(angleDegrees)) {
            return;
          }
          const radians = angleDegrees * Math.PI / 180;
          line.points = source.points.map((point) => rotateAround(point, center, radians));
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "scale-line") {
        const source = scene.lines[line.binding.sourceIndex];
        const center = scene.points[line.binding.centerIndex];
        if (source && center) {
          line.points = source.points.map((point) => scaleAround(point, center, line.binding.factor));
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "reflect-line") {
        const source = scene.lines[line.binding.sourceIndex];
        const lineStart = scene.points[line.binding.lineStartIndex];
        const lineEnd = scene.points[line.binding.lineEndIndex];
        if (source && lineStart && lineEnd) {
          line.points = source.points.map((point) => reflectAcrossLine(point, lineStart, lineEnd));
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "custom-transform-trace") {
        const point = scene.points[line.binding.pointIndex];
        const binding = point?.binding;
        if (binding?.kind === "custom-transform") {
          const origin = scene.points[binding.originIndex];
          const axisEnd = scene.points[binding.axisEndIndex];
          const traceMax = parameterValueFromPoint(scene, binding.sourceIndex);
          if (origin && axisEnd && Number.isFinite(traceMax)) {
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
            if (sampled.length >= 2) {
              line.points = sampled;
            }
          }
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind === "coordinate-trace") {
        const sampled = window.GspViewerModules.scene.sampleCoordinateTracePoints(env, line.binding);
        if (sampled && sampled.length >= 2) {
          line.points = sampled;
        }
        preservedLines.push(line);
        return;
      }
      if (line.binding?.kind !== "rotate-edge") {
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
      const rotate = (step) => rotateAround(vertex, center, (angleDegrees * step) * Math.PI / 180);
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

  /** @param {ViewerEnv} env */
  function refreshDynamicLabels(env, scene) {
    const parameters = parameterMap(env);
    scene.labels.forEach((label) => {
      if (!label.binding) return;
      if (label.binding.kind === "parameter-value") {
        const value = parameters.get(label.binding.name);
        if (value !== null && value !== undefined) {
          label.text = `${label.binding.name} = ${env.formatNumber(value)}`;
        }
      } else if (label.binding.kind === "point-expression-value") {
        const currentValue = parameters.get(label.binding.parameterName);
        if (Number.isFinite(currentValue)) {
          const value = evaluateRecursiveExpression(
            label.binding.expr,
            label.binding.parameterName,
            currentValue,
            parameters,
          );
          if (value !== null) {
            label.text = formatSequenceValue(value);
          }
        }
      } else if (label.binding.kind === "expression-value") {
        const value = evaluateExpr(label.binding.expr, 0, parameters);
        if (value !== null) {
          label.text = label.binding.exprLabel === "360° / n"
            ? `360°\n——— = ${env.formatNumber(value)}°\n  n`
            : `${label.binding.exprLabel} = ${env.formatNumber(value)}`;
        }
      } else if (label.binding.kind === "polygon-boundary-parameter") {
        const value = polygonBoundaryParameterFromPoint(scene, label.binding.pointIndex);
        if (value !== null) {
          label.text = `${label.binding.pointName}在${label.binding.polygonName}上的t值 = ${env.formatNumber(value)}`;
        }
      } else if (label.binding.kind === "segment-parameter") {
        const point = scene.points[label.binding.pointIndex];
        const value = point?.constraint?.kind === "segment" ? point.constraint.t : null;
        if (value !== null) {
          label.text = `${label.binding.pointName}在${label.binding.segmentName}上的t值 = ${env.formatNumber(value)}`;
        }
      } else if (label.binding.kind === "circle-parameter") {
        const point = scene.points[label.binding.pointIndex];
        const constraint = point?.constraint;
        if (constraint?.kind === "circle") {
          const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
          const tau = Math.PI * 2;
          const value = ((pointAngle % tau) + tau) % tau / tau;
          label.text = `${label.binding.pointName}在⊙${label.binding.circleName}上的值 = ${env.formatNumber(value)}`;
        }
      } else if (label.binding.kind === "angle-marker-value") {
        const start = scene.points[label.binding.startIndex];
        const vertex = scene.points[label.binding.vertexIndex];
        const end = scene.points[label.binding.endIndex];
        if (start && vertex && end) {
          const first = {
            x: start.x - vertex.x,
            y: start.y - vertex.y,
          };
          const second = {
            x: end.x - vertex.x,
            y: end.y - vertex.y,
          };
          const firstLen = Math.hypot(first.x, first.y);
          const secondLen = Math.hypot(second.x, second.y);
          if (firstLen > 1e-9 && secondLen > 1e-9) {
            const cross = (first.x / firstLen) * (second.y / secondLen)
              - (first.y / firstLen) * (second.x / secondLen);
            const dot = (first.x / firstLen) * (second.x / secondLen)
              + (first.y / firstLen) * (second.y / secondLen);
            const value = Math.abs(Math.atan2(cross, dot)) * 180 / Math.PI;
            if (Number.isFinite(value)) {
              label.text = value.toFixed(label.binding.decimals);
            }
          }
        }
      } else if (label.binding.kind === "custom-transform-value") {
        const value = parameterValueFromPoint(scene, label.binding.pointIndex);
        if (Number.isFinite(value)) {
          const exprParameters = new Map(parameters);
          const names = new Set();
          collectExprParameterNames(label.binding.expr, names);
          names.forEach((name) => exprParameters.set(name, value));
          const evaluated = evaluateExpr(label.binding.expr, value, exprParameters);
          if (evaluated !== null) {
            label.text = `${label.binding.exprLabel} = ${env.formatNumber(evaluated * label.binding.valueScale)}${label.binding.valueSuffix}`;
          }
        }
      }
    });
  }

  /** @param {ViewerEnv} env */
  function syncDynamicScene(env) {
    env.updateScene((draft) => {
      const parameters = parameterMap(env);
      env.currentDynamics().parameters.forEach((parameter) => {
        if (typeof parameter.labelIndex === "number" && draft.labels[parameter.labelIndex]) {
          draft.labels[parameter.labelIndex].text = `${parameter.name} = ${parameter.value.toFixed(2)}`;
        }
      });
      draft.points.forEach((point) => {
        if (point.binding?.kind !== "parameter" || !point.constraint) {
          if (point.binding?.kind === "coordinate") {
            const value = parameters.get(point.binding.name);
            if (!Number.isFinite(value)) return;
            point.x = value;
            const y = evaluateExpr(point.binding.expr, 0, parameters);
            if (y !== null) {
              point.y = y;
            }
          } else if (point.binding?.kind === "coordinate-source") {
            const source = draft.points[point.binding.sourceIndex];
            if (!source || !Number.isFinite(source.x)) return;
            const exprParameters = new Map(parameters);
            exprParameters.set(point.binding.name, parameters.get(point.binding.name));
            const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
            if (offset !== null) {
              if (point.binding.axis === "horizontal") {
                point.x = source.x + offset;
                point.y = source.y;
              } else {
                point.x = source.x;
                point.y = source.y + offset;
              }
            }
          } else if (point.binding?.kind === "custom-transform") {
            const value = parameterValueFromPoint(draft, point.binding.sourceIndex);
            if (!Number.isFinite(value)) return;
            const exprParameters = new Map(parameters);
            const names = new Set();
            collectExprParameterNames(point.binding.distanceExpr, names);
            collectExprParameterNames(point.binding.angleExpr, names);
            names.forEach((name) => exprParameters.set(name, value));
            const distanceValue = evaluateExpr(point.binding.distanceExpr, value, exprParameters);
            const angleValue = evaluateExpr(point.binding.angleExpr, value, exprParameters);
            const origin = draft.points[point.binding.originIndex];
            const axisEnd = draft.points[point.binding.axisEndIndex];
            if (
              distanceValue !== null && angleValue !== null &&
              origin && axisEnd
            ) {
              const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
              const radians = (baseAngle + angleValue * point.binding.angleDegreesScale) * Math.PI / 180;
              const distance = distanceValue * point.binding.distanceRawScale;
              point.x = origin.x + distance * Math.cos(radians);
              point.y = origin.y - distance * Math.sin(radians);
            }
          }
          return;
        }
        const value = parameters.get(point.binding.name);
        if (!Number.isFinite(value)) return;
        applyNormalizedParameterToPoint(point, draft, value);
      });
      env.currentDynamics().functions.forEach((functionDef) => {
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
        functionDef.constrainedPointIndices.forEach((pointIndex) => {
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
   * @param {any} scene
   * @param {Map<string, number>} parameters
   */
  function refreshIterationGeometry(env, scene, parameters) {
    rebuildIterationPoints(env, scene, parameters);
    rebuildIteratedLines(env, scene, parameters);
    rebuildIteratedPolygons(env, scene, parameters);
    rebuildIteratedLabels(env, scene, parameters);
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
          let value = Number.parseFloat(event.target.value);
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
    ));
    if (controls.length > 0) {
      env.van.add(env.parameterControls, ...controls);
    }
  }

  modules.dynamics = {
    buildParameterControls,
    evaluateExpr,
    formatExpr,
    refreshDerivedPoints,
    refreshDynamicLabels,
    refreshIterationGeometry,
    syncDynamicScene,
  };
})();
