// @ts-check

(() => {
  const van = window.van;
  const { label, input } = van.tags;
  const {
    scene: sceneModule,
    render: renderModule,
    overlay: overlayModule,
    drag: dragModule,
    dynamics: dynamicsModule,
  } = window.GspViewerModules;
  const SVG_NS = "http://www.w3.org/2000/svg";
  const XLINK_NS = "http://www.w3.org/1999/xlink";
  /** @type {SceneData} */
  const sourceScene = JSON.parse(document.getElementById("scene-data").textContent);
  /** @type {SVGSVGElement} */
  const canvas = /** @type {SVGSVGElement} */ (/** @type {unknown} */ (document.getElementById("view")));
  /** @type {SVGGElement} */
  const gridLayer = /** @type {SVGGElement} */ (/** @type {unknown} */ (document.getElementById("grid-layer")));
  /** @type {SVGGElement} */
  const sceneLayer = /** @type {SVGGElement} */ (/** @type {unknown} */ (document.getElementById("scene-layer")));
  /** @type {SVGTextElement} */
  const measureTextNode = /** @type {SVGTextElement} */ (/** @type {unknown} */ (document.getElementById("measure-text")));
  /** @type {HTMLButtonElement} */
  const resetButton = /** @type {HTMLButtonElement} */ (document.getElementById("reset-view"));
  /** @type {HTMLButtonElement} */
  const debugToggleButton = /** @type {HTMLButtonElement} */ (document.getElementById("toggle-debug"));
  /** @type {HTMLElement} */
  const parameterControls = /** @type {HTMLElement} */ (document.getElementById("parameter-controls"));
  /** @type {HTMLElement} */
  const buttonOverlays = /** @type {HTMLElement} */ (document.getElementById("button-overlays"));
  /** @type {HTMLElement} */
  const debugPanel = /** @type {HTMLElement} */ (document.getElementById("debug-panel"));
  /** @type {HTMLElement} */
  const debugOutput = /** @type {HTMLElement} */ (document.getElementById("debug-output"));
  /** @type {HTMLButtonElement} */
  const debugDumpConsoleButton = /** @type {HTMLButtonElement} */ (document.getElementById("debug-dump-console"));
  /** @type {HTMLButtonElement[]} */
  const debugTabButtons = /** @type {HTMLButtonElement[]} */ (Array.from(
    document.querySelectorAll("[data-debug-tab]"),
  ));
  /** @type {HTMLElement} */
  const coordReadout = /** @type {HTMLElement} */ (document.getElementById("coord-readout"));
  /** @type {HTMLElement} */
  const zoomReadout = /** @type {HTMLElement} */ (document.getElementById("zoom-readout"));
  /** @typedef {{ category: string; index: number; hotspotIndex?: number | null; label?: string | null }} DebugTarget */
  const margin = 32;
  const trigMode = !!sourceScene.piMode;
  const savedViewportMode = !!sourceScene.savedViewport;
  const baseBounds = sourceScene.bounds;
  const baseCenterX = (baseBounds.minX + baseBounds.maxX) / 2;
  const baseCenterY = (baseBounds.minY + baseBounds.maxY) / 2;
  const rawBaseSpanX = Math.max(1e-6, baseBounds.maxX - baseBounds.minX);
  const rawBaseSpanY = Math.max(1e-6, baseBounds.maxY - baseBounds.minY);
  const usableWidth = Math.max(1, sourceScene.width - margin * 2);
  const usableHeight = Math.max(1, sourceScene.height - margin * 2);
  const canvasAspect = usableWidth / usableHeight;
  const boundsAspect = rawBaseSpanX / rawBaseSpanY;
  const baseSpanX = boundsAspect < canvasAspect
    ? rawBaseSpanY * canvasAspect
    : rawBaseSpanX;
  const baseSpanY = boundsAspect > canvasAspect
    ? rawBaseSpanX / canvasAspect
    : rawBaseSpanY;
  const minZoom = 0.05;
  const pointHitRadius = 10;
  const pointMatchTolerance = 1e-3;
  const autoOpenDebug = new URLSearchParams(window.location.search).get("debug") === "1";
  const defaultZoom = sourceScene.graphMode ? 1 : 0.9;
  const pointerWorldState = van.state(null);
  /** @type {{ val: "selection" | "scene" | "json" }} */
  const debugViewState = van?.state ? van.state("selection") : { val: "selection" };
  /** @type {{ val: DebugTarget | null }} */
  const selectedDebugTargetState = van?.state ? van.state(null) : { val: null };
  /** @type {Map<string, { element: Element, target: DebugTarget }>} */
  const debugElementRegistry = new Map();
  let nextDebugElementId = 1;
  const viewState = van?.state ? van.state({
    centerX: baseCenterX,
    centerY: baseCenterY,
    zoom: defaultZoom,
  }) : { val: {
    centerX: baseCenterX,
    centerY: baseCenterY,
    zoom: defaultZoom,
  } };
  /** @type {ViewState} */
  const view = new Proxy(/** @type {ViewState} */ ({}), {
    get: (_, key) => viewState.val[/** @type {keyof ViewState} */ (key)],
    set: (_, key, value) => {
      viewState.val = {
        ...viewState.val,
        [/** @type {keyof ViewState} */ (key)]: /** @type {ViewState[keyof ViewState]} */ (value),
      };
      return true;
    },
  });
  /** @type {{ val: DragState }} */
  const dragState = van?.state ? van.state(null) : { val: null };
  /** @type {{ val: number | null }} */
  const hoverPointIndex = van?.state ? van.state(null) : { val: null };
  const labelAttachDistance = 40;
  /** @type {ViewerOverlayRuntime} */
  let overlayRuntime = {
    currentButtons() {
      return [];
    },
    currentHotspotFlashes() {
      return [];
    },
    render() {},
  };
  const coordText = van.derive(() => {
    const world = pointerWorldState.val;
    return world ? `x ${formatNumber(world.x)}, y ${formatNumber(world.y)}` : "x -, y -";
  });
  const zoomText = van.derive(() => `zoom ${Math.round(viewState.val.zoom * 100)}%`);

  coordReadout.replaceChildren();
  zoomReadout.replaceChildren();
  van.add(coordReadout, coordText);
  van.add(zoomReadout, zoomText);

  /**
   * @param {string} name
   * @param {Record<string, string | number | boolean | null | undefined>} [attrs]
   */
  function createSvgElement(name, attrs = {}) {
    const element = document.createElementNS(SVG_NS, name);
    setSvgAttributes(element, attrs);
    return element;
  }

  /**
   * @param {Element} element
   * @param {Record<string, string | number | boolean | null | undefined>} attrs
   */
  function setSvgAttributes(element, attrs) {
    Object.entries(attrs).forEach(([key, value]) => {
      if (value === null || value === undefined || value === false) {
        element.removeAttribute(key);
        return;
      }
      if (key === "href") {
        element.setAttributeNS(XLINK_NS, "href", String(value));
        element.setAttribute("href", String(value));
        return;
      }
      if (value === true) {
        element.setAttribute(key, "");
        return;
      }
      element.setAttribute(key, String(value));
    });
  }

  /** @param {Element} element */
  function clearSvgChildren(element) {
    element.replaceChildren();
  }

  /**
   * @param {string} text
   * @param {number} [fontSize]
   * @param {number | string} [fontWeight]
   */
  function measureText(text, fontSize = 18, fontWeight = 400) {
    const normalized = text || "";
    measureTextNode.setAttribute("font-size", String(fontSize));
    measureTextNode.setAttribute("font-weight", String(fontWeight));
    measureTextNode.setAttribute("font-family", "\"Noto Sans\", \"Segoe UI\", sans-serif");
    measureTextNode.textContent = normalized || " ";
    const width = measureTextNode.getBBox().width;
    measureTextNode.textContent = "";
    return normalized ? width : 0;
  }

  /**
   * @param {Point} left
   * @param {Point} right
   */
  function samePoint(left, right) {
    return Math.abs(left.x - right.x) < pointMatchTolerance
      && Math.abs(left.y - right.y) < pointMatchTolerance;
  }

  /**
   * @param {number} index
   * @returns {Point}
   */
  function resolveSourcePoint(index) {
    const point = sourceScene.points[index];
    if (!point) {
      return { x: 0, y: 0 };
    }
    const resolved = sceneModule.resolveConstrainedPoint(
      { sourceScene },
      point.constraint,
      resolveSourcePoint,
    );
    if (resolved) {
      return resolved;
    }
    return { x: point.x, y: point.y };
  }

  /**
   * @param {Point} point
   * @returns {PointHandle}
   */
  function attachPointRef(point) {
    const pointIndex = sourceScene.points.findIndex((candidate, index) => samePoint(resolveSourcePoint(index), point));
    if (pointIndex >= 0) {
      return { pointIndex };
    }
    return { x: point.x, y: point.y };
  }

  /**
   * @param {PointHandle} handle
   * @returns {Point}
   */
  function resolveSourceHandle(handle) {
    if (hasPointIndexHandle(handle)) {
      return resolveSourcePoint(handle.pointIndex);
    }
    return /** @type {Point} */ (handle);
  }

  /**
   * @param {Point} left
   * @param {Point} right
   */
  function distanceSquared(left, right) {
    const dx = left.x - right.x;
    const dy = left.y - right.y;
    return dx * dx + dy * dy;
  }

  /**
   * @param {Point} point
   * @param {Point} start
   * @param {Point} end
   */
  function distanceToSegmentSquared(point, start, end) {
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lengthSquared = dx * dx + dy * dy;
    if (lengthSquared <= 1e-9) {
      return distanceSquared(point, start);
    }
    const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared));
    return distanceSquared(point, {
      x: start.x + dx * t,
      y: start.y + dy * t,
    });
  }

  /**
   * @param {Point} point
   * @param {Point[]} polyline
   */
  function distanceToPolylineSquared(point, polyline) {
    let best = Number.POSITIVE_INFINITY;
    for (let index = 0; index + 1 < polyline.length; index += 1) {
      best = Math.min(best, distanceToSegmentSquared(point, polyline[index], polyline[index + 1]));
    }
    return best;
  }

  /**
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   */
  function arcGeometryFromPoints(start, mid, end) {
    const determinant = 2 * (
      start.x * (mid.y - end.y)
      + mid.x * (end.y - start.y)
      + end.x * (start.y - mid.y)
    );
    if (Math.abs(determinant) <= 1e-9) return null;

    const startSq = start.x * start.x + start.y * start.y;
    const midSq = mid.x * mid.x + mid.y * mid.y;
    const endSq = end.x * end.x + end.y * end.y;
    const center = {
      x: (
        startSq * (mid.y - end.y)
        + midSq * (end.y - start.y)
        + endSq * (start.y - mid.y)
      ) / determinant,
      y: (
        startSq * (end.x - mid.x)
        + midSq * (start.x - end.x)
        + endSq * (mid.x - start.x)
      ) / determinant,
    };
    const radius = Math.hypot(start.x - center.x, start.y - center.y);
    if (radius <= 1e-9) return null;

    const startAngle = Math.atan2(start.y - center.y, start.x - center.x);
    const midAngle = Math.atan2(mid.y - center.y, mid.x - center.x);
    const endAngle = Math.atan2(end.y - center.y, end.x - center.x);
    const forwardSpan = ((endAngle - startAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2);
    const forwardMid = ((midAngle - startAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2);

    return {
      center,
      radius,
      startAngle,
      endAngle,
      counterClockwise: forwardMid > forwardSpan + 1e-9,
    };
  }

  /**
   * @param {Point} start
   * @param {Point} end
   * @param {Point} center
   * @param {boolean} counterclockwise
   * @param {boolean} yUp
   */
  function midpointOnCircleWorld(start, end, center, counterclockwise, yUp) {
    const ySign = yUp ? 1 : -1;
    const startAngle = Math.atan2((start.y - center.y) * ySign, start.x - center.x);
    const endAngle = Math.atan2((end.y - center.y) * ySign, end.x - center.x);
    const radius = (Math.hypot(start.x - center.x, start.y - center.y) + Math.hypot(end.x - center.x, end.y - center.y)) / 2;
    if (radius <= 1e-9) return null;
    const span = counterclockwise
      ? ((endAngle - startAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2)
      : -(((startAngle - endAngle) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2));
    const midpointAngle = startAngle + span * 0.5;
    return {
      x: center.x + radius * Math.cos(midpointAngle),
      y: center.y + ySign * radius * Math.sin(midpointAngle),
    };
  }

  /** @param {any} debug */
  function clonePayloadDebug(debug) {
    return debug ? {
      ...debug,
      recordTypes: [...(debug.recordTypes || [])],
      recordNames: [...(debug.recordNames || [])],
    } : null;
  }

  /**
   * @param {Point} point
   * @param {Array<{ points: PointHandle[] }>} hydratedLines
   * @returns {PointHandle}
   */
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
    hydratedLines.forEach((/** @type {{ points: PointHandle[] }} */ line, /** @type {number} */ lineIndex) => {
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

  /**
   * @param {{ binding?: { kind?: string; pointIndex?: number } | null; anchor: Point }} label
   * @param {Array<{ points: PointHandle[] }>} hydratedLines
   */
  function attachPointCenteredLabelAnchor(label, hydratedLines) {
    if (typeof label.binding?.pointIndex === "number") {
      return { pointIndex: label.binding.pointIndex };
    }
    return attachLabelAnchor(label.anchor, hydratedLines);
  }

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { pointIndex: number }>}
   */
  function hasPointIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  /**
   * @param {SceneData} scene
   * @returns {ViewerSceneData}
   */
  function hydrateScene(scene) {
    const hydratedLines = scene.lines.map((line) => ({
      color: line.color,
      dashed: line.dashed,
      visible: line.visible !== false,
      points: line.points.map(attachPointRef),
      binding: line.binding ? { ...line.binding } : null,
      debug: clonePayloadDebug(line.debug),
    }));
    return {
      ...scene,
      graphMode: scene.graphMode,
      bounds: scene.bounds ? { ...scene.bounds } : null,
      images: (scene.images || []).map((image) => ({
        topLeft: { ...image.topLeft },
        bottomRight: { ...image.bottomRight },
        src: image.src,
        screenSpace: !!image.screenSpace,
        debug: clonePayloadDebug(image.debug),
      })),
      points: scene.points.map((point) => ({
        x: point.x,
        y: point.y,
        color: point.color,
        visible: point.visible !== false,
        draggable: point.draggable !== false,
        constraint: point.constraint
          ? {
              ...point.constraint,
              ...(point.constraint.kind === "polyline"
                ? { points: point.constraint.points.map(attachPointRef) }
                : null),
            }
          : null,
        binding: point.binding ? { ...point.binding } : null,
        debug: clonePayloadDebug(point.debug),
      })),
      origin: scene.origin ? attachPointRef(scene.origin) : null,
      lines: hydratedLines,
      polygons: scene.polygons.map((polygon) => ({
        color: polygon.color,
        outlineColor: polygon.outlineColor,
        visible: polygon.visible !== false,
        points: polygon.points.map(attachPointRef),
        binding: polygon.binding ? { ...polygon.binding } : null,
        debug: clonePayloadDebug(polygon.debug),
      })),
      circles: scene.circles.map((circle) => ({
        color: circle.color,
        fillColor: circle.fillColor || null,
        fillColorBinding: circle.fillColorBinding ? { ...circle.fillColorBinding } : null,
        dashed: !!circle.dashed,
        visible: circle.visible !== false,
        center: attachPointRef(circle.center),
        radiusPoint: attachPointRef(circle.radiusPoint),
        binding: circle.binding ? { ...circle.binding } : null,
        debug: clonePayloadDebug(circle.debug),
      })),
      arcs: (scene.arcs || []).map((arc) => ({
        color: arc.color,
        visible: arc.visible !== false,
        points: arc.points.map(attachPointRef),
        center: arc.center ? attachPointRef(arc.center) : null,
        counterclockwise: !!arc.counterclockwise,
        debug: clonePayloadDebug(arc.debug),
      })),
      labels: scene.labels.map((label) => ({
        text: label.text,
        richMarkup: label.richMarkup || null,
        color: label.color,
        visible: label.visible !== false,
        anchor: label.screenSpace
          ? { ...label.anchor }
          : label.binding?.kind === "point-expression-value"
            ? attachPointCenteredLabelAnchor(label, hydratedLines)
            : attachLabelAnchor(label.anchor, hydratedLines),
        binding: label.binding ? { ...label.binding } : null,
        screenSpace: !!label.screenSpace,
        centeredOnAnchor: label.binding?.kind === "point-expression-value",
        hotspots: (label.hotspots || []).map((hotspot) => ({
          ...hotspot,
          action: hotspot.action ? { ...hotspot.action } : null,
        })),
        debug: clonePayloadDebug(label.debug),
      })),
      iterationTables: (scene.iterationTables || []).map((table) => ({
        ...table,
        debug: clonePayloadDebug(table.debug),
        /** @type {RuntimeIterationRow[]} */
        rows: [],
      })),
      buttons: (scene.buttons || []).map((button) => ({
        ...button,
        debug: clonePayloadDebug(button.debug),
        baseText: button.text,
        visible: true,
        active: false,
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
  /** @type {Set<string>} */
  const pendingDependencyRootIds = new Set();
  /** @type {any} */
  let lastDependencyRun = null;

  /**
   * @param {string | string[]} rootIds
   */
  function markDependencyRootsDirty(rootIds) {
    const values = Array.isArray(rootIds) ? rootIds : [rootIds];
    values.forEach((rootId) => {
      if (typeof rootId === "string" && rootId.length > 0) {
        pendingDependencyRootIds.add(rootId);
      }
    });
  }

  /**
   * @param {(draft: ViewerSceneData) => void} mutator
   * @param {"graph" | "none"} [mode]
   */
  function updateScene(mutator, mode = "none") {
    const next = sceneState.val;
    mutator(next);
    if (mode === "graph" && dynamicsModule.runDependencyGraph) {
      lastDependencyRun = dynamicsModule.runDependencyGraph(
        viewerEnv,
        next,
        Array.from(pendingDependencyRootIds),
      );
      pendingDependencyRootIds.clear();
    } else {
      lastDependencyRun = null;
    }
    sceneState.val = { ...next };
  }

  /** @param {(draft: RuntimeDynamicsState) => void} mutator */
  function updateDynamics(mutator) {
    const next = dynamicsState.val;
    mutator(next);
    dynamicsState.val = { ...next };
  }

  /** @param {[number, number, number, number]} color */
  function rgba(color) {
    return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${(color[3] / 255).toFixed(3)})`;
  }

  /** @param {number} value */
  function formatNumber(value) {
    return Number.isFinite(value) ? value.toFixed(2) : "-";
  }

  /** @param {number} value */
  function formatAxisNumber(value) {
    if (Math.abs(value - Math.round(value)) < 1e-6) {
      return String(Math.round(value));
    }
    return value.toFixed(1);
  }

  /** @param {number} stepIndex */
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

  /** @param {unknown} value */
  function cloneForDebug(value) {
    if (typeof structuredClone === "function") {
      return structuredClone(value);
    }
    return JSON.parse(JSON.stringify(value));
  }

  /**
   * Replace embedded payload-time parameter defaults with the current runtime
   * parameter values for debug/inspection output.
   *
   * @param {unknown} value
   * @param {Map<string, number>} parameters
   * @returns {unknown}
   */
  function cloneWithLiveParameterValues(value, parameters) {
    if (!value || typeof value !== "object") {
      return value;
    }
    if (Array.isArray(value)) {
      return value.map((item) => cloneWithLiveParameterValues(item, parameters));
    }
    const cloned = Object.fromEntries(
      Object.entries(/** @type {Record<string, unknown>} */ (value)).map(([key, child]) => [
        key,
        cloneWithLiveParameterValues(child, parameters),
      ]),
    );
    if (
      cloned.kind === "parameter"
      && typeof cloned.name === "string"
      && "value" in cloned
      && Number.isFinite(parameters.get(cloned.name))
    ) {
      cloned.value = parameters.get(cloned.name);
    }
    if (
      typeof cloned.depth === "number"
      && typeof cloned.parameterName === "string"
      && Number.isFinite(parameters.get(cloned.parameterName))
    ) {
      cloned.depth = Math.max(0, Math.floor(parameters.get(cloned.parameterName) + 1e-9));
    }
    if (
      typeof cloned.depth === "number"
      && typeof cloned.depthParameterName === "string"
      && Number.isFinite(parameters.get(cloned.depthParameterName))
    ) {
      cloned.depth = Math.max(0, Math.floor(parameters.get(cloned.depthParameterName) + 1e-9));
    }
    return cloned;
  }

  /**
   * @param {unknown} entity
   * @returns {unknown}
   */
  function debugEntityWithLiveParameters(entity) {
    const parameters = dynamicsModule.parameterMapForScene
      ? dynamicsModule.parameterMapForScene(viewerEnv, currentScene())
      : new Map();
    return cloneWithLiveParameterValues(entity, parameters);
  }

  function pruneDebugRegistry() {
    for (const [id, entry] of debugElementRegistry.entries()) {
      if (!entry.element.isConnected) {
        debugElementRegistry.delete(id);
      }
    }
  }

  /**
   * @param {DebugTarget | null} target
   * @returns {string}
   */
  function debugTargetKey(target) {
    if (!target) return "";
    return `${target.category}:${target.index}:${target.hotspotIndex ?? ""}`;
  }

  function syncDebugSelectionHighlight() {
    pruneDebugRegistry();
    const selectedKey = debugTargetKey(selectedDebugTargetState.val);
    for (const entry of debugElementRegistry.values()) {
      entry.element.setAttribute(
        "data-gsp-debug-selected",
        debugTargetKey(entry.target) === selectedKey ? "true" : "false",
      );
    }
  }

  /**
   * @param {Element} element
   * @param {DebugTarget | null | undefined} target
   */
  function registerDebugElement(element, target) {
    if (!element || !target) {
      return;
    }
    const debugId = `dbg-${nextDebugElementId++}`;
    element.setAttribute("data-gsp-debug-id", debugId);
    element.setAttribute("data-gsp-kind", target.category);
    element.setAttribute("data-gsp-index", String(target.index));
    if (target.hotspotIndex !== undefined && target.hotspotIndex !== null) {
      element.setAttribute("data-gsp-hotspot-index", String(target.hotspotIndex));
    }
    const entity = lookupDebugEntity(target);
    if (entity?.debug?.groupOrdinal) {
      element.setAttribute("data-gsp-group", String(entity.debug.groupOrdinal));
    }
    debugElementRegistry.set(debugId, { element, target });
    syncDebugSelectionHighlight();
  }

  /**
   * @param {DebugTarget} target
   */
  function selectDebugTarget(target) {
    selectedDebugTargetState.val = target;
    debugViewState.val = "selection";
    syncDebugSelectionHighlight();
    renderDebugOutput();
  }

  /** @param {Element | null} element */
  function selectDebugTargetFromElement(element) {
    const carrier = element?.closest?.("[data-gsp-debug-id]");
    if (!carrier) {
      return false;
    }
    const debugId = carrier.getAttribute("data-gsp-debug-id");
    const entry = debugId ? debugElementRegistry.get(debugId) : null;
    if (!entry) {
      return false;
    }
    selectDebugTarget(entry.target);
    return true;
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   * @returns {DebugTarget | null}
   */
  function findDebugTargetAtScreen(screenX, screenY) {
    const pointIndex = findHitPoint(screenX, screenY);
    if (pointIndex !== null) {
      return { category: "points", index: pointIndex };
    }
    const imageIndex = renderModule.findHitImage ? renderModule.findHitImage(viewerEnv, screenX, screenY) : null;
    if (imageIndex !== null) {
      return { category: "images", index: imageIndex };
    }
    const iterationTableIndex = findHitIterationTable(screenX, screenY);
    if (iterationTableIndex !== null) {
      return { category: "iterationTables", index: iterationTableIndex };
    }
    const labelIndex = findHitLabel(screenX, screenY);
    if (labelIndex !== null) {
      return { category: "labels", index: labelIndex };
    }
    const lineIndex = findHitLine(screenX, screenY);
    if (lineIndex !== null) {
      return { category: "lines", index: lineIndex };
    }
    const polygonIndex = findHitPolygon(screenX, screenY);
    if (polygonIndex !== null) {
      return { category: "polygons", index: polygonIndex };
    }
    const circleIndex = findHitCircle(screenX, screenY);
    if (circleIndex !== null) {
      return { category: "circles", index: circleIndex };
    }
    const arcIndex = findHitArc(screenX, screenY);
    if (arcIndex !== null) {
      return { category: "arcs", index: arcIndex };
    }
    return null;
  }

  /** @param {DebugTarget} target */
  function lookupDebugEntity(target) {
    const scene = currentScene();
    switch (target.category) {
      case "images":
        return scene.images?.[target.index] ?? null;
      case "polygons":
        return scene.polygons?.[target.index] ?? null;
      case "lines":
        return scene.lines?.[target.index] ?? null;
      case "circles":
        return scene.circles?.[target.index] ?? null;
      case "arcs":
        return scene.arcs?.[target.index] ?? null;
      case "points":
        return scene.points?.[target.index] ?? null;
      case "labels":
        return scene.labels?.[target.index] ?? null;
      case "iterationTables":
        return scene.iterationTables?.[target.index] ?? null;
      case "buttons":
        return overlayRuntime.currentButtons?.()?.[target.index] ?? scene.buttons?.[target.index] ?? null;
      case "labelHotspots": {
        const label = scene.labels?.[target.index];
        if (!label || target.hotspotIndex === undefined || target.hotspotIndex === null) {
          return null;
        }
        return label.hotspots?.[target.hotspotIndex] ?? null;
      }
      default:
        return null;
    }
  }

  /** @param {DebugTarget} target */
  function describeDebugTarget(target) {
    const suffix = target.hotspotIndex !== undefined && target.hotspotIndex !== null
      ? `[${target.hotspotIndex}]`
      : "";
    return `${target.category}[${target.index}]${suffix}`;
  }

  /** @param {Record<string, unknown> | null | undefined} debug */
  function formatPayloadDebug(debug) {
    if (!debug) {
      return ["payload: derived or unavailable"];
    }
    const lines = [];
    if (typeof debug.groupOrdinal === "number") {
      lines.push(`payload group: #${debug.groupOrdinal}`);
    }
    if (typeof debug.groupKind === "string") {
      lines.push(`group kind: ${debug.groupKind}`);
    }
    if (Array.isArray(debug.recordNames) && debug.recordNames.length) {
      lines.push(`records: ${debug.recordNames.join(", ")}`);
    } else if (Array.isArray(debug.recordTypes) && debug.recordTypes.length) {
      lines.push(`records: ${debug.recordTypes.join(", ")}`);
    }
    return lines;
  }

  function buildSelectionDebugOutput() {
    const target = selectedDebugTargetState.val;
    if (!target) {
      return [
        "Selection",
        "  No element selected.",
        "  Open Debug and click a rendered element or overlay to inspect its payload origin.",
      ].join("\n");
    }
    const entity = debugEntityWithLiveParameters(lookupDebugEntity(target));
    const entityRecord = /** @type {Record<string, unknown> | null} */ (
      entity && typeof entity === "object" ? entity : null
    );
    const lines = [
      "Selection",
      `  target: ${describeDebugTarget(target)}`,
      `  summary: ${summarizeDebugEntity(entity) || "(no summary)"}`,
    ];
    formatPayloadDebug(entityRecord?.debug && typeof entityRecord.debug === "object"
      ? /** @type {Record<string, unknown>} */ (entityRecord.debug)
      : null).forEach((line) => {
      lines.push(`  ${line}`);
    });
    const refs = collectReferenceTokens(entity);
    if (refs.length) {
      lines.push(`  refs: ${refs.join(", ")}`);
    }
    lines.push("");
    lines.push(JSON.stringify(cloneForDebug(entity), null, 2));
    return lines.join("\n");
  }

  function buildDebugJson() {
    return JSON.stringify(buildRuntimeSnapshot(), null, 2);
  }

  /**
   * @param {string} key
   * @param {number} value
   */
  function formatReference(key, value) {
    if (!Number.isInteger(value)) {
      return null;
    }
    switch (key) {
      case "buttonIndices":
        return `buttons[${value}]`;
      case "circleIndices":
      case "circleIndex":
        return `circles[${value}]`;
      case "lineIndices":
      case "lineIndex":
        return `lines[${value}]`;
      case "polygonIndices":
      case "polygonIndex":
        return `polygons[${value}]`;
      case "seedLabelIndex":
      case "labelIndex":
        return `labels[${value}]`;
      case "functionKey":
        return `functions[${value}]`;
      case "segmentIndex":
        return null;
      default:
        if (
          key === "pointIndex"
          || key === "targetPointIndex"
          || key === "pointSeedIndex"
          || key === "seedIndex"
          || key === "sourceIndex"
          || key === "centerIndex"
          || key === "originIndex"
          || key === "radiusIndex"
          || key === "startIndex"
          || key === "endIndex"
          || key === "midIndex"
          || key === "throughIndex"
          || key === "vertexIndex"
          || key === "lineStartIndex"
          || key === "lineEndIndex"
        ) {
          return `points[${value}]`;
        }
        return null;
    }
  }

  /** @param {unknown} value */
  function collectReferenceTokens(value) {
    /** @type {string[]} */
    const refs = [];
    /** @param {unknown} node */
    function visit(node) {
      if (!node || typeof node !== "object") {
        return;
      }
      if (Array.isArray(node)) {
        node.forEach(visit);
        return;
      }
      Object.entries(node).forEach(([key, child]) => {
        if (typeof child === "number") {
          const ref = formatReference(key, child);
          if (ref) {
            refs.push(ref);
          }
          return;
        }
        if (Array.isArray(child)) {
          const directRefs = child
            .map((/** @type {unknown} */ item) => (typeof item === "number" ? formatReference(key, item) : null))
            .filter(Boolean);
          refs.push(...directRefs);
          child.forEach(visit);
          return;
        }
        visit(child);
      });
    }
    visit(value);
    return [...new Set(refs)];
  }

  /** @param {unknown} entity */
  function summarizeDebugEntity(entity) {
    const item = /** @type {Record<string, unknown> & { anchor?: Record<string, unknown> }} */ (entity ?? {});
    const parts = [];
    if (typeof item.text === "string") {
      parts.push(JSON.stringify(item.text));
    }
    if (typeof item.name === "string") {
      parts.push(`name=${item.name}`);
    }
    if (typeof item.kind === "string") {
      parts.push(`kind=${item.kind}`);
    }
    if (typeof item.visible === "boolean") {
      parts.push(item.visible ? "visible" : "hidden");
    }
    if (typeof item.depth === "number") {
      parts.push(`depth=${item.depth}`);
    }
    if (typeof item.edgeCount === "number") {
      parts.push(`edges=${item.edgeCount}`);
    }
    if (typeof item.parameterName === "string" && item.parameterName.length > 0) {
      parts.push(`param=${item.parameterName}`);
    }
    if (item.anchor && typeof item.anchor === "object") {
      if (typeof item.anchor.x === "number" && typeof item.anchor.y === "number") {
        parts.push(`anchor @ (${formatNumber(item.anchor.x)}, ${formatNumber(item.anchor.y)})`);
      }
      if (item.screenSpace === true) {
        parts.push("screenSpace");
      }
    }
    if (typeof item.x === "number" && typeof item.y === "number" && !item.kind) {
      parts.push(`@ (${formatNumber(item.x)}, ${formatNumber(item.y)})`);
    }
    return parts.join(" ");
  }

  /**
   * @param {string[]} lines
   * @param {string} title
   * @param {string} itemLabel
   * @param {unknown[]} items
   */
  function appendGraphSection(lines, title, itemLabel, items) {
    lines.push(`${title} (${items.length})`);
    items.forEach((/** @type {unknown} */ item, /** @type {number} */ index) => {
      const summary = summarizeDebugEntity(item);
      const refs = collectReferenceTokens(item);
      lines.push(`  ${itemLabel}[${index}]${summary ? ` ${summary}` : ""}`);
      if (refs.length > 0) {
        lines.push(`    -> ${refs.join(", ")}`);
      }
    });
  }

  /** @param {ViewerSceneData} scene */
  function collectDebugLineIterations(scene) {
    const iterations = Array.isArray(scene.lineIterations) ? [...scene.lineIterations] : [];
    if (iterations.some((/** @type {any} */ item) => item?.kind === "rotate" || item?.kind === "rotate-edge")) {
      return iterations;
    }
    /** @type {Map<string, { kind: string, centerIndex: number, vertexIndex: number, parameterName: string, edgeCount: number, visible: boolean }>} */
    const rotateFamilies = new Map();
    (scene.lines || []).forEach((/** @type {RuntimeLineJson} */ line) => {
      const binding = line?.binding;
      if (binding?.kind !== "rotate-edge") {
        return;
      }
      const key = `${binding.centerIndex}:${binding.vertexIndex}:${binding.parameterName}`;
      const current = rotateFamilies.get(key);
      if (current) {
        current.edgeCount += 1;
        current.visible = current.visible || line.visible !== false;
        return;
      }
      rotateFamilies.set(key, {
        kind: "rotate-edge-family",
        centerIndex: binding.centerIndex,
        vertexIndex: binding.vertexIndex,
        parameterName: binding.parameterName,
        edgeCount: 1,
        visible: line.visible !== false,
      });
    });
    return [...iterations, ...rotateFamilies.values()];
  }

  /** @param {ViewerSceneData} scene */
  function buildDebugGraph(scene) {
    const lines = [
      "Scene",
      `  size ${scene.width}x${scene.height}`,
      `  modes graph=${!!scene.graphMode} pi=${!!scene.piMode} savedViewport=${!!scene.savedViewport} yUp=${!!scene.yUp}`,
      `  bounds [${formatNumber(scene.bounds.minX)}, ${formatNumber(scene.bounds.minY)}] -> [${formatNumber(scene.bounds.maxX)}, ${formatNumber(scene.bounds.maxY)}]`,
    ];
    if (scene.origin) {
      lines.push(`  origin -> ${collectReferenceTokens({ origin: scene.origin }).join(", ") || "raw point"}`);
    }
    appendGraphSection(lines, "Points", "point", scene.points || []);
    appendGraphSection(lines, "Lines", "line", scene.lines || []);
    appendGraphSection(lines, "Polygons", "polygon", scene.polygons || []);
    appendGraphSection(lines, "Circles", "circle", scene.circles || []);
    appendGraphSection(lines, "Arcs", "arc", scene.arcs || []);
    appendGraphSection(lines, "Labels", "label", scene.labels || []);
    appendGraphSection(lines, "Point Iterations", "pointIteration", scene.pointIterations || []);
    appendGraphSection(lines, "Line Iterations", "lineIteration", collectDebugLineIterations(scene));
    appendGraphSection(lines, "Polygon Iterations", "polygonIteration", scene.polygonIterations || []);
    appendGraphSection(lines, "Label Iterations", "labelIteration", scene.labelIterations || []);
    appendGraphSection(lines, "Buttons", "button", scene.buttons || []);
    appendGraphSection(lines, "Parameters", "parameter", scene.parameters || []);
    appendGraphSection(lines, "Functions", "function", scene.functions || []);
    return lines.join("\n");
  }

  function buildRuntimeSnapshot() {
    return /** @type {{ view: ViewState; scene: ViewerSceneData; dynamics: RuntimeDynamicsState; buttons: RuntimeButtonJson[] }} */ (debugEntityWithLiveParameters({
      view: { ...viewState.val },
      scene: currentScene(),
      dynamics: currentDynamics(),
      buttons: overlayRuntime.currentButtons(),
    }));
  }

  function renderDebugOutput() {
    if (!debugOutput) {
      return;
    }
    const activeTab = debugViewState.val === "json"
      ? "json"
      : debugViewState.val === "scene"
        ? "scene"
        : "selection";
    debugOutput.textContent = activeTab === "json"
      ? buildDebugJson()
      : activeTab === "scene"
        ? buildDebugGraph(currentScene())
        : buildSelectionDebugOutput();
    debugTabButtons.forEach((button) => {
      const isActive = button.dataset.debugTab === activeTab;
      button.setAttribute("aria-pressed", isActive ? "true" : "false");
      button.classList.toggle("is-active", isActive);
    });
  }

  /** @param {boolean} open */
  function setDebugPanelOpen(open) {
    if (!debugPanel || !debugToggleButton) {
      return;
    }
    debugPanel.hidden = !open;
    debugToggleButton.setAttribute("aria-expanded", open ? "true" : "false");
    debugToggleButton.classList.toggle("is-active", open);
    if (open) {
      renderDebugOutput();
    }
  }

  function dumpDebugToConsole() {
    const selection = buildSelectionDebugOutput();
    const graph = buildDebugGraph(currentScene());
    const runtime = buildRuntimeSnapshot();
    console.groupCollapsed("gspDebug");
    console.log(selection);
    console.log(graph);
    console.log("sourceScene", cloneForDebug(sourceScene));
    console.log("runtime", runtime);
    console.groupEnd();
  }

  /**
   * @param {number | null} [screenX]
   * @param {number | null} [screenY]
   */
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
    view.zoom = defaultZoom;
    draw();
    overlayRuntime.render();
    updateReadout();
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitPoint(screenX, screenY) {
    return renderModule.findHitPoint(viewerEnv, screenX, screenY);
  }

  /** @param {number} index */
  function isOriginPointIndex(index) {
    const origin = currentScene().origin;
    return !!origin && "pointIndex" in origin && typeof origin.pointIndex === "number" && origin.pointIndex === index;
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitLabel(screenX, screenY) {
    return renderModule.findHitLabel(viewerEnv, screenX, screenY);
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitIterationTable(screenX, screenY) {
    return renderModule.findHitIterationTable(viewerEnv, screenX, screenY);
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitPolygon(screenX, screenY) {
    return renderModule.findHitPolygon ? renderModule.findHitPolygon(viewerEnv, screenX, screenY) : null;
  }

  /**
   * @param {RuntimeLineJson} line
   * @returns {Point[] | null}
   */
  function resolveLineScreenPoints(line) {
    if (!line || line.visible === false || line.binding?.kind === "graph-helper-line") {
      return null;
    }
    if (
      line.binding?.kind === "line"
      || line.binding?.kind === "ray"
      || line.binding?.kind === "angle-bisector-ray"
      || line.binding?.kind === "perpendicular-line"
      || line.binding?.kind === "parallel-line"
    ) {
      /** @param {LineBindingJson} binding */
      const resolveHostLinePoints = (binding) => {
        if (
          binding
          && typeof binding === "object"
          && "lineStartIndex" in binding
          && "lineEndIndex" in binding
          && typeof binding.lineStartIndex === "number"
          && typeof binding.lineEndIndex === "number"
        ) {
          return [
            viewerEnv.resolveScenePoint(binding.lineStartIndex),
            viewerEnv.resolveScenePoint(binding.lineEndIndex),
          ];
        }
        if (
          binding
          && typeof binding === "object"
          && "lineIndex" in binding
          && typeof binding.lineIndex === "number"
        ) {
          return viewerEnv.resolveLinePoints(binding.lineIndex);
        }
        return null;
      };
      const start = line.binding.kind === "perpendicular-line" || line.binding.kind === "parallel-line"
        ? (() => {
            const through = viewerEnv.resolveScenePoint(line.binding.throughIndex);
            return through ? viewerEnv.toScreen(through) : null;
          })()
        : line.binding.kind === "angle-bisector-ray"
          ? (() => {
              const vertex = viewerEnv.resolveScenePoint(line.binding.vertexIndex);
              return vertex ? viewerEnv.toScreen(vertex) : null;
            })()
          : (() => {
              const startPoint = viewerEnv.resolveScenePoint(line.binding.startIndex);
              if (!startPoint) return null;
              return viewerEnv.toScreen(startPoint);
            })();
      const end = line.binding.kind === "perpendicular-line"
        ? (() => {
            const through = viewerEnv.resolveScenePoint(line.binding.throughIndex);
            if (!through) return null;
            const hostLine = resolveHostLinePoints(line.binding);
            if (!hostLine) return null;
            const [lineStart, lineEnd] = hostLine;
            if (!lineStart || !lineEnd) return null;
            const dx = lineEnd.x - lineStart.x;
            const dy = lineEnd.y - lineStart.y;
            const len = Math.hypot(dx, dy);
            if (len <= 1e-9) return null;
            return viewerEnv.toScreen({ x: through.x - dy / len, y: through.y + dx / len });
          })()
        : line.binding.kind === "parallel-line"
          ? (() => {
              const through = viewerEnv.resolveScenePoint(line.binding.throughIndex);
              if (!through) return null;
              const hostLine = resolveHostLinePoints(line.binding);
              if (!hostLine) return null;
              const [lineStart, lineEnd] = hostLine;
              if (!lineStart || !lineEnd) return null;
              const dx = lineEnd.x - lineStart.x;
              const dy = lineEnd.y - lineStart.y;
              const len = Math.hypot(dx, dy);
              if (len <= 1e-9) return null;
              return viewerEnv.toScreen({ x: through.x + dx / len, y: through.y + dy / len });
            })()
          : line.binding.kind === "angle-bisector-ray"
            ? (() => {
                const startPoint = viewerEnv.resolveScenePoint(line.binding.startIndex);
                const vertex = viewerEnv.resolveScenePoint(line.binding.vertexIndex);
                const endPoint = viewerEnv.resolveScenePoint(line.binding.endIndex);
                if (!startPoint || !vertex || !endPoint) return null;
                const startDx = startPoint.x - vertex.x;
                const startDy = startPoint.y - vertex.y;
                const startLen = Math.hypot(startDx, startDy);
                const endDx = endPoint.x - vertex.x;
                const endDy = endPoint.y - vertex.y;
                const endLen = Math.hypot(endDx, endDy);
                if (startLen <= 1e-9 || endLen <= 1e-9) return null;
                const sumX = startDx / startLen + endDx / endLen;
                const sumY = startDy / startLen + endDy / endLen;
                const sumLen = Math.hypot(sumX, sumY);
                const direction = sumLen > 1e-9
                  ? { x: sumX / sumLen, y: sumY / sumLen }
                  : { x: -startDy / startLen, y: startDx / startLen };
                return viewerEnv.toScreen({ x: vertex.x + direction.x, y: vertex.y + direction.y });
              })()
            : (() => {
                const endPoint = viewerEnv.resolveScenePoint(line.binding.endIndex);
                return endPoint ? viewerEnv.toScreen(endPoint) : null;
              })();
      if (!start || !end) {
        return null;
      }
      return [start, end];
    }
    const points = viewerEnv.resolveLinePoints
      ? viewerEnv.resolveLinePoints(line)
      : line.points.map((/** @type {PointHandle} */ handle) => viewerEnv.resolvePoint(handle));
    if (!points || points.length < 2 || points.some((/** @type {Point | null} */ point) => !point)) {
      return null;
    }
    return points.map((/** @type {Point} */ point) => viewerEnv.toScreen(point));
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitLine(screenX, screenY) {
    const lines = currentScene().lines || [];
    const point = { x: screenX, y: screenY };
    const toleranceSquared = 10 * 10;
    for (let index = lines.length - 1; index >= 0; index -= 1) {
      const screenPoints = resolveLineScreenPoints(lines[index]);
      if (!screenPoints || screenPoints.length < 2) {
        continue;
      }
      if (distanceToPolylineSquared(point, screenPoints) <= toleranceSquared) {
        return index;
      }
    }
    return null;
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitCircle(screenX, screenY) {
    const circles = currentScene().circles || [];
    const strokeTolerance = 10;
    for (let index = circles.length - 1; index >= 0; index -= 1) {
      const circle = circles[index];
      if (circle.visible === false) {
        continue;
      }
      const centerWorld = viewerEnv.resolvePoint(circle.center);
      const radiusPointWorld = viewerEnv.resolvePoint(circle.radiusPoint);
      if (!centerWorld || !radiusPointWorld) {
        continue;
      }
      const center = viewerEnv.toScreen(centerWorld);
      const radius = Math.hypot(
        radiusPointWorld.x - centerWorld.x,
        radiusPointWorld.y - centerWorld.y,
      ) * center.scale;
      if (!Number.isFinite(radius) || radius <= 1e-6) {
        continue;
      }
      const distance = Math.hypot(screenX - center.x, screenY - center.y);
      const hitsStroke = Math.abs(distance - radius) <= strokeTolerance;
      const hitsFill = !!circle.fillColor && distance <= radius;
      if (hitsStroke || hitsFill) {
        return index;
      }
    }
    return null;
  }

  /**
   * @param {RuntimeArcJson} arc
   * @returns {Point[] | null}
   */
  function resolveArcScreenPolyline(arc) {
    if (arc.visible === false || !Array.isArray(arc.points) || arc.points.length !== 3) {
      return null;
    }
    /** @type {Point[]} */
    let screenPoints;
    if (arc.center) {
      const startWorld = viewerEnv.resolvePoint(arc.points[0]);
      const endWorld = viewerEnv.resolvePoint(arc.points[2]);
      const centerWorld = viewerEnv.resolvePoint(arc.center);
      if (!startWorld || !endWorld || !centerWorld) {
        return null;
      }
      const midpointWorld = midpointOnCircleWorld(
        startWorld,
        endWorld,
        centerWorld,
        arc.counterclockwise !== false,
        !!viewerEnv.sourceScene.yUp,
      );
      if (!midpointWorld) {
        return null;
      }
      screenPoints = [
        viewerEnv.toScreen(startWorld),
        viewerEnv.toScreen(midpointWorld),
        viewerEnv.toScreen(endWorld),
      ];
    } else {
      const worldPoints = arc.points.map((/** @type {PointHandle} */ handle) => viewerEnv.resolvePoint(handle));
      if (worldPoints.some((/** @type {Point | null} */ point) => !point)) {
        return null;
      }
      screenPoints = worldPoints.map((/** @type {Point} */ point) => viewerEnv.toScreen(point));
    }
    const geometry = arcGeometryFromPoints(screenPoints[0], screenPoints[1], screenPoints[2]);
    if (!geometry) {
      return null;
    }
    const tau = Math.PI * 2;
    const ccwSpan = ((geometry.endAngle - geometry.startAngle) % tau + tau) % tau;
    const clockwiseSpan = ccwSpan === 0 ? 0 : tau - ccwSpan;
    const useCounterClockwise = !!geometry.counterClockwise;
    const sweep = useCounterClockwise ? ccwSpan : -clockwiseSpan;
    const samples = 24;
    return Array.from({ length: samples + 1 }, (_, index) => {
      const t = index / samples;
      const angle = geometry.startAngle + sweep * t;
      return {
        x: geometry.center.x + geometry.radius * Math.cos(angle),
        y: geometry.center.y + geometry.radius * Math.sin(angle),
      };
    });
  }

  /**
   * @param {number} screenX
   * @param {number} screenY
   */
  function findHitArc(screenX, screenY) {
    const arcs = currentScene().arcs || [];
    const point = { x: screenX, y: screenY };
    const toleranceSquared = 10 * 10;
    for (let index = arcs.length - 1; index >= 0; index -= 1) {
      const screenPolyline = resolveArcScreenPolyline(arcs[index]);
      if (!screenPolyline || screenPolyline.length < 2) {
        continue;
      }
      if (distanceToPolylineSquared(point, screenPolyline) <= toleranceSquared) {
        return index;
      }
    }
    return null;
  }

  /**
   * @param {number} pointerId
   * @param {Point} position
   * @param {number | null} pointIndex
   * @param {number | null} labelIndex
   * @param {number | null} polygonIndex
   * @param {number | null} iterationTableIndex
   * @param {number | null} imageIndex
   */
  function beginDrag(pointerId, position, pointIndex, labelIndex, polygonIndex, iterationTableIndex, imageIndex) {
    dragModule.beginDrag(
      viewerEnv,
      pointerId,
      position,
      pointIndex,
      labelIndex,
      polygonIndex,
      iterationTableIndex,
      imageIndex,
    );
  }

  /** @param {Point} world */
  function updateDraggedPoint(world) {
    dragModule.updateDraggedPoint(viewerEnv, world);
  }

  /** @param {Point} world */
  function updateDraggedLabel(world) {
    dragModule.updateDraggedLabel(viewerEnv, world);
  }

  /** @param {Point} position */
  function updateDraggedImage(position) {
    dragModule.updateDraggedImage(viewerEnv, position);
  }

  /** @param {Point} position */
  function updateDraggedIterationTable(position) {
    dragModule.updateDraggedIterationTable(viewerEnv, position);
  }

  /** @param {Point} world */
  function updateDraggedPolygon(world) {
    dragModule.updateDraggedPolygon(viewerEnv, world);
  }

  /** @param {Point} position */
  function panFromPointerDelta(position) {
    dragModule.panFromPointerDelta(viewerEnv, position);
    draw();
    overlayRuntime.render();
  }

  function draw() {
    renderModule.draw(viewerEnv);
  }

  /** @type {ViewerEnv} */
  const viewerEnv = {
    canvas,
    svg: canvas,
    gridLayer,
    sceneLayer,
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
    currentHotspotFlashes: () => overlayRuntime.currentHotspotFlashes(),
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
    markDependencyRootsDirty,
    syncDynamicScene: () => dynamicsModule.syncDynamicScene(viewerEnv),
    isOriginPointIndex,
    formatNumber,
    formatAxisNumber,
    formatPiLabel,
    inputTag: input,
    labelTag: label,
    parameterControls,
    van,
    createSvgElement,
    setSvgAttributes,
    clearSvgChildren,
    measureText,
    registerDebugElement,
    selectDebugTarget,
    drawGrid: () => sceneModule.drawGrid(viewerEnv),
  };
  overlayRuntime = overlayModule?.init ? overlayModule.init(viewerEnv, buttonOverlays) : overlayRuntime;
  canvas?.addEventListener("click", (event) => {
    const targetElement = /** @type {Element | null} */ (event.target);
    if (selectDebugTargetFromElement(targetElement)) {
      return;
    }
    const elementAtPoint = document.elementFromPoint(event.clientX, event.clientY);
    if (selectDebugTargetFromElement(elementAtPoint)) {
      return;
    }
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const debugTarget = findDebugTargetAtScreen(position.x, position.y);
    if (debugTarget) {
      selectDebugTarget(debugTarget);
    }
  });
  buttonOverlays?.addEventListener("click", (event) => {
    const targetElement = /** @type {Element | null} */ (event.target);
    if (selectDebugTargetFromElement(targetElement)) {
      return;
    }
    const elementAtPoint = document.elementFromPoint(event.clientX, event.clientY);
    selectDebugTargetFromElement(elementAtPoint);
  });

  window.gspDebug = {
    sourceScene,
    viewerEnv,
    get runtime() {
      return buildRuntimeSnapshot();
    },
    get dependencyRun() {
      return cloneForDebug(lastDependencyRun);
    },
    get selection() {
      return debugEntityWithLiveParameters(lookupDebugEntity(selectedDebugTargetState.val));
    },
    json() {
      return buildDebugJson();
    },
    scene() {
      return buildDebugGraph(currentScene());
    },
    graph() {
      return buildDebugGraph(currentScene());
    },
    inspectSelection() {
      return buildSelectionDebugOutput();
    },
    /** @param {Element} element */
    inspectElement(element) {
      const carrier = element?.closest?.("[data-gsp-debug-id]");
      if (!carrier) {
        return null;
      }
      const debugId = carrier.getAttribute("data-gsp-debug-id");
      const entry = debugId ? debugElementRegistry.get(debugId) : null;
      if (!entry) {
        return null;
      }
      selectDebugTarget(entry.target);
      return cloneForDebug(lookupDebugEntity(entry.target));
    },
    dumpJson() {
      console.log(buildDebugJson());
    },
    dumpScene() {
      console.log(buildDebugGraph(currentScene()));
    },
    dumpSelection() {
      console.log(buildSelectionDebugOutput());
    },
    dependencyGraph() {
      return dynamicsModule.describeDependencyGraph
        ? dynamicsModule.describeDependencyGraph(viewerEnv)
        : [];
    },
    dump() {
      dumpDebugToConsole();
    },
    openPanel() {
      setDebugPanelOpen(true);
    },
    closePanel() {
      setDebugPanelOpen(false);
    },
    togglePanel() {
      setDebugPanelOpen(debugPanel?.hidden !== false);
    },
  };

  debugToggleButton?.addEventListener("click", () => {
    setDebugPanelOpen(debugPanel?.hidden !== false);
  });
  debugDumpConsoleButton?.addEventListener("click", () => {
    dumpDebugToConsole();
  });
  debugTabButtons.forEach((button) => {
    button.addEventListener("click", () => {
      const tab = button.dataset.debugTab;
      debugViewState.val = tab === "json" || tab === "scene" ? tab : "selection";
      renderDebugOutput();
    });
  });
  renderDebugOutput();

  van.derive(() => {
    draw();
    return 0;
  });

  van.derive(() => {
    overlayRuntime.render();
    return 0;
  });

  canvas.addEventListener("pointerdown", (event) => {
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const pointIndex = findHitPoint(position.x, position.y);
    const imageIndex = pointIndex === null
      ? (renderModule.findHitImage ? renderModule.findHitImage(viewerEnv, position.x, position.y) : null)
      : null;
    const iterationTableIndex =
      pointIndex === null && imageIndex === null
        ? findHitIterationTable(position.x, position.y)
        : null;
    const labelIndex = pointIndex === null && imageIndex === null && iterationTableIndex === null
      ? findHitLabel(position.x, position.y)
      : null;
    const polygonIndex = pointIndex === null && imageIndex === null && iterationTableIndex === null && labelIndex === null
      ? findHitPolygon(position.x, position.y)
      : null;
    beginDrag(event.pointerId, position, pointIndex, labelIndex, polygonIndex, iterationTableIndex, imageIndex);
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
    } else if (dragState.val.mode === "image") {
      updateDraggedImage(position);
    } else if (dragState.val.mode === "polygon") {
      updateDraggedPolygon(sceneModule.toWorld(viewerEnv, position.x, position.y));
    } else if (dragState.val.mode === "label") {
      updateDraggedLabel(position);
    } else if (dragState.val.mode === "iteration-table") {
      updateDraggedIterationTable(position);
    } else {
      panFromPointerDelta(position);
    }
    dragState.val = { ...dragState.val, lastX: position.x, lastY: position.y };
  });

  /** @param {number} pointerId */
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
    draw();
    overlayRuntime.render();
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
      return;
    }
    if (event.key === "D" && event.shiftKey) {
      event.preventDefault();
      setDebugPanelOpen(debugPanel?.hidden !== false);
    }
  });

  dynamicsModule.syncDynamicScene(viewerEnv);
  dynamicsModule.buildParameterControls(viewerEnv);
  resetView();
  if (autoOpenDebug) {
    setDebugPanelOpen(true);
    dumpDebugToConsole();
  }
})();
