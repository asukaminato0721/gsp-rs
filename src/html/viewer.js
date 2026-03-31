(() => {
  const van = window.van;
  const { label, input } = van.tags;
  const sourceScene = JSON.parse(document.getElementById("scene-data").textContent);
  const canvas = document.getElementById("view");
  const ctx = canvas.getContext("2d");
  const resetButton = document.getElementById("reset-view");
  const parameterControls = document.getElementById("parameter-controls");
  const coordReadout = document.getElementById("coord-readout");
  const zoomReadout = document.getElementById("zoom-readout");
  const margin = 32;
  const trigMode = !!sourceScene.piMode;
  const savedViewportMode = !!sourceScene.savedViewport;
  const baseBounds = sourceScene.bounds;
  const baseCenterX = (baseBounds.minX + baseBounds.maxX) / 2;
  const baseCenterY = (baseBounds.minY + baseBounds.maxY) / 2;
  const baseSpanX = Math.max(1e-6, baseBounds.maxX - baseBounds.minX);
  const baseSpanY = Math.max(1e-6, baseBounds.maxY - baseBounds.minY);
  const pointHitRadius = 10;
  const pointMatchTolerance = 1e-3;
  const pointerWorldState = van.state(null);
  const viewState = van?.state ? van.state({
    centerX: baseCenterX,
    centerY: baseCenterY,
    zoom: 1,
  }) : { val: {
    centerX: baseCenterX,
    centerY: baseCenterY,
    zoom: 1,
  } };
  const view = new Proxy({}, {
    get: (_, key) => viewState.val[key],
    set: (_, key, value) => {
      viewState.val = { ...viewState.val, [key]: value };
      return true;
    },
  });
  const dragState = van?.state ? van.state(null) : { val: null };
  const hoverPointIndex = van?.state ? van.state(null) : { val: null };
  const labelAttachDistance = 40;
  const coordText = van.derive(() => {
    const world = pointerWorldState.val;
    return world ? `x ${formatNumber(world.x)}, y ${formatNumber(world.y)}` : "x -, y -";
  });
  const zoomText = van.derive(() => `zoom ${Math.round(viewState.val.zoom * 100)}%`);

  coordReadout.replaceChildren();
  zoomReadout.replaceChildren();
  van.add(coordReadout, coordText);
  van.add(zoomReadout, zoomText);

  function samePoint(left, right) {
    return Math.abs(left.x - right.x) < pointMatchTolerance
      && Math.abs(left.y - right.y) < pointMatchTolerance;
  }

  function resolveSourcePoint(index) {
    const point = sourceScene.points[index];
    if (!point) {
      return { x: 0, y: 0 };
    }
    const resolved = resolveConstrainedPoint(point.constraint, resolveSourcePoint);
    if (resolved) {
      return resolved;
    }
    return { x: point.x, y: point.y };
  }

  function attachPointRef(point) {
    const pointIndex = sourceScene.points.findIndex((candidate, index) => samePoint(resolveSourcePoint(index), point));
    if (pointIndex >= 0) {
      return { pointIndex };
    }
    return { x: point.x, y: point.y };
  }

  function resolveSourceHandle(handle) {
    if (typeof handle.pointIndex === "number") {
      return resolveSourcePoint(handle.pointIndex);
    }
    return handle;
  }

  function distanceSquared(left, right) {
    const dx = left.x - right.x;
    const dy = left.y - right.y;
    return dx * dx + dy * dy;
  }

  function attachLabelAnchor(point, hydratedLines) {
    let bestPointIndex = null;
    let bestPointDistanceSquared = Number.POSITIVE_INFINITY;
    sourceScene.points.forEach((candidate, index) => {
      const resolved = resolveSourcePoint(index);
      const distSq = distanceSquared(resolved, point);
      if (distSq < bestPointDistanceSquared) {
        bestPointDistanceSquared = distSq;
        bestPointIndex = index;
      }
    });
    if (bestPointIndex !== null && bestPointDistanceSquared <= labelAttachDistance ** 2) {
      const base = resolveSourcePoint(bestPointIndex);
      return {
        pointIndex: bestPointIndex,
        dx: point.x - base.x,
        dy: point.y - base.y,
      };
    }

    let bestLineAnchor = null;
    let bestLineDistanceSquared = Number.POSITIVE_INFINITY;
    hydratedLines.forEach((line, lineIndex) => {
      for (let segmentIndex = 0; segmentIndex < line.points.length - 1; segmentIndex += 1) {
        const start = resolveSourceHandle(line.points[segmentIndex]);
        const end = resolveSourceHandle(line.points[segmentIndex + 1]);
        const midpoint = {
          x: (start.x + end.x) / 2,
          y: (start.y + end.y) / 2,
        };
        const distSq = distanceSquared(midpoint, point);
        if (distSq < bestLineDistanceSquared) {
          bestLineDistanceSquared = distSq;
          bestLineAnchor = {
            lineIndex,
            segmentIndex,
            t: 0.5,
            dx: point.x - midpoint.x,
            dy: point.y - midpoint.y,
          };
        }
      }
    });
    if (bestLineAnchor && bestLineDistanceSquared <= labelAttachDistance ** 2) {
      return bestLineAnchor;
    }

    return { x: point.x, y: point.y };
  }

  function hydrateScene(scene) {
    const hydratedLines = scene.lines.map((line) => ({
      color: line.color,
      dashed: line.dashed,
      points: line.points.map(attachPointRef),
    }));
    return {
      graphMode: scene.graphMode,
      points: scene.points.map((point) => ({
        x: point.x,
        y: point.y,
        constraint: point.constraint ? { ...point.constraint } : null,
      })),
      origin: scene.origin ? attachPointRef(scene.origin) : null,
      lines: hydratedLines,
      polygons: scene.polygons.map((polygon) => ({
        color: polygon.color,
        outlineColor: polygon.outlineColor,
        points: polygon.points.map(attachPointRef),
      })),
      circles: scene.circles.map((circle) => ({
        color: circle.color,
        center: attachPointRef(circle.center),
        radiusPoint: attachPointRef(circle.radiusPoint),
      })),
      labels: scene.labels.map((label) => ({
        text: label.text,
        color: label.color,
        anchor: attachLabelAnchor(label.anchor, hydratedLines),
      })),
    };
  }

  const sceneState = van?.state ? van.state(hydrateScene(sourceScene)) : { val: hydrateScene(sourceScene) };
  const dynamicsState = van?.state ? van.state({
    parameters: (sourceScene.parameters || []).map((parameter) => ({ ...parameter })),
    functions: (sourceScene.functions || []).map((functionDef) => ({
      ...functionDef,
      expr: functionDef.expr,
      domain: functionDef.domain,
      constrainedPointIndices: [...functionDef.constrainedPointIndices],
    })),
  }) : { val: {
    parameters: (sourceScene.parameters || []).map((parameter) => ({ ...parameter })),
    functions: (sourceScene.functions || []).map((functionDef) => ({
      ...functionDef,
      expr: functionDef.expr,
      domain: functionDef.domain,
      constrainedPointIndices: [...functionDef.constrainedPointIndices],
    })),
  } };
  const currentScene = () => sceneState.val;
  const currentDynamics = () => dynamicsState.val;

  function updateScene(mutator) {
    const next = sceneState.val;
    mutator(next);
    sceneState.val = { ...next };
  }

  function updateDynamics(mutator) {
    const next = dynamicsState.val;
    mutator(next);
    dynamicsState.val = { ...next };
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

  function formatExprTerm(term) {
    switch (term.kind) {
      case "variable": return "x";
      case "constant": return formatAxisNumber(term.value);
      case "parameter": return term.name;
      case "unary_x": return `${term.op}(x)`;
      case "product": return `${formatExprTerm(term.left)}*${formatExprTerm(term.right)}`;
      default: return "?";
    }
  }

  function formatExpr(expr) {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return "x";
    if (expr.kind === "parsed") {
      let text = formatExprTerm(expr.head);
      for (const part of expr.tail) {
        text += part.op === "sub" ? " - " : " + ";
        text += formatExprTerm(part.term);
      }
      return text;
    }
    return "?";
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
      value = part.op === "sub" ? value - rhs : value + rhs;
    }
    return Number.isFinite(value) ? value : null;
  }

  function parameterMap() {
    return new Map(currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]));
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

  function syncDynamicScene() {
    const parameters = parameterMap();
    updateScene((draft) => {
      currentDynamics().parameters.forEach((parameter) => {
        if (draft.labels[parameter.labelIndex]) {
          draft.labels[parameter.labelIndex].text = `${parameter.name} = ${parameter.value.toFixed(2)}`;
        }
      });
      currentDynamics().functions.forEach((functionDef) => {
        if (draft.labels[functionDef.labelIndex]) {
          const head = functionDef.derivative ? `${functionDef.name}'(x)` : `${functionDef.name}(x)`;
          draft.labels[functionDef.labelIndex].text = `${head} = ${formatExpr(functionDef.expr)}`;
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

  function buildParameterControls() {
    parameterControls.replaceChildren();
    van.add(parameterControls, () => currentDynamics().parameters.map((parameter, index) => label(
      `${parameter.name} =`,
      input({
        type: "number",
        step: "0.1",
        value: parameter.value.toFixed(2),
        oninput: (event) => {
          const value = Number.parseFloat(event.target.value);
          if (Number.isFinite(value)) {
            updateDynamics((draft) => {
              draft.parameters[index].value = value;
            });
            syncDynamicScene();
          }
        },
      }),
    )));
  }

  function getViewBounds() {
    const spanX = baseSpanX / view.zoom;
    const spanY = baseSpanY / view.zoom;
    return {
      minX: view.centerX - spanX / 2,
      maxX: view.centerX + spanX / 2,
      minY: view.centerY - spanY / 2,
      maxY: view.centerY + spanY / 2,
      spanX,
      spanY,
    };
  }

  function resolvePoint(handle) {
    if (typeof handle.pointIndex === "number") {
      const point = resolveScenePoint(handle.pointIndex);
      return {
        x: point.x + (handle.dx || 0),
        y: point.y + (handle.dy || 0),
      };
    }
    if (typeof handle.lineIndex === "number") {
      const line = currentScene().lines[handle.lineIndex];
      if (!line || line.points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      const start = resolvePoint(line.points[segmentIndex]);
      const end = resolvePoint(line.points[segmentIndex + 1]);
      return {
        x: start.x + (end.x - start.x) * t + (handle.dx || 0),
        y: start.y + (end.y - start.y) * t + (handle.dy || 0),
      };
    }
    return handle;
  }

  function resolveAnchorBase(handle) {
    if (typeof handle.pointIndex === "number") {
      return resolveScenePoint(handle.pointIndex);
    }
    if (typeof handle.lineIndex === "number") {
      const line = currentScene().lines[handle.lineIndex];
      if (!line || line.points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      const start = resolvePoint(line.points[segmentIndex]);
      const end = resolvePoint(line.points[segmentIndex + 1]);
      return {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
      };
    }
    return handle;
  }

  function resolveScenePoint(index) {
    const point = currentScene().points[index];
    if (!point) {
      return { x: 0, y: 0 };
    }
    const resolved = resolveConstrainedPoint(point.constraint, resolveScenePoint);
    if (resolved) {
      return resolved;
    }
    return point;
  }

  function resolveConstrainedPoint(constraint, resolveFn) {
    if (!constraint) {
      return null;
    }
    if (constraint.kind === "offset") {
      const origin = resolveFn(constraint.originIndex);
      return {
        x: origin.x + constraint.dx,
        y: origin.y + constraint.dy,
      };
    }
    if (constraint.kind === "segment") {
      const start = resolveFn(constraint.startIndex);
      const end = resolveFn(constraint.endIndex);
      return {
        x: start.x + (end.x - start.x) * constraint.t,
        y: start.y + (end.y - start.y) * constraint.t,
      };
    }
    if (constraint.kind === "polyline") {
      const count = constraint.points.length;
      if (count < 2) {
        return null;
      }
      const segmentIndex = Math.max(0, Math.min(count - 2, constraint.segmentIndex));
      const start = constraint.points[segmentIndex];
      const end = constraint.points[segmentIndex + 1];
      return {
        x: start.x + (end.x - start.x) * constraint.t,
        y: start.y + (end.y - start.y) * constraint.t,
      };
    }
    if (constraint.kind === "polygon-boundary") {
      const count = constraint.vertexIndices.length;
      if (count < 2) {
        return null;
      }
      const start = resolveFn(constraint.vertexIndices[((constraint.edgeIndex % count) + count) % count]);
      const end = resolveFn(constraint.vertexIndices[(constraint.edgeIndex + 1 + count) % count]);
      return {
        x: start.x + (end.x - start.x) * constraint.t,
        y: start.y + (end.y - start.y) * constraint.t,
      };
    }
    if (constraint.kind === "circle") {
      const center = resolveFn(constraint.centerIndex);
      const radiusPoint = resolveFn(constraint.radiusIndex);
      const radius = Math.hypot(radiusPoint.x - center.x, radiusPoint.y - center.y);
      return {
        x: center.x + radius * constraint.unitX,
        y: center.y + radius * constraint.unitY,
      };
    }
    return null;
  }

  function toScreen(point) {
    const usableWidth = Math.max(1, sourceScene.width - margin * 2);
    const usableHeight = Math.max(1, sourceScene.height - margin * 2);
    const bounds = getViewBounds();
    const scale = Math.min(usableWidth / bounds.spanX, usableHeight / bounds.spanY);
    return {
      x: margin + (point.x - bounds.minX) * scale,
      y: sourceScene.yUp
        ? sourceScene.height - margin - (point.y - bounds.minY) * scale
        : margin + (point.y - bounds.minY) * scale,
      scale,
    };
  }

  function toWorld(screenX, screenY) {
    const usableWidth = Math.max(1, sourceScene.width - margin * 2);
    const usableHeight = Math.max(1, sourceScene.height - margin * 2);
    const bounds = getViewBounds();
    const scale = Math.min(usableWidth / bounds.spanX, usableHeight / bounds.spanY);
    return {
      x: bounds.minX + (screenX - margin) / scale,
      y: sourceScene.yUp
        ? bounds.minY + (sourceScene.height - margin - screenY) / scale
        : bounds.minY + (screenY - margin) / scale,
      scale,
    };
  }

  function getCanvasCoords(event) {
    const rect = canvas.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left) * (sourceScene.width / rect.width),
      y: (event.clientY - rect.top) * (sourceScene.height / rect.height),
    };
  }

  function rgba(color) {
    return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${(color[3] / 255).toFixed(3)})`;
  }

  function formatNumber(value) {
    return Number.isFinite(value) ? value.toFixed(2) : "-";
  }

  function chooseGridStep(span, targetLines) {
    const rough = Math.max(1e-6, span / Math.max(1, targetLines));
    const magnitude = 10 ** Math.floor(Math.log10(rough));
    const normalized = rough / magnitude;
    if (normalized <= 1) return magnitude;
    if (normalized <= 2) return magnitude * 2;
    if (normalized <= 5) return magnitude * 5;
    return magnitude * 10;
  }

  function formatAxisNumber(value) {
    if (Math.abs(value - Math.round(value)) < 1e-6) {
      return String(Math.round(value));
    }
    return value.toFixed(1);
  }

  function formatPiLabel(stepIndex) {
    if (stepIndex === 0) return "";
    const sign = stepIndex < 0 ? "-" : "";
    const absIndex = Math.abs(stepIndex);
    if (absIndex % 2 === 0) {
      const multiple = absIndex / 2;
      return multiple === 1 ? `${sign}\u03c0` : `${sign}${multiple}\u03c0`;
    }
    return absIndex === 1 ? `${sign}\u03c0/2` : `${sign}${absIndex}\u03c0/2`;
  }

  function updateReadout(screenX = null, screenY = null) {
    if (screenX === null || screenY === null) {
      pointerWorldState.val = null;
      return;
    }
    pointerWorldState.val = toWorld(screenX, screenY);
  }

  function resetView() {
    view.centerX = baseCenterX;
    view.centerY = baseCenterY;
    view.zoom = 1;
    updateReadout();
  }

  function drawGrid() {
    if (!currentScene().graphMode) return;
    const bounds = getViewBounds();
    const spanY = bounds.maxY - bounds.minY;
    const yMinorStep = savedViewportMode ? 1 : chooseGridStep(spanY, 14);
    const yMajorStep = savedViewportMode ? 2 : chooseGridStep(spanY, 7);
    const minYIndex = Math.floor(bounds.minY / yMinorStep);
    const maxYIndex = Math.ceil(bounds.maxY / yMinorStep);

    ctx.save();
    ctx.lineWidth = 1;
    ctx.font = "12px \"Noto Sans\", \"Segoe UI\", sans-serif";
    ctx.fillStyle = "rgb(20,20,20)";
    const xAxisY = bounds.minY <= 0 && 0 <= bounds.maxY
      ? toScreen({ x: bounds.minX, y: 0 }).y
      : sourceScene.height - 18;
    const yAxisX = bounds.minX <= 0 && 0 <= bounds.maxX
      ? toScreen({ x: 0, y: bounds.minY }).x
      : sourceScene.width / 2;
    if (trigMode) {
      const xMinorStep = Math.PI / 2;
      const startIndex = Math.ceil(bounds.minX / xMinorStep);
      const endIndex = Math.floor(bounds.maxX / xMinorStep);
      for (let stepIndex = startIndex; stepIndex <= endIndex; stepIndex += 1) {
        const x = stepIndex * xMinorStep;
        const screen = toScreen({ x, y: bounds.minY });
        const major = stepIndex % 2 === 0;
        ctx.strokeStyle = Math.abs(x) < 1e-9
          ? "rgb(40,40,40)"
          : major
            ? "rgb(190,190,190)"
            : "rgb(220,220,220)";
        ctx.beginPath();
        ctx.moveTo(screen.x, 0);
        ctx.lineTo(screen.x, sourceScene.height);
        ctx.stroke();
        if (bounds.minY <= 0 && 0 <= bounds.maxY) {
          ctx.strokeStyle = "rgb(40,40,40)";
          ctx.beginPath();
          ctx.moveTo(screen.x, xAxisY - (major ? 6 : 4));
          ctx.lineTo(screen.x, xAxisY + (major ? 6 : 4));
          ctx.stroke();
        }
        if (major && stepIndex !== 0) {
          const label = formatPiLabel(stepIndex);
          const width = ctx.measureText(label).width;
          ctx.fillText(
            label,
            screen.x - width / 2,
            Math.min(sourceScene.height - 4, xAxisY + 16),
          );
        }
      }
    } else {
      const spanX = bounds.maxX - bounds.minX;
      const xLabelStep = spanX > 20 ? 5 : 2;
      const minX = Math.floor(bounds.minX);
      const maxX = Math.ceil(bounds.maxX);
      for (let x = minX; x <= maxX; x += 1) {
        const screen = toScreen({ x, y: bounds.minY });
        ctx.strokeStyle = x === 0 ? "rgb(40,40,40)" : "rgb(200,200,200)";
        ctx.beginPath();
        ctx.moveTo(screen.x, 0);
        ctx.lineTo(screen.x, sourceScene.height);
        ctx.stroke();
        if (bounds.minY <= 0 && 0 <= bounds.maxY) {
          ctx.strokeStyle = "rgb(40,40,40)";
          ctx.beginPath();
          ctx.moveTo(screen.x, xAxisY - (x === 0 ? 6 : 4));
          ctx.lineTo(screen.x, xAxisY + (x === 0 ? 6 : 4));
          ctx.stroke();
        }
        if (x !== 0 && x % xLabelStep === 0) {
          const label = String(x);
          const width = ctx.measureText(label).width;
          ctx.fillText(
            label,
            screen.x - width / 2,
            Math.min(sourceScene.height - 4, xAxisY + 16),
          );
        }
      }
    }
    for (let yIndex = minYIndex; yIndex <= maxYIndex; yIndex += 1) {
      const y = yIndex * yMinorStep;
      const major = Math.abs((y / yMajorStep) - Math.round(y / yMajorStep)) < 1e-6;
      const screen = toScreen({ x: bounds.minX, y });
      ctx.strokeStyle = Math.abs(y) < 1e-6
        ? "rgb(40,40,40)"
        : major
          ? "rgb(200,200,200)"
          : "rgb(225,225,225)";
      ctx.beginPath();
      ctx.moveTo(0, screen.y);
      ctx.lineTo(sourceScene.width, screen.y);
      ctx.stroke();
      if (bounds.minX <= 0 && 0 <= bounds.maxX) {
        ctx.strokeStyle = "rgb(40,40,40)";
        ctx.beginPath();
        ctx.moveTo(yAxisX - (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2), screen.y);
        ctx.lineTo(yAxisX + (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2), screen.y);
        ctx.stroke();
      }
      if (major && Math.abs(y) >= 1e-6) {
        const label = formatAxisNumber(y);
        const width = ctx.measureText(label).width;
        ctx.fillText(label, yAxisX - width - 8, screen.y - 6);
      }
    }
    if (currentScene().origin) {
      const origin = toScreen(resolvePoint(currentScene().origin));
      ctx.fillStyle = "rgba(255, 60, 40, 1)";
      ctx.beginPath();
      ctx.arc(origin.x, origin.y, 3, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }

  function findHitPoint(screenX, screenY) {
    let bestIndex = null;
    let bestDistanceSquared = pointHitRadius * pointHitRadius;
    currentScene().points.forEach((_, index) => {
      const screen = toScreen(resolveScenePoint(index));
      const dx = screen.x - screenX;
      const dy = screen.y - screenY;
      const distanceSquared = dx * dx + dy * dy;
      if (distanceSquared <= bestDistanceSquared) {
        bestDistanceSquared = distanceSquared;
        bestIndex = index;
      }
    });
    return bestIndex;
  }

  function isOriginPointIndex(index) {
    return typeof currentScene().origin?.pointIndex === "number" && currentScene().origin.pointIndex === index;
  }

  function findHitLabel(screenX, screenY) {
    ctx.save();
    ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    ctx.textBaseline = "top";
    for (let index = currentScene().labels.length - 1; index >= 0; index -= 1) {
      const label = currentScene().labels[index];
      const screen = toScreen(resolvePoint(label.anchor));
      const lines = label.text.split("\n");
      const width = lines.reduce((best, line) => Math.max(best, ctx.measureText(line).width), 0);
      const height = lines.length * 22;
      const left = screen.x + 2;
      const top = screen.y - 14;
      if (
        screenX >= left
        && screenX <= left + width + 8
        && screenY >= top
        && screenY <= top + height
      ) {
        ctx.restore();
        return index;
      }
    }
    ctx.restore();
    return null;
  }

  function draw() {
    ctx.clearRect(0, 0, sourceScene.width, sourceScene.height);
    ctx.fillStyle = "rgb(250,250,248)";
    ctx.fillRect(0, 0, sourceScene.width, sourceScene.height);
    drawGrid();

    for (const polygon of currentScene().polygons) {
      if (polygon.points.length < 3) continue;
      ctx.beginPath();
      polygon.points.forEach((handle, index) => {
        const screen = toScreen(resolvePoint(handle));
        if (index === 0) {
          ctx.moveTo(screen.x, screen.y);
        } else {
          ctx.lineTo(screen.x, screen.y);
        }
      });
      ctx.closePath();
      ctx.fillStyle = rgba(polygon.color);
      ctx.strokeStyle = rgba(polygon.outlineColor);
      ctx.lineWidth = 1.5;
      ctx.fill();
      ctx.stroke();
    }

    for (const line of currentScene().lines) {
      if (line.points.length < 2) continue;
      ctx.beginPath();
      line.points.forEach((handle, index) => {
        const screen = toScreen(resolvePoint(handle));
        if (index === 0) {
          ctx.moveTo(screen.x, screen.y);
        } else {
          ctx.lineTo(screen.x, screen.y);
        }
      });
      ctx.strokeStyle = rgba(line.color);
      ctx.lineWidth = 2;
      ctx.setLineDash(line.dashed ? [8, 8] : []);
      ctx.stroke();
    }
    ctx.setLineDash([]);

    for (const circle of currentScene().circles) {
      const centerWorld = resolvePoint(circle.center);
      const radiusPointWorld = resolvePoint(circle.radiusPoint);
      const center = toScreen(centerWorld);
      const radius = Math.hypot(
        radiusPointWorld.x - centerWorld.x,
        radiusPointWorld.y - centerWorld.y,
      ) * center.scale;
      ctx.beginPath();
      ctx.arc(center.x, center.y, radius, 0, Math.PI * 2);
      ctx.strokeStyle = rgba(circle.color);
      ctx.lineWidth = 2;
      ctx.stroke();
    }

    currentScene().points.forEach((point, index) => {
      const screen = toScreen(resolveScenePoint(index));
      ctx.beginPath();
      ctx.arc(screen.x, screen.y, index === hoverPointIndex.val ? 6 : 4, 0, Math.PI * 2);
      ctx.fillStyle = index === hoverPointIndex.val ? "rgba(255, 120, 20, 1)" : "rgba(255, 60, 40, 1)";
      ctx.fill();
    });

    ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    ctx.textBaseline = "top";
    for (const label of currentScene().labels) {
      const screen = toScreen(resolvePoint(label.anchor));
      ctx.fillStyle = rgba(label.color);
      const lines = label.text.split("\n");
      lines.forEach((line, index) => {
        ctx.fillText(line, screen.x + 6, screen.y - 10 + index * 22);
      });
    }
  }

  van.derive(() => {
    draw();
    return 0;
  });

  canvas.addEventListener("pointerdown", (event) => {
    const position = getCanvasCoords(event);
    const pointIndex = findHitPoint(position.x, position.y);
    const labelIndex = pointIndex === null ? findHitLabel(position.x, position.y) : null;
    dragState.val = {
      pointerId: event.pointerId,
      mode: pointIndex !== null
        ? (currentScene().graphMode && isOriginPointIndex(pointIndex) ? "origin-pan" : "point")
        : (labelIndex !== null ? "label" : "pan"),
      pointIndex,
      labelIndex,
      lastX: position.x,
      lastY: position.y,
    };
    hoverPointIndex.val = pointIndex;
    canvas.classList.add("is-dragging");
    canvas.setPointerCapture(event.pointerId);
  });

  canvas.addEventListener("pointermove", (event) => {
    const position = getCanvasCoords(event);
    updateReadout(position.x, position.y);
    hoverPointIndex.val = findHitPoint(position.x, position.y);
    if (!dragState.val || dragState.val.pointerId !== event.pointerId) {
      return;
    }
    if (dragState.val.mode === "point") {
      const world = toWorld(position.x, position.y);
      updateScene((draft) => {
        const point = draft.points[dragState.val.pointIndex];
        if (point.constraint && point.constraint.kind === "offset") {
          const originPoint = draft.points[point.constraint.originIndex];
          if (originPoint && !originPoint.constraint) {
            originPoint.x = world.x - point.constraint.dx;
            originPoint.y = world.y - point.constraint.dy;
          } else {
            const origin = resolveScenePoint(point.constraint.originIndex);
            point.constraint.dx = world.x - origin.x;
            point.constraint.dy = world.y - origin.y;
          }
        } else if (point.constraint && point.constraint.kind === "segment") {
          const start = resolveScenePoint(point.constraint.startIndex);
          const end = resolveScenePoint(point.constraint.endIndex);
          const dx = end.x - start.x;
          const dy = end.y - start.y;
          const lengthSquared = dx * dx + dy * dy;
          if (lengthSquared > 1e-9) {
            const t = ((world.x - start.x) * dx + (world.y - start.y) * dy) / lengthSquared;
            point.constraint.t = Math.max(0, Math.min(1, t));
          }
        } else if (point.constraint && point.constraint.kind === "polyline") {
          const count = point.constraint.points.length;
          let bestSegmentIndex = point.constraint.segmentIndex;
          let bestT = point.constraint.t;
          let bestDistanceSquared = Number.POSITIVE_INFINITY;
          for (let segmentIndex = 0; segmentIndex < count - 1; segmentIndex += 1) {
            const start = point.constraint.points[segmentIndex];
            const end = point.constraint.points[segmentIndex + 1];
            const dx = end.x - start.x;
            const dy = end.y - start.y;
            const lengthSquared = dx * dx + dy * dy;
            if (lengthSquared <= 1e-9) {
              continue;
            }
            const t = Math.max(0, Math.min(1, ((world.x - start.x) * dx + (world.y - start.y) * dy) / lengthSquared));
            const projX = start.x + dx * t;
            const projY = start.y + dy * t;
            const distSq = (world.x - projX) ** 2 + (world.y - projY) ** 2;
            if (distSq < bestDistanceSquared) {
              bestDistanceSquared = distSq;
              bestSegmentIndex = segmentIndex;
              bestT = t;
            }
          }
          point.constraint.segmentIndex = bestSegmentIndex;
          point.constraint.t = bestT;
        } else if (point.constraint && point.constraint.kind === "polygon-boundary") {
          const count = point.constraint.vertexIndices.length;
          let bestEdgeIndex = point.constraint.edgeIndex;
          let bestT = point.constraint.t;
          let bestDistanceSquared = Number.POSITIVE_INFINITY;
          for (let edgeIndex = 0; edgeIndex < count; edgeIndex += 1) {
            const start = resolveScenePoint(point.constraint.vertexIndices[edgeIndex]);
            const end = resolveScenePoint(point.constraint.vertexIndices[(edgeIndex + 1) % count]);
            const dx = end.x - start.x;
            const dy = end.y - start.y;
            const lengthSquared = dx * dx + dy * dy;
            if (lengthSquared <= 1e-9) {
              continue;
            }
            const t = Math.max(0, Math.min(1, ((world.x - start.x) * dx + (world.y - start.y) * dy) / lengthSquared));
            const projX = start.x + dx * t;
            const projY = start.y + dy * t;
            const distSq = (world.x - projX) ** 2 + (world.y - projY) ** 2;
            if (distSq < bestDistanceSquared) {
              bestDistanceSquared = distSq;
              bestEdgeIndex = edgeIndex;
              bestT = t;
            }
          }
          point.constraint.edgeIndex = bestEdgeIndex;
          point.constraint.t = bestT;
        } else if (point.constraint && point.constraint.kind === "circle") {
          const center = resolveScenePoint(point.constraint.centerIndex);
          const dx = world.x - center.x;
          const dy = world.y - center.y;
          const length = Math.hypot(dx, dy);
          if (length > 1e-9) {
            point.constraint.unitX = dx / length;
            point.constraint.unitY = dy / length;
          }
        } else {
          point.x = world.x;
          point.y = world.y;
        }
      });
      hoverPointIndex.val = dragState.val.pointIndex;
    } else if (dragState.val.mode === "label") {
      const world = toWorld(position.x, position.y);
      updateScene((draft) => {
        const label = draft.labels[dragState.val.labelIndex];
        if (typeof label.anchor.pointIndex === "number" || typeof label.anchor.lineIndex === "number") {
          const base = resolveAnchorBase(label.anchor);
          label.anchor.dx = world.x - base.x;
          label.anchor.dy = world.y - base.y;
        } else {
          label.anchor.x = world.x;
          label.anchor.y = world.y;
        }
      });
    } else {
      const worldNow = toWorld(position.x, position.y);
      const worldLast = toWorld(dragState.val.lastX, dragState.val.lastY);
      view.centerX -= worldNow.x - worldLast.x;
      view.centerY -= worldNow.y - worldLast.y;
    }
    dragState.val = { ...dragState.val, lastX: position.x, lastY: position.y };
  });

  function endDrag(pointerId) {
    if (dragState.val && dragState.val.pointerId === pointerId) {
      dragState.val = null;
      canvas.classList.remove("is-dragging");
    }
  }

  canvas.addEventListener("pointerup", (event) => endDrag(event.pointerId));
  canvas.addEventListener("pointercancel", (event) => endDrag(event.pointerId));
  canvas.addEventListener("pointerleave", () => {
    hoverPointIndex.val = null;
    if (!dragState.val) {
      updateReadout();
    }
  });

  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const position = getCanvasCoords(event);
    const before = toWorld(position.x, position.y);
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    view.zoom = Math.max(0.25, Math.min(64, view.zoom * factor));
    const after = toWorld(position.x, position.y);
    view.centerX += before.x - after.x;
    view.centerY += before.y - after.y;
    updateReadout(position.x, position.y);
  }, { passive: false });

  canvas.addEventListener("dblclick", () => {
    resetView();
  });

  resetButton.addEventListener("click", () => {
    resetView();
  });

  window.addEventListener("keydown", (event) => {
    if (event.key === "0") {
      resetView();
    }
  });

  syncDynamicScene();
  buildParameterControls();
  resetView();
})();
