(() => {
  const van = window.van;
  const { label, input } = van.tags;
  const {
    scene: sceneModule,
    render: renderModule,
    overlay: overlayModule,
    drag: dragModule,
    dynamics: dynamicsModule,
  } = window.GspViewerModules as ViewerModules;
  const SVG_NS = "http://www.w3.org/2000/svg";
  const XLINK_NS = "http://www.w3.org/1999/xlink";
  const sceneDataElement = document.getElementById("scene-data");
  const {
    raw: rawSceneData,
    pages: documentPages,
    activePageIndex,
    sourceScene,
  } = window.GspViewerModules.appDocument.readSceneData(sceneDataElement);

  const canvas = document.getElementById("view") as unknown as SVGSVGElement;
  document.documentElement.style.setProperty("--scene-width", String(sourceScene.width));
  document.documentElement.style.setProperty("--scene-height", String(sourceScene.height));
  canvas.setAttribute("viewBox", `0 0 ${sourceScene.width} ${sourceScene.height}`);
  canvas.setAttribute("width", String(sourceScene.width));
  canvas.setAttribute("height", String(sourceScene.height));
  if (sourceScene.backgroundColor) {
    canvas.style.background = rgba(sourceScene.backgroundColor);
  }

  const gridLayer = document.getElementById("grid-layer") as unknown as SVGGElement;

  const sceneLayer = document.getElementById("scene-layer") as unknown as SVGGElement;

  const measureTextNode = document.getElementById("measure-text") as unknown as SVGTextElement;

  const viewerShell = document.getElementById("viewer-shell");

  const resetButton = document.getElementById("reset-view");

  const fullscreenToggleButton = document.getElementById("toggle-fullscreen");

  const debugToggleButton = document.getElementById("toggle-debug");

  const parameterControls = document.getElementById("parameter-controls");

  const buttonOverlays = document.getElementById("button-overlays");

  const debugPanel = document.getElementById("debug-panel");

  const debugOutput = document.getElementById("debug-output");

  const debugDumpConsoleButton = document.getElementById("debug-dump-console");

  const debugTabButtons = Array.from(
    document.querySelectorAll<HTMLButtonElement>("[data-debug-tab]"),
  );

  const pageTabButtons = Array.from(
    document.querySelectorAll<HTMLButtonElement>("[data-page-index]"),
  );

  const coordReadout = document.getElementById("coord-readout");

  const zoomReadout = document.getElementById("zoom-readout");
  type DebugTarget = { category: string; index: number; hotspotIndex?: number | null; label?: string | null };
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

  const debugViewState = van?.state ? van.state("selection") : { val: "selection" };

  const selectedDebugTargetState = van?.state ? van.state(null) : { val: null };

  const debugElementRegistry = new Map();
  let nextDebugElementId = 1;

  window.GspViewerModules.appDocument.installPageNavigation(
    documentPages,
    activePageIndex,
    pageTabButtons,
  );

  const viewState = van?.state ? van.state({
    centerX: baseCenterX,
    centerY: baseCenterY,
    zoom: defaultZoom,
  }) : { val: {
    centerX: baseCenterX,
    centerY: baseCenterY,
    zoom: defaultZoom,
  } };


  function setViewState(next: ViewState) {
    viewState.val = next;
  }


  function updateViewState(mutator: (draft: ViewState) => void) {
    const next = { ...viewState.val };
    mutator(next);
    setViewState(next);
  }


  function toWorldForView(viewSnapshot: ViewState, screenX: number, screenY: number) {
    const spanX = baseSpanX / viewSnapshot.zoom;
    const spanY = baseSpanY / viewSnapshot.zoom;
    const minX = viewSnapshot.centerX - spanX / 2;
    const minY = viewSnapshot.centerY - spanY / 2;
    const scale = Math.min(usableWidth / spanX, usableHeight / spanY);
    return {
      x: minX + (screenX - margin) / scale,
      y: sourceScene.yUp
        ? minY + (sourceScene.height - margin - screenY) / scale
        : minY + (screenY - margin) / scale,
      scale,
    };
  }


  const view: ViewState = new Proxy({} as ViewState, {
    get: (_target: ViewState, key: string | symbol) => viewState.val[key as keyof ViewState],
    set: (_target: ViewState, key: string | symbol, value: number) => {
      updateViewState((draft: ViewState) => {
        draft[key as keyof ViewState] = value;
      });
      return true;
    },
  });

  const dragState = van?.state ? van.state(null) : { val: null };

  const hoverPointIndex = van?.state ? van.state(null) : { val: null };
  const labelAttachDistance = 40;

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


  function createSvgElement(name: string, attrs: Record<string, string | number | boolean | null | undefined> = {}) {
    const element = document.createElementNS(SVG_NS, name);
    setSvgAttributes(element, attrs);
    return element;
  }


  function setSvgAttributes(element: Element, attrs: Record<string, string | number | boolean | null | undefined>) {
    Object.entries(attrs).forEach(([key, value]: [string, string | number | boolean | null | undefined]) => {
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


  function clearSvgChildren(element: Element) {
    element.replaceChildren();
  }


  function measureText(text: string, fontSize: number = 18, fontWeight: number | string = 400) {
    const normalized = text || "";
    measureTextNode.setAttribute("font-size", String(fontSize));
    measureTextNode.setAttribute("font-weight", String(fontWeight));
    measureTextNode.setAttribute("font-family", "\"Noto Sans\", \"Segoe UI\", sans-serif");
    measureTextNode.textContent = normalized || " ";
    const width = measureTextNode.getBBox().width;
    measureTextNode.textContent = "";
    return normalized ? width : 0;
  }


  function samePoint(left: Point, right: Point) {
    return Math.abs(left.x - right.x) < pointMatchTolerance
      && Math.abs(left.y - right.y) < pointMatchTolerance;
  }


  function resolveSourcePoint(index: number) {
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


  function attachPointRef(point: Point) {
    const pointIndex = sourceScene.points.findIndex((_candidate: ScenePointJson, index: number) => samePoint(resolveSourcePoint(index), point));
    if (pointIndex >= 0) {
      return { pointIndex };
    }
    return { x: point.x, y: point.y };
  }


  function resolveSourceHandle(handle: PointHandle) {
    if (hasPointIndexHandle(handle)) {
      return resolveSourcePoint(handle.pointIndex);
    }
    return  (handle);
  }


  function distanceSquared(left: Point, right: Point) {
    const dx = left.x - right.x;
    const dy = left.y - right.y;
    return dx * dx + dy * dy;
  }


  function distanceToSegmentSquared(point: Point, start: Point, end: Point) {
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


  function distanceToPolylineSquared(point: Point, polyline: Point[]) {
    let best = Number.POSITIVE_INFINITY;
    for (let index = 0; index + 1 < polyline.length; index += 1) {
      best = Math.min(best, distanceToSegmentSquared(point, polyline[index], polyline[index + 1]));
    }
    return best;
  }


  function arcGeometryFromPoints(start: Point, mid: Point, end: Point) {
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


  function midpointOnCircleWorld(start: Point, end: Point, center: Point, counterclockwise: boolean, yUp: boolean) {
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


  function clonePayloadDebug(debug: DebugSourceJson | null) {
    return debug ? {
      ...debug,
      recordTypes: [...(debug.recordTypes || [])],
      recordNames: [...(debug.recordNames || [])],
    } : null;
  }


  function attachLabelAnchor(point: Point, hydratedLines: Array<{ points: PointHandle[] }>) {
    let bestPointIndex = null;
    let bestPointDistanceSquared = Number.POSITIVE_INFINITY;
    sourceScene.points.forEach((_candidate: ScenePointJson, index: number) => {
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
    hydratedLines.forEach((line: { points: PointHandle[] }, lineIndex: number) => {
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


  function attachPointCenteredLabelAnchor(label: { binding?: { kind?: string; pointIndex?: number; anchorDx?: number; anchorDy?: number } | null; anchor: Point }, hydratedLines: Array<{ points: PointHandle[] }>) {
    if (typeof label.binding?.pointIndex === "number") {
      return {
        pointIndex: label.binding.pointIndex,
        dx: label.binding.anchorDx || 0,
        dy: label.binding.anchorDy || 0,
      };
    }
    return attachLabelAnchor(label.anchor, hydratedLines);
  }


  function usesFixedLabelAnchor(label: { binding?: { kind?: string } | null }) {
    return label.binding?.kind === "point-coordinate-value"
      || label.binding?.kind === "point-axis-value"
      || label.binding?.kind === "point-distance-value";
  }


  function hasPointIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { pointIndex: number }> {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }


  function hydrateScene(scene: SceneData): ViewerSceneData {
    const hydratedLines = scene.lines.map((line: LineJson): RuntimeLineJson => ({
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
      bounds: { ...scene.bounds },
      images: (scene.images || []).map((image: ImageJson) => ({
        topLeft: { ...image.topLeft },
        bottomRight: { ...image.bottomRight },
        src: image.src,
        visible: image.visible !== false,
        screenSpace: !!image.screenSpace,
        debug: clonePayloadDebug(image.debug),
      })),
      points: scene.points.map((point: ScenePointJson): RuntimeScenePointJson => ({
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
      polygons: scene.polygons.map((polygon: PolygonJson): RuntimePolygonJson => ({
        color: polygon.color,
        visible: polygon.visible !== false,
        points: polygon.points.map(attachPointRef),
        colorBinding: polygon.colorBinding ? { ...polygon.colorBinding } : null,
        binding: polygon.binding ? { ...polygon.binding } : null,
        debug: clonePayloadDebug(polygon.debug),
      })),
      circles: scene.circles.map((circle: CircleJson): RuntimeCircleJson => ({
        color: circle.color,
        fillColor: circle.fillColor || null,
        fillVisible: circle.fillVisible !== false,
        fillColorBinding: circle.fillColorBinding ? { ...circle.fillColorBinding } : null,
        dashed: !!circle.dashed,
        visible: circle.visible !== false,
        center: attachPointRef(circle.center),
        radiusPoint: attachPointRef(circle.radiusPoint),
        binding: circle.binding ? { ...circle.binding } : null,
        debug: clonePayloadDebug(circle.debug),
      })),
      arcs: (scene.arcs || []).map((arc: ArcJson): RuntimeArcJson => ({
        color: arc.color,
        visible: arc.visible !== false,
        points: arc.points.map(attachPointRef),
        center: arc.center ? attachPointRef(arc.center) : null,
        counterclockwise: !!arc.counterclockwise,
        debug: clonePayloadDebug(arc.debug),
      })),
      labels: scene.labels.map((label: LabelJson): RuntimeLabelJson => ({
        text: label.text,
        richMarkup: label.richMarkup || null,
        color: label.color,
        fontSize: label.fontSize || null,
        fontFamily: label.fontFamily || null,
        visible: label.visible !== false,
        anchor: label.screenSpace
          ? { ...label.anchor }
          : usesFixedLabelAnchor(label)
            ? { ...label.anchor }
          : label.binding?.kind === "point-anchor"
            ? {
                pointIndex: label.binding.pointIndex,
                dx: label.binding.anchorDx,
                dy: label.binding.anchorDy,
              }
          : label.binding?.kind === "point-expression-value"
            ? attachPointCenteredLabelAnchor(label, hydratedLines)
            : attachLabelAnchor(label.anchor, hydratedLines),
        binding: label.binding ? { ...label.binding } : null,
        screenSpace: !!label.screenSpace,
        centeredOnAnchor: false,
        hotspots: (label.hotspots || []).map((hotspot: LabelHotspotJson): RuntimeLabelHotspotJson => ({
          ...hotspot,
          action: hotspot.action ? { ...hotspot.action } : null,
        })),
        debug: clonePayloadDebug(label.debug),
      })),
      iterationTables: (scene.iterationTables || []).map((table: IterationTableJson): RuntimeIterationTableJson => ({
        ...table,
        debug: clonePayloadDebug(table.debug),

        rows: [],
      })),
      buttons: (scene.buttons || []).map((button: ButtonJson): RuntimeButtonJson => ({
        ...button,
        debug: clonePayloadDebug(button.debug),
        baseText: button.text,
        visible: button.visible !== false,
        active: false,
      })),
    };
  }

  const sceneState = van?.state ? van.state(hydrateScene(sourceScene)) : { val: hydrateScene(sourceScene) };
  const dynamicsState = van?.state ? van.state({
    parameters: (sourceScene.parameters || []).map((parameter: ParameterJson) => ({ ...parameter })),
    functions: (sourceScene.functions || []).map((functionDef: FunctionJson) => ({
      ...functionDef,
      expr: functionDef.expr,
      domain: functionDef.domain,
      constrainedPointIndices: [...functionDef.constrainedPointIndices],
    })),
  }) : { val: {
    parameters: (sourceScene.parameters || []).map((parameter: ParameterJson) => ({ ...parameter })),
    functions: (sourceScene.functions || []).map((functionDef: FunctionJson) => ({
      ...functionDef,
      expr: functionDef.expr,
      domain: functionDef.domain,
      constrainedPointIndices: [...functionDef.constrainedPointIndices],
    })),
  } };
  const currentScene = () => sceneState.val;
  const currentDynamics = () => dynamicsState.val;

  const pendingDependencyRootIds = new Set<string>();

  let lastDependencyRun = null;


  function markDependencyRootsDirty(rootIds: string | string[]) {
    const values = Array.isArray(rootIds) ? rootIds : [rootIds];
    values.forEach((rootId: string) => {
      if (typeof rootId === "string" && rootId.length > 0) {
        pendingDependencyRootIds.add(rootId);
      }
    });
  }


  function updateScene(mutator: (draft: ViewerSceneData) => void, mode: "graph" | "none" = "none") {
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


  function updateDynamics(mutator: (draft: RuntimeDynamicsState) => void) {
    const next = dynamicsState.val;
    mutator(next);
    dynamicsState.val = { ...next };
  }


  function rgba(color: [number, number, number, number]) {
    return `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${(color[3] / 255).toFixed(3)})`;
  }


  function formatNumber(value: number) {
    if (!Number.isFinite(value)) return "-";
    return Math.abs(value - Math.round(value)) < 0.005
      ? String(Math.round(value))
      : value.toFixed(2);
  }

  const debugGraphRuntime = window.GspViewerModules.appDebugGraph.createDebugGraphRuntime({
    formatNumber,
  });


  function formatAxisNumber(value: number) {
    if (Math.abs(value - Math.round(value)) < 1e-6) {
      return String(Math.round(value));
    }
    return value.toFixed(1);
  }


  function formatPiLabel(stepIndex: number) {
    if (stepIndex === 0) return "";
    const sign = stepIndex < 0 ? "-" : "";
    const absIndex = Math.abs(stepIndex);
    if (absIndex % 2 === 0) {
      const multiple = absIndex / 2;
      return multiple === 1 ? `${sign}\u03c0` : `${sign}${multiple}\u03c0`;
    }
    return absIndex === 1 ? `${sign}\u03c0/2` : `${sign}${absIndex}\u03c0/2`;
  }


  function cloneForDebug(value: unknown) {
    if (typeof structuredClone === "function") {
      return structuredClone(value);
    }
    return JSON.parse(JSON.stringify(value));
  }


  function cloneWithLiveParameterValues(value: unknown, parameters: Map<string, number>) {
    if (!value || typeof value !== "object") {
      return value;
    }
    if (Array.isArray(value)) {
      return value.map((item) => cloneWithLiveParameterValues(item, parameters));
    }
    const cloned = Object.fromEntries(
      Object.entries(value).map(([key, child]: [string, unknown]) => [
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
    if (typeof cloned.depth === "number" && typeof cloned.parameterName === "string") {
      const depth = parameters.get(cloned.parameterName);
      if (typeof depth === "number" && Number.isFinite(depth)) {
        cloned.depth = Math.max(0, Math.floor(depth + 1e-9));
      }
    }
    if (typeof cloned.depth === "number" && typeof cloned.depthParameterName === "string") {
      const depth = parameters.get(cloned.depthParameterName);
      if (typeof depth === "number" && Number.isFinite(depth)) {
        cloned.depth = Math.max(0, Math.floor(depth + 1e-9));
      }
    }
    return cloned;
  }


  function debugEntityWithLiveParameters(entity: unknown) {
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


  function debugTargetKey(target: DebugTarget | null) {
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


  function registerDebugElement(element: Element, target: DebugTarget | null | undefined) {
    if (!element || !target) {
      return;
    }
    let debugId = element.getAttribute("data-gsp-debug-id");
    if (!debugId || !debugElementRegistry.has(debugId)) {
      debugId = `dbg-${nextDebugElementId++}`;
    }
    element.setAttribute("data-gsp-debug-id", debugId);
    element.setAttribute("data-gsp-kind", target.category);
    element.setAttribute("data-gsp-index", String(target.index));
    if (target.hotspotIndex !== undefined && target.hotspotIndex !== null) {
      element.setAttribute("data-gsp-hotspot-index", String(target.hotspotIndex));
    } else {
      element.removeAttribute("data-gsp-hotspot-index");
    }
    const entity = lookupDebugEntity(target);
    if (entity?.debug?.groupOrdinal) {
      element.setAttribute("data-gsp-group", String(entity.debug.groupOrdinal));
    } else {
      element.removeAttribute("data-gsp-group");
    }
    debugElementRegistry.set(debugId, { element, target });
    syncDebugSelectionHighlight();
  }


  function selectDebugTarget(target: DebugTarget) {
    selectedDebugTargetState.val = target;
    debugViewState.val = "selection";
    syncDebugSelectionHighlight();
    renderDebugOutput();
  }


  function selectDebugTargetFromElement(element: Element | null) {
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


  function findDebugTargetAtScreen(screenX: number, screenY: number) {
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


  function lookupDebugEntity(target: DebugTarget) {
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


  function describeDebugTarget(target: DebugTarget) {
    const suffix = target.hotspotIndex !== undefined && target.hotspotIndex !== null
      ? `[${target.hotspotIndex}]`
      : "";
    return `${target.category}[${target.index}]${suffix}`;
  }


  function formatPayloadDebug(debug: Record<string, unknown> | null | undefined) {
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
    const entityRecord =  (
      entity && typeof entity === "object" ? entity : null
    );
    const lines = [
      "Selection",
      `  target: ${describeDebugTarget(target)}`,
      `  summary: ${debugGraphRuntime.summarizeDebugEntity(entity) || "(no summary)"}`,
    ];
    formatPayloadDebug(entityRecord?.debug && typeof entityRecord.debug === "object"
      ?  (entityRecord.debug)
      : null).forEach((line) => {
      lines.push(`  ${line}`);
    });
    const refs = debugGraphRuntime.collectReferenceTokens(entity);
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


  function buildRuntimeSnapshot() {
    return  (debugEntityWithLiveParameters({
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
        ? debugGraphRuntime.buildDebugGraph(currentScene())
        : buildSelectionDebugOutput();
    debugTabButtons.forEach((button) => {
      const isActive = button.dataset.debugTab === activeTab;
      button.setAttribute("aria-pressed", isActive ? "true" : "false");
      button.classList.toggle("is-active", isActive);
    });
  }

  function isViewerFullscreen() {
    return document.fullscreenElement === viewerShell;
  }

  function syncFullscreenButton() {
    const fullscreenActive = isViewerFullscreen();
    if (!fullscreenToggleButton) {
      return;
    }
    fullscreenToggleButton.textContent = fullscreenActive ? "退出全屏" : "全屏";
    fullscreenToggleButton.classList.toggle("is-active", fullscreenActive);
    fullscreenToggleButton.setAttribute("aria-pressed", fullscreenActive ? "true" : "false");
  }

  async function toggleFullscreen() {
    if (!viewerShell) {
      return;
    }
    if (isViewerFullscreen()) {
      await document.exitFullscreen?.();
      return;
    }
    await viewerShell.requestFullscreen?.();
  }


  function setDebugPanelOpen(open: boolean) {
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
    const graph = debugGraphRuntime.buildDebugGraph(currentScene());
    const runtime = buildRuntimeSnapshot();
    console.groupCollapsed("gspDebug");
    console.log(selection);
    console.log(graph);
    console.log("sourceScene", cloneForDebug(sourceScene));
    console.log("runtime", runtime);
    console.groupEnd();
  }


  function updateReadout(screenX: number | null = null, screenY: number | null = null) {
    if (screenX === null || screenY === null) {
      pointerWorldState.val = null;
      return;
    }
    pointerWorldState.val = sceneModule.toWorld(viewerEnv, screenX, screenY);
  }

  function resetView() {
    setViewState({
      centerX: baseCenterX,
      centerY: baseCenterY,
      zoom: defaultZoom,
    });
    updateReadout();
  }


  function findHitPoint(screenX: number, screenY: number) {
    return renderModule.findHitPoint(viewerEnv, screenX, screenY);
  }


  function isOriginPointIndex(index: number) {
    const origin = currentScene().origin;
    return !!origin && "pointIndex" in origin && typeof origin.pointIndex === "number" && origin.pointIndex === index;
  }


  function findHitLabel(screenX: number, screenY: number) {
    return renderModule.findHitLabel(viewerEnv, screenX, screenY);
  }


  function findHitIterationTable(screenX: number, screenY: number) {
    return renderModule.findHitIterationTable(viewerEnv, screenX, screenY);
  }


  function findHitPolygon(screenX: number, screenY: number) {
    return renderModule.findHitPolygon ? renderModule.findHitPolygon(viewerEnv, screenX, screenY) : null;
  }


  function resolveLineScreenPoints(line: RuntimeLineJson) {
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
      : line.points.map(( handle) => viewerEnv.resolvePoint(handle));
    if (!points || points.length < 2 || points.some(( point) => !point)) {
      return null;
    }
    return points.map(( point) => viewerEnv.toScreen(point));
  }


  function findHitLine(screenX: number, screenY: number) {
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


  function findHitCircle(screenX: number, screenY: number) {
    const circles = currentScene().circles || [];
    const strokeTolerance = 10;
    for (let index = circles.length - 1; index >= 0; index -= 1) {
      const circle = circles[index];
      if (circle.visible === false && !(circle.fillColor && circle.fillVisible !== false)) {
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
      const hitsStroke = circle.visible !== false && Math.abs(distance - radius) <= strokeTolerance;
      const hitsFill = !!circle.fillColor && circle.fillVisible !== false && distance <= radius;
      if (hitsStroke || hitsFill) {
        return index;
      }
    }
    return null;
  }


  function resolveArcScreenPolyline(arc: RuntimeArcJson) {
    if (arc.visible === false || !Array.isArray(arc.points) || arc.points.length !== 3) {
      return null;
    }

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
      const worldPoints = arc.points.map(( handle) => viewerEnv.resolvePoint(handle));
      if (worldPoints.some(( point) => !point)) {
        return null;
      }
      screenPoints = worldPoints.map(( point) => viewerEnv.toScreen(point));
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
    return Array.from({ length: samples + 1 }, (_, index: number) => {
      const t = index / samples;
      const angle = geometry.startAngle + sweep * t;
      return {
        x: geometry.center.x + geometry.radius * Math.cos(angle),
        y: geometry.center.y + geometry.radius * Math.sin(angle),
      };
    });
  }


  function findHitArc(screenX: number, screenY: number) {
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


  function beginDrag(pointerId: number, position: Point, pointIndex: number | null, labelIndex: number | null, polygonIndex: number | null, iterationTableIndex: number | null, imageIndex: number | null) {
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


  function updateDraggedPoint(world: Point) {
    dragModule.updateDraggedPoint(viewerEnv, world);
  }


  function updateDraggedLabel(world: Point) {
    dragModule.updateDraggedLabel(viewerEnv, world);
  }


  function updateDraggedImage(position: Point) {
    dragModule.updateDraggedImage(viewerEnv, position);
  }


  function updateDraggedIterationTable(position: Point) {
    dragModule.updateDraggedIterationTable(viewerEnv, position);
  }


  function updateDraggedPolygon(world: Point) {
    dragModule.updateDraggedPolygon(viewerEnv, world);
  }


  function panFromPointerDelta(position: Point) {
    const drag = dragState.val;
    if (!drag) return;
    const currentView = viewState.val;
    const worldNow = toWorldForView(currentView, position.x, position.y);
    const worldLast = toWorldForView(currentView, drag.lastX, drag.lastY);
    updateViewState((draft: ViewState) => {
      draft.centerX -= worldNow.x - worldLast.x;
      draft.centerY -= worldNow.y - worldLast.y;
    });
  }

  function draw() {
    renderModule.draw(viewerEnv);
  }


  const viewerEnv: ViewerEnv = {
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
    resolveScenePoint: (index: number) => sceneModule.resolveScenePoint(viewerEnv, index),
    resolvePoint: (handle: PointHandle) => sceneModule.resolvePoint(viewerEnv, handle),
    resolveAnchorBase: (handle: PointHandle) => sceneModule.resolveAnchorBase(viewerEnv, handle),
    resolveLinePoints: (lineOrIndex: number | RuntimeLineJson) => sceneModule.resolveLinePoints(viewerEnv, lineOrIndex),
    toScreen: (point: Point) => sceneModule.toScreen(viewerEnv, point),
    toWorld: (x: number, y: number) => sceneModule.toWorld(viewerEnv, x, y),
    getViewBounds: () => sceneModule.getViewBounds(viewerEnv),
    rgba,
    updateScene,
    updateDynamics,
    updateViewState,
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
  canvas?.addEventListener("click", (event: MouseEvent) => {
    const targetElement = event.target instanceof Element ? event.target : null;
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
  buttonOverlays?.addEventListener("click", (event: MouseEvent) => {
    const targetElement = event.target instanceof Element ? event.target : null;
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
      const target = selectedDebugTargetState.val;
      return debugEntityWithLiveParameters(target ? lookupDebugEntity(target) : null);
    },
    json() {
      return buildDebugJson();
    },
    scene() {
      return debugGraphRuntime.buildDebugGraph(currentScene());
    },
    graph() {
      return debugGraphRuntime.buildDebugGraph(currentScene());
    },
    inspectSelection() {
      return buildSelectionDebugOutput();
    },

    inspectElement(element: Element) {
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
      console.log(debugGraphRuntime.buildDebugGraph(currentScene()));
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
  fullscreenToggleButton?.addEventListener("click", () => {
    toggleFullscreen().catch((error: unknown) => {
      console.warn("failed to toggle fullscreen", error);
    });
  });
  debugDumpConsoleButton?.addEventListener("click", () => {
    dumpDebugToConsole();
  });
  debugTabButtons.forEach((button: HTMLButtonElement) => {
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

  canvas.addEventListener("pointerdown", (event: PointerEvent) => {
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

  canvas.addEventListener("pointermove", (event: PointerEvent) => {
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


  function endDrag(pointerId: number) {
    if (dragState.val && dragState.val.pointerId === pointerId) {
      dragState.val = null;
      canvas.classList.remove("is-dragging");
    }
  }

  canvas.addEventListener("pointerup", (event: PointerEvent) => endDrag(event.pointerId));
  canvas.addEventListener("pointercancel", (event: PointerEvent) => endDrag(event.pointerId));
  canvas.addEventListener("pointerleave", () => {
    hoverPointIndex.val = null;
    if (!dragState.val) {
      updateReadout();
    }
  });

  canvas.addEventListener("wheel", (event: WheelEvent) => {
    event.preventDefault();
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const currentView = viewState.val;
    const before = toWorldForView(currentView, position.x, position.y);
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    const nextView = {
      ...currentView,
      zoom: Math.max(minZoom, Math.min(64, currentView.zoom * factor)),
    };
    const after = toWorldForView(nextView, position.x, position.y);
    nextView.centerX += before.x - after.x;
    nextView.centerY += before.y - after.y;
    setViewState(nextView);
    updateReadout(position.x, position.y);
  }, { passive: false });

  canvas.addEventListener("dblclick", () => {
    resetView();
  });

  resetButton.addEventListener("click", () => {
    resetView();
  });

  window.addEventListener("keydown", (event: KeyboardEvent) => {
    if (event.key === "0") {
      resetView();
      return;
    }
    if (event.key === "D" && event.shiftKey) {
      event.preventDefault();
      setDebugPanelOpen(debugPanel?.hidden !== false);
    }
  });
  document.addEventListener("fullscreenchange", syncFullscreenButton);

  dynamicsModule.syncDynamicScene(viewerEnv);
  dynamicsModule.buildParameterControls(viewerEnv);
  syncFullscreenButton();
  resetView();
  if (autoOpenDebug) {
    setDebugPanelOpen(true);
    dumpDebugToConsole();
  }
})();
