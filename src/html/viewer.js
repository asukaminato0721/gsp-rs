// @ts-check

(() => {
  const van = window.van;
  const { label, input } = van.tags;
  const {
    scene: sceneModule,
    render: renderModule,
    drag: dragModule,
    dynamics: dynamicsModule,
  } = window.GspViewerModules;
  /** @type {SceneData} */
  const sourceScene = JSON.parse(document.getElementById("scene-data").textContent);
  /** @type {HTMLCanvasElement} */
  const canvas = /** @type {HTMLCanvasElement} */ (document.getElementById("view"));
  const ctx = canvas.getContext("2d");
  /** @type {HTMLButtonElement} */
  const resetButton = /** @type {HTMLButtonElement} */ (document.getElementById("reset-view"));
  /** @type {HTMLElement} */
  const parameterControls = /** @type {HTMLElement} */ (document.getElementById("parameter-controls"));
  /** @type {HTMLElement} */
  const buttonOverlays = /** @type {HTMLElement} */ (document.getElementById("button-overlays"));
  /** @type {HTMLElement} */
  const coordReadout = /** @type {HTMLElement} */ (document.getElementById("coord-readout"));
  /** @type {HTMLElement} */
  const zoomReadout = /** @type {HTMLElement} */ (document.getElementById("zoom-readout"));
  const margin = 32;
  const trigMode = !!sourceScene.piMode;
  const savedViewportMode = !!sourceScene.savedViewport;
  const baseBounds = sourceScene.bounds;
  const baseCenterX = (baseBounds.minX + baseBounds.maxX) / 2;
  const baseCenterY = (baseBounds.minY + baseBounds.maxY) / 2;
  const baseSpanX = Math.max(1e-6, baseBounds.maxX - baseBounds.minX);
  const baseSpanY = Math.max(1e-6, baseBounds.maxY - baseBounds.minY);
  const minZoom = 0.05;
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
  /** @type {ViewState} */
  const view = new Proxy(/** @type {ViewState} */ ({}), {
    get: (_, key) => viewState.val[key],
    set: (_, key, value) => {
      viewState.val = { ...viewState.val, [key]: value };
      return true;
    },
  });
  const dragState = van?.state ? van.state(null) : { val: null };
  const hoverPointIndex = van?.state ? van.state(null) : { val: null };
  const buttonsState = van?.state ? van.state((sourceScene.buttons || []).map((button) => ({
    ...button,
    visible: true,
    active: false,
  }))) : { val: (sourceScene.buttons || []).map((button) => ({
    ...button,
    visible: true,
    active: false,
  })) };
  const buttonTimers = new Map();
  const buttonAnimations = new Map();
  let buttonPointerState = null;
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

  function attachPointCenteredLabelAnchor(label, hydratedLines) {
    if (typeof label.binding?.pointIndex === "number") {
      return { pointIndex: label.binding.pointIndex };
    }
    return attachLabelAnchor(label.anchor, hydratedLines);
  }

  function hydrateScene(scene) {
    const hydratedLines = scene.lines.map((line) => ({
      color: line.color,
      dashed: line.dashed,
      points: line.points.map(attachPointRef),
      binding: line.binding ? { ...line.binding } : null,
    }));
    return {
      graphMode: scene.graphMode,
      bounds: scene.bounds ? { ...scene.bounds } : null,
      points: scene.points.map((point) => ({
        x: point.x,
        y: point.y,
        visible: true,
        constraint: point.constraint ? { ...point.constraint } : null,
        binding: point.binding ? { ...point.binding } : null,
      })),
      origin: scene.origin ? attachPointRef(scene.origin) : null,
      lines: hydratedLines,
      polygons: scene.polygons.map((polygon) => ({
        color: polygon.color,
        outlineColor: polygon.outlineColor,
        visible: true,
        points: polygon.points.map(attachPointRef),
        binding: polygon.binding ? { ...polygon.binding } : null,
      })),
      circles: scene.circles.map((circle) => ({
        color: circle.color,
        visible: true,
        center: attachPointRef(circle.center),
        radiusPoint: attachPointRef(circle.radiusPoint),
        binding: circle.binding ? { ...circle.binding } : null,
      })),
      labels: scene.labels.map((label) => ({
        text: label.text,
        color: label.color,
        visible: true,
        anchor: label.screenSpace
          ? { ...label.anchor }
          : label.binding?.kind === "point-expression-value"
            ? attachPointCenteredLabelAnchor(label, hydratedLines)
            : attachLabelAnchor(label.anchor, hydratedLines),
        binding: label.binding ? { ...label.binding } : null,
        screenSpace: !!label.screenSpace,
        centeredOnAnchor: label.binding?.kind === "point-expression-value",
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
    dynamicsModule.refreshDerivedPoints(viewerEnv, next);
    dynamicsModule.refreshDynamicLabels(viewerEnv, next);
    sceneState.val = { ...next };
  }

  function updateDynamics(mutator) {
    const next = dynamicsState.val;
    mutator(next);
    dynamicsState.val = { ...next };
  }

  function updateButtons(mutator) {
    const next = buttonsState.val.slice();
    mutator(next);
    buttonsState.val = next;
  }

  function rgba(color) {
    return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${(color[3] / 255).toFixed(3)})`;
  }

  function formatNumber(value) {
    return Number.isFinite(value) ? value.toFixed(2) : "-";
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
    dynamicsModule.syncDynamicScene(viewerEnv);
    updateReadout();
  }

  function renderButtons() {
    if (!buttonOverlays) {
      return;
    }
    buttonOverlays.replaceChildren();
    const stackedOffsets = new Map();
    buttonsState.val.forEach((buttonDef, buttonIndex) => {
      if (buttonDef.visible === false) {
        return;
      }
      const anchor = document.createElement("button");
      anchor.className = "scene-link-button";
      anchor.setAttribute("aria-pressed", buttonDef.active ? "true" : "false");
      if (buttonDef.active) {
        anchor.classList.add("is-active");
      }
      anchor.type = "button";
      anchor.textContent = buttonDef.text;
      const key = `${Math.round(buttonDef.x)}:${Math.round(buttonDef.y)}`;
      const stackedOffset = stackedOffsets.get(key) || 0;
      stackedOffsets.set(key, stackedOffset + 1);
      anchor.style.left = `${(buttonDef.x / sourceScene.width) * 100}%`;
      anchor.style.top = `${((buttonDef.y + stackedOffset * 34) / sourceScene.height) * 100}%`;
      if (buttonDef.width) {
        anchor.style.width = `${(buttonDef.width / sourceScene.width) * 100}%`;
      }
      if (buttonDef.height) {
        anchor.style.height = `${(buttonDef.height / sourceScene.height) * 100}%`;
      }
      anchor.addEventListener("pointerdown", (event) => {
        beginButtonPointer(buttonIndex, event);
      });
      buttonOverlays.append(anchor);
    });
  }

  function buttonPointerScale() {
    const rect = canvas.getBoundingClientRect();
    return {
      scaleX: rect.width > 0 ? sourceScene.width / rect.width : 1,
      scaleY: rect.height > 0 ? sourceScene.height / rect.height : 1,
    };
  }

  function beginButtonPointer(buttonIndex, event) {
    const button = buttonsState.val[buttonIndex];
    if (!button) {
      return;
    }
    const { scaleX, scaleY } = buttonPointerScale();
    buttonPointerState = {
      buttonIndex,
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      originX: button.x,
      originY: button.y,
      scaleX,
      scaleY,
      dragged: false,
    };
    window.addEventListener("pointermove", handleButtonPointerMove);
    window.addEventListener("pointerup", handleButtonPointerUp);
    window.addEventListener("pointercancel", handleButtonPointerUp);
    event.preventDefault();
  }

  function handleButtonPointerMove(event) {
    if (!buttonPointerState || event.pointerId !== buttonPointerState.pointerId) {
      return;
    }
    const dx = (event.clientX - buttonPointerState.startClientX) * buttonPointerState.scaleX;
    const dy = (event.clientY - buttonPointerState.startClientY) * buttonPointerState.scaleY;
    if (!buttonPointerState.dragged && Math.hypot(dx, dy) >= 4) {
      buttonPointerState.dragged = true;
    }
    if (!buttonPointerState.dragged) {
      return;
    }
    updateButtons((buttons) => {
      const button = buttons[buttonPointerState.buttonIndex];
      if (!button) {
        return;
      }
      button.x = buttonPointerState.originX + dx;
      button.y = buttonPointerState.originY + dy;
    });
  }

  function clearButtonPointer() {
    window.removeEventListener("pointermove", handleButtonPointerMove);
    window.removeEventListener("pointerup", handleButtonPointerUp);
    window.removeEventListener("pointercancel", handleButtonPointerUp);
    buttonPointerState = null;
  }

  function handleButtonPointerUp(event) {
    if (!buttonPointerState || event.pointerId !== buttonPointerState.pointerId) {
      return;
    }
    const { buttonIndex, dragged } = buttonPointerState;
    clearButtonPointer();
    if (!dragged) {
      runButtonAction(buttonIndex);
    }
  }

  function setTargetsVisibility(action, visible) {
    updateScene((scene) => {
      (action.pointIndices || []).forEach((index) => {
        if (scene.points[index]) scene.points[index].visible = visible;
      });
      (action.lineIndices || []).forEach((index) => {
        if (scene.lines[index]) scene.lines[index].visible = visible;
      });
      (action.circleIndices || []).forEach((index) => {
        if (scene.circles[index]) scene.circles[index].visible = visible;
      });
      (action.polygonIndices || []).forEach((index) => {
        if (scene.polygons[index]) scene.polygons[index].visible = visible;
      });
    });
  }

  function toggleTargetsVisibility(action) {
    const scene = currentScene();
    const hiddenPoint = (action.pointIndices || []).some((index) => scene.points[index]?.visible === false);
    const hiddenLine = (action.lineIndices || []).some((index) => scene.lines[index]?.visible === false);
    const hiddenCircle = (action.circleIndices || []).some((index) => scene.circles[index]?.visible === false);
    const hiddenPolygon = (action.polygonIndices || []).some((index) => scene.polygons[index]?.visible === false);
    setTargetsVisibility(action, hiddenPoint || hiddenLine || hiddenCircle || hiddenPolygon);
  }

  function stopButtonAnimation(buttonIndex) {
    const handle = buttonAnimations.get(buttonIndex);
    if (handle?.rafId) {
      window.cancelAnimationFrame(handle.rafId);
    }
    if (handle) {
      handle.stop = true;
    }
    buttonAnimations.delete(buttonIndex);
    updateButtons((buttons) => {
      if (buttons[buttonIndex]) {
        buttons[buttonIndex].active = false;
      }
    });
  }

  function toggleAnimatedPoint(buttonIndex, pointIndex, mode, targetPointIndex = null) {
    if (buttonsState.val[buttonIndex]?.active) {
      stopButtonAnimation(buttonIndex);
      return;
    }
    const scene = currentScene();
    const point = scene.points[pointIndex];
    if (!point) {
      return;
    }
    const base = { x: point.x, y: point.y };
    let initialDirection = 1;
    if (point.constraint?.kind === "segment") {
      if (targetPointIndex === point.constraint.startIndex) {
        initialDirection = -1;
      } else if (targetPointIndex === point.constraint.endIndex) {
        initialDirection = 1;
      } else {
        initialDirection = point.constraint.t < 0.5 ? 1 : -1;
      }
    }
    const state = {
      stop: false,
      direction: initialDirection,
      t: 0,
      vx: (Math.random() - 0.5) * 0.003,
      vy: (Math.random() - 0.5) * 0.003,
      nextTurnAt: 500 + Math.random() * 700,
      elapsedMs: 0,
      rafId: 0,
    };
    buttonAnimations.set(buttonIndex, state);
    updateButtons((buttons) => {
      if (buttons[buttonIndex]) {
        buttons[buttonIndex].active = true;
      }
    });
    let lastTime = null;
    const step = (timestamp) => {
      if (state.stop) {
        return;
      }
      if (lastTime === null) {
        lastTime = timestamp;
      }
      const dt = Math.min(64, timestamp - lastTime);
      lastTime = timestamp;
      updateScene((draft) => {
        const draftPoint = draft.points[pointIndex];
        if (!draftPoint) {
          return;
        }
        if (draftPoint.constraint?.kind === "segment") {
          const durationMs = mode === "scroll" ? 16000 : 12000;
          const delta = dt / durationMs;
          if (mode === "scroll") {
            draftPoint.constraint.t = (draftPoint.constraint.t + delta) % 1;
          } else {
            let next = draftPoint.constraint.t + delta * state.direction;
            if (next >= 1) {
              next = 1;
              state.direction = -1;
            } else if (next <= 0) {
              next = 0;
              state.direction = 1;
            }
            draftPoint.constraint.t = next;
          }
        } else if (mode === "scroll") {
          state.t += dt * 0.004;
          draftPoint.x = base.x + Math.sin(state.t) * 36;
        } else {
          state.elapsedMs += dt;
          if (state.elapsedMs >= state.nextTurnAt) {
            state.elapsedMs = 0;
            state.nextTurnAt = 500 + Math.random() * 700;
            state.vx += (Math.random() - 0.5) * 0.0016;
            state.vy += (Math.random() - 0.5) * 0.0016;
          }
          state.vx += (base.x - draftPoint.x) * 0.00008;
          state.vy += (base.y - draftPoint.y) * 0.00008;
          const speed = Math.hypot(state.vx, state.vy);
          if (speed > 0.005) {
            state.vx = (state.vx / speed) * 0.005;
            state.vy = (state.vy / speed) * 0.005;
          } else if (speed < 0.0008) {
            const angle = Math.random() * Math.PI * 2;
            state.vx = Math.cos(angle) * 0.0015;
            state.vy = Math.sin(angle) * 0.0015;
          }

          draftPoint.x += state.vx * dt;
          draftPoint.y += state.vy * dt;

          const maxDx = 0.8;
          const maxDy = 0.6;
          if (draftPoint.x < base.x - maxDx || draftPoint.x > base.x + maxDx) {
            state.vx *= -0.7;
            draftPoint.x = Math.max(base.x - maxDx, Math.min(base.x + maxDx, draftPoint.x));
          }
          if (draftPoint.y < base.y - maxDy || draftPoint.y > base.y + maxDy) {
            state.vy *= -0.7;
            draftPoint.y = Math.max(base.y - maxDy, Math.min(base.y + maxDy, draftPoint.y));
          }
        }
      });
      state.rafId = window.requestAnimationFrame(step);
    };
    state.rafId = window.requestAnimationFrame(step);
  }

  function runButtonAction(buttonIndex) {
    const button = buttonsState.val[buttonIndex];
    if (!button) {
      return;
    }
    const action = button.action || {};
    switch (action.kind) {
      case "link":
        if (action.href) {
          window.open(action.href, "_blank", "noopener,noreferrer");
        }
        break;
      case "toggle-visibility":
        toggleTargetsVisibility(action);
        break;
      case "set-visibility":
        setTargetsVisibility(action, !!action.visible);
        break;
      case "move-point":
        if (typeof action.pointIndex === "number") {
          toggleAnimatedPoint(
            buttonIndex,
            action.pointIndex,
            "move",
            action.targetPointIndex ?? null,
          );
        }
        break;
      case "animate-point":
        if (typeof action.pointIndex === "number") {
          toggleAnimatedPoint(buttonIndex, action.pointIndex, "animate");
        }
        break;
      case "scroll-point":
        if (typeof action.pointIndex === "number") {
          toggleAnimatedPoint(buttonIndex, action.pointIndex, "scroll");
        }
        break;
      case "sequence": {
        const intervalMs = Math.max(0, action.intervalMs || 0);
        (action.buttonIndices || []).forEach((childButtonIndex, offset) => {
          const timer = window.setTimeout(() => {
            runButtonAction(childButtonIndex);
            buttonTimers.delete(timer);
          }, offset * intervalMs);
          buttonTimers.set(timer, true);
        });
        break;
      }
      default:
        break;
    }
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

  function findHitPolygon(screenX, screenY) {
    return renderModule.findHitPolygon ? renderModule.findHitPolygon(viewerEnv, screenX, screenY) : null;
  }

  function beginDrag(pointerId, position, pointIndex, labelIndex, polygonIndex) {
    dragModule.beginDrag(viewerEnv, pointerId, position, pointIndex, labelIndex, polygonIndex);
  }

  function updateDraggedPoint(world) {
    dragModule.updateDraggedPoint(viewerEnv, world);
  }

  function updateDraggedLabel(world) {
    dragModule.updateDraggedLabel(viewerEnv, world);
  }

  function updateDraggedPolygon(world) {
    dragModule.updateDraggedPolygon(viewerEnv, world);
  }

  function panFromPointerDelta(position) {
    dragModule.panFromPointerDelta(viewerEnv, position);
    dynamicsModule.syncDynamicScene(viewerEnv);
  }

  function draw() {
    renderModule.draw(viewerEnv);
  }

  /** @type {ViewerEnv} */
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
    currentDynamics,
    resolveScenePoint: (index) => sceneModule.resolveScenePoint(viewerEnv, index),
    resolvePoint: (handle) => sceneModule.resolvePoint(viewerEnv, handle),
    resolveAnchorBase: (handle) => sceneModule.resolveAnchorBase(viewerEnv, handle),
    resolveLinePoints: (lineOrIndex) => sceneModule.resolveLinePoints(viewerEnv, lineOrIndex),
    toScreen: (point) => sceneModule.toScreen(viewerEnv, point),
    toWorld: (x, y) => sceneModule.toWorld(viewerEnv, x, y),
    getViewBounds: () => sceneModule.getViewBounds(viewerEnv),
    rgba,
    updateScene,
    updateDynamics,
    syncDynamicScene: () => dynamicsModule.syncDynamicScene(viewerEnv),
    isOriginPointIndex,
    formatNumber,
    formatAxisNumber,
    formatPiLabel,
    inputTag: input,
    labelTag: label,
    parameterControls,
    van,
    drawGrid: () => sceneModule.drawGrid(viewerEnv),
  };

  van.derive(() => {
    draw();
    return 0;
  });

  van.derive(() => {
    renderButtons();
    return 0;
  });

  canvas.addEventListener("pointerdown", (event) => {
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const pointIndex = findHitPoint(position.x, position.y);
    const labelIndex = pointIndex === null ? findHitLabel(position.x, position.y) : null;
    const polygonIndex = pointIndex === null && labelIndex === null
      ? findHitPolygon(position.x, position.y)
      : null;
    beginDrag(event.pointerId, position, pointIndex, labelIndex, polygonIndex);
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
    } else if (dragState.val.mode === "polygon") {
      updateDraggedPolygon(sceneModule.toWorld(viewerEnv, position.x, position.y));
    } else if (dragState.val.mode === "label") {
      updateDraggedLabel(position);
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
    view.zoom = Math.max(minZoom, Math.min(64, view.zoom * factor));
    const after = sceneModule.toWorld(viewerEnv, position.x, position.y);
    view.centerX += before.x - after.x;
    view.centerY += before.y - after.y;
    dynamicsModule.syncDynamicScene(viewerEnv);
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

  dynamicsModule.syncDynamicScene(viewerEnv);
  dynamicsModule.buildParameterControls(viewerEnv);
  resetView();
})();
