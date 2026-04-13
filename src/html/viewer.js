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
  /** @type {{ val: "graph" | "json" }} */
  const debugViewState = van?.state ? van.state("graph") : { val: "graph" };
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
      })),
      origin: scene.origin ? attachPointRef(scene.origin) : null,
      lines: hydratedLines,
      polygons: scene.polygons.map((polygon) => ({
        color: polygon.color,
        outlineColor: polygon.outlineColor,
        visible: polygon.visible !== false,
        points: polygon.points.map(attachPointRef),
        binding: polygon.binding ? { ...polygon.binding } : null,
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
      })),
      arcs: (scene.arcs || []).map((arc) => ({
        color: arc.color,
        visible: arc.visible !== false,
        points: arc.points.map(attachPointRef),
        center: arc.center ? attachPointRef(arc.center) : null,
        counterclockwise: !!arc.counterclockwise,
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
      })),
      iterationTables: (scene.iterationTables || []).map((table) => ({
        ...table,
        /** @type {RuntimeIterationRow[]} */
        rows: [],
      })),
      buttons: (scene.buttons || []).map((button) => ({
        ...button,
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

  /** @param {(draft: ViewerSceneData) => void} mutator */
  function updateScene(mutator) {
    const next = sceneState.val;
    mutator(next);
    if (dynamicsModule.refreshDerivedPoints) {
      dynamicsModule.refreshDerivedPoints(viewerEnv, next);
    }
    if (dynamicsModule.refreshIterationGeometry) {
      const parameters = dynamicsModule.parameterMapForScene
        ? dynamicsModule.parameterMapForScene(viewerEnv, next)
        : new Map();
      dynamicsModule.refreshIterationGeometry(viewerEnv, next, parameters);
    }
    if (dynamicsModule.refreshDynamicLabels) {
      dynamicsModule.refreshDynamicLabels(viewerEnv, next);
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
    return cloneForDebug({
      view: { ...viewState.val },
      scene: currentScene(),
      dynamics: currentDynamics(),
      buttons: overlayRuntime.currentButtons(),
    });
  }

  function renderDebugOutput() {
    if (!debugOutput) {
      return;
    }
    const activeTab = debugViewState.val === "json" ? "json" : "graph";
    debugOutput.textContent = activeTab === "json"
      ? buildDebugJson()
      : buildDebugGraph(currentScene());
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
    const graph = buildDebugGraph(currentScene());
    const runtime = buildRuntimeSnapshot();
    console.groupCollapsed("gspDebug");
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
    drawGrid: () => sceneModule.drawGrid(viewerEnv),
  };
  overlayRuntime = overlayModule?.init ? overlayModule.init(viewerEnv, buttonOverlays) : overlayRuntime;

  window.gspDebug = {
    sourceScene,
    viewerEnv,
    get runtime() {
      return buildRuntimeSnapshot();
    },
    json() {
      return buildDebugJson();
    },
    graph() {
      return buildDebugGraph(currentScene());
    },
    dumpJson() {
      console.log(buildDebugJson());
    },
    dumpGraph() {
      console.log(buildDebugGraph(currentScene()));
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
      debugViewState.val = button.dataset.debugTab === "json" ? "json" : "graph";
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
