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

  function formatExprTerm(term, formatAxisNumber) {
    switch (term.kind) {
      case "variable": return "x";
      case "constant": return formatAxisNumber(term.value);
      case "parameter": return term.name;
      case "unary_x": return `${term.op}(x)`;
      case "product": return `${formatExprTerm(term.left, formatAxisNumber)}*${formatExprTerm(term.right, formatAxisNumber)}`;
      default: return "?";
    }
  }

  function formatExpr(expr, formatAxisNumber) {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return "x";
    if (expr.kind === "parsed") {
      let text = formatExprTerm(expr.head, formatAxisNumber);
      for (const part of expr.tail) {
        text += part.op === "sub"
          ? " - "
          : part.op === "mul"
            ? " * "
            : part.op === "div"
              ? " / "
              : " + ";
        text += formatExprTerm(part.term, formatAxisNumber);
      }
      return text;
    }
    return "?";
  }

  /** @param {ViewerEnv} env */
  function parameterMap(env) {
    return new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]));
  }

  function sampleDynamicFunction(functionDef, parameters) {
    const points = [];
    const last = Math.max(1, functionDef.domain.sampleCount - 1);
    for (let index = 0; index < functionDef.domain.sampleCount; index += 1) {
      const t = index / last;
      const x = functionDef.domain.xMin + (functionDef.domain.xMax - functionDef.domain.xMin) * t;
      const y = evaluateExpr(functionDef.expr, x, parameters);
      if (y === null) continue;
      points.push({ x, y });
    }
    return points;
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
    if (constraint.kind === "polygon-boundary") {
      return polygonBoundaryParameterFromPoint(scene, pointIndex);
    }
    if (constraint.kind === "circle") {
      return circleParameterFromPoint(scene, pointIndex);
    }
    return null;
  }

  function applyNormalizedParameterToPoint(point, scene, value) {
    if (!point.constraint) return;
    const clamped = Math.max(0, Math.min(1, value));
    if (point.constraint.kind === "segment") {
      point.constraint.t = clamped;
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
      const target = clamped * perimeter;
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
      const angle = Math.PI * 2 * clamped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    }
  }

  /** @param {ViewerEnv} env */
  function refreshDerivedPoints(env, scene) {
    const bounds = env.getViewBounds ? env.getViewBounds() : (scene.bounds || env.sourceScene.bounds);
    const resolveHandle = (handle) => {
      if (typeof handle?.pointIndex === "number") {
        return scene.points[handle.pointIndex] || { x: 0, y: 0 };
      }
      return handle || { x: 0, y: 0 };
    };

    scene.points.forEach((point) => {
      if (point.binding?.kind === "derived-parameter") {
        const value = parameterValueFromPoint(scene, point.binding.sourceIndex);
        if (value !== null) {
          applyNormalizedParameterToPoint(point, scene, value);
        }
      } else if (point.binding?.kind === "translate") {
        const source = scene.points[point.binding.sourceIndex];
        const vectorStart = scene.points[point.binding.vectorStartIndex];
        const vectorEnd = scene.points[point.binding.vectorEndIndex];
        if (!source || !vectorStart || !vectorEnd) return;
        point.x = source.x + (vectorEnd.x - vectorStart.x);
        point.y = source.y + (vectorEnd.y - vectorStart.y);
      } else if (point.binding?.kind === "reflect") {
        const source = scene.points[point.binding.sourceIndex];
        const lineStart = scene.points[point.binding.lineStartIndex];
        const lineEnd = scene.points[point.binding.lineEndIndex];
        if (!source || !lineStart || !lineEnd) return;
        const reflected = reflectAcrossLine(source, lineStart, lineEnd);
        point.x = reflected.x;
        point.y = reflected.y;
      } else if (point.binding?.kind === "rotate") {
        const source = scene.points[point.binding.sourceIndex];
        const center = scene.points[point.binding.centerIndex];
        if (!source || !center) return;
        const rotated = rotateAround(source, center, point.binding.angleDegrees * Math.PI / 180);
        point.x = rotated.x;
        point.y = rotated.y;
      } else if (point.binding?.kind === "scale") {
        const source = scene.points[point.binding.sourceIndex];
        const center = scene.points[point.binding.centerIndex];
        if (!source || !center) return;
        const scaled = scaleAround(source, center, point.binding.factor);
        point.x = scaled.x;
        point.y = scaled.y;
      }
    });

    scene.circles.forEach((circle) => {
      if (circle.binding?.kind === "rotate-circle") {
        const source = scene.circles[circle.binding.sourceIndex];
        const center = scene.points[circle.binding.centerIndex];
        if (!source || !center) return;
        const sourceCenter = resolveHandle(source.center);
        const sourceRadius = resolveHandle(source.radiusPoint);
        const radians = circle.binding.angleDegrees * Math.PI / 180;
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
        const radians = polygon.binding.angleDegrees * Math.PI / 180;
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
    const parameters = parameterMap(env);
    scene.lines.forEach((line) => {
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
      if (line.binding?.kind === "rotate-line") {
        const source = scene.lines[line.binding.sourceIndex];
        const center = scene.points[line.binding.centerIndex];
        if (source && center) {
          const radians = line.binding.angleDegrees * Math.PI / 180;
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
      }
    });
  }

  /** @param {ViewerEnv} env */
  function syncDynamicScene(env) {
    const parameters = parameterMap(env);
    env.updateScene((draft) => {
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
          }
          return;
        }
        const value = parameters.get(point.binding.name);
        if (!Number.isFinite(value)) return;
        applyNormalizedParameterToPoint(point, draft, value);
      });
      env.currentDynamics().functions.forEach((functionDef) => {
        if (draft.labels[functionDef.labelIndex]) {
          const head = functionDef.derivative ? `${functionDef.name}'(x)` : `${functionDef.name}(x)`;
          draft.labels[functionDef.labelIndex].text = `${head} = ${formatExpr(functionDef.expr, env.formatAxisNumber)}`;
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
    });
  }

  /** @param {ViewerEnv} env */
  function buildParameterControls(env) {
    env.parameterControls.replaceChildren();
    const controls = env.currentDynamics().parameters.map((parameter, index) => env.labelTag(
      `${parameter.name} =`,
      env.inputTag({
        type: "number",
        step: parameter.name === "n" ? "1" : "0.1",
        min: parameter.name === "n" ? "2" : undefined,
        value: parameter.value.toFixed(2),
        oninput: (event) => {
          let value = Number.parseFloat(event.target.value);
          if (Number.isFinite(value)) {
            if (parameter.name === "n") {
              value = Math.max(2, Math.round(value));
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
    syncDynamicScene,
  };
})();
