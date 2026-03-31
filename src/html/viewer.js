(() => {
  const van = window.van;
  const { label, input } = van.tags;
  const { scene: sceneModule, render: renderModule, drag: dragModule } = window.GspViewerModules;
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
    const resolved = sceneModule.resolveConstrainedPoint(
      null,
      point.constraint,
      resolveSourcePoint,
    );
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
        binding: label.binding ? { ...label.binding } : null,
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
    refreshDynamicLabels(next);
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

  function rgba(color) {
    return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${(color[3] / 255).toFixed(3)})`;
  }

  function formatNumber(value) {
    return Number.isFinite(value) ? value.toFixed(2) : "-";
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

  function refreshDynamicLabels(scene) {
    scene.labels.forEach((label) => {
      if (!label.binding) return;
      if (label.binding.kind === "polygon-boundary-parameter") {
        const value = polygonBoundaryParameterFromPoint(scene, label.binding.pointIndex);
        if (value !== null) {
          label.text = `${label.binding.pointName}在${label.binding.polygonName}上的t值 = ${formatNumber(value)}`;
        }
      } else if (label.binding.kind === "segment-parameter") {
        const point = scene.points[label.binding.pointIndex];
        const value = point?.constraint?.kind === "segment" ? point.constraint.t : null;
        if (value !== null) {
          label.text = `${label.binding.pointName}在${label.binding.segmentName}上的t值 = ${formatNumber(value)}`;
        }
      }
    });
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
    pointerWorldState.val = sceneModule.toWorld(viewerEnv, screenX, screenY);
  }

  function resetView() {
    view.centerX = baseCenterX;
    view.centerY = baseCenterY;
    view.zoom = 1;
    updateReadout();
  }

  function findHitPoint(screenX, screenY) {
    return renderModule.findHitPoint(viewerEnv, screenX, screenY);
  }

  function isOriginPointIndex(index) {
    return typeof currentScene().origin?.pointIndex === "number" && currentScene().origin.pointIndex === index;
  }

  function findHitLabel(screenX, screenY) {
    return renderModule.findHitLabel(viewerEnv, screenX, screenY);
  }

  function beginDrag(pointerId, position, pointIndex, labelIndex) {
    dragModule.beginDrag(viewerEnv, pointerId, position, pointIndex, labelIndex);
  }

  function updateDraggedPoint(world) {
    dragModule.updateDraggedPoint(viewerEnv, world);
  }

  function updateDraggedLabel(world) {
    dragModule.updateDraggedLabel(viewerEnv, world);
  }

  function panFromPointerDelta(position) {
    dragModule.panFromPointerDelta(viewerEnv, position);
  }

  function draw() {
    renderModule.draw(viewerEnv);
  }

  const viewerEnv = {
    canvas,
    ctx,
    sourceScene,
    margin,
    trigMode,
    savedViewportMode,
    baseSpanX,
    baseSpanY,
    pointHitRadius,
    hoverPointIndex,
    dragState,
    view,
    currentScene,
    resolveScenePoint: (index) => sceneModule.resolveScenePoint(viewerEnv, index),
    resolvePoint: (handle) => sceneModule.resolvePoint(viewerEnv, handle),
    resolveAnchorBase: (handle) => sceneModule.resolveAnchorBase(viewerEnv, handle),
    toScreen: (point) => sceneModule.toScreen(viewerEnv, point),
    toWorld: (x, y) => sceneModule.toWorld(viewerEnv, x, y),
    getViewBounds: () => sceneModule.getViewBounds(viewerEnv),
    rgba,
    updateScene,
    isOriginPointIndex,
    formatAxisNumber,
    formatPiLabel,
    drawGrid: () => sceneModule.drawGrid(viewerEnv),
  };

  van.derive(() => {
    draw();
    return 0;
  });

  canvas.addEventListener("pointerdown", (event) => {
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const pointIndex = findHitPoint(position.x, position.y);
    const labelIndex = pointIndex === null ? findHitLabel(position.x, position.y) : null;
    beginDrag(event.pointerId, position, pointIndex, labelIndex);
    canvas.setPointerCapture(event.pointerId);
  });

  canvas.addEventListener("pointermove", (event) => {
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    updateReadout(position.x, position.y);
    hoverPointIndex.val = findHitPoint(position.x, position.y);
    if (!dragState.val || dragState.val.pointerId !== event.pointerId) {
      return;
    }
    if (dragState.val.mode === "point") {
      updateDraggedPoint(sceneModule.toWorld(viewerEnv, position.x, position.y));
    } else if (dragState.val.mode === "label") {
      updateDraggedLabel(sceneModule.toWorld(viewerEnv, position.x, position.y));
    } else {
      panFromPointerDelta(position);
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
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const before = sceneModule.toWorld(viewerEnv, position.x, position.y);
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    view.zoom = Math.max(0.25, Math.min(64, view.zoom * factor));
    const after = sceneModule.toWorld(viewerEnv, position.x, position.y);
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
