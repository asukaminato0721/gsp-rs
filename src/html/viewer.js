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
    baseText: button.text,
    visible: true,
    active: false,
  }))) : { val: (sourceScene.buttons || []).map((button) => ({
    ...button,
    baseText: button.text,
    visible: true,
    active: false,
  })) };
  const buttonTimers = new Map();
  const buttonAnimations = new Map();
  const hotspotFlashesState = van?.state ? van.state([]) : { val: [] };
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

  function cleanRichText(text) {
    return text
      .replaceAll("\u2013", "-")
      .replaceAll("\u2014", "-")
      .replaceAll("厘米", "cm");
  }

  function decodeRichMarkupText(token) {
    if (!token.startsWith("T")) {
      return null;
    }
    const xIndex = token.indexOf("x");
    if (xIndex < 0) {
      return null;
    }
    return cleanRichText(token.slice(xIndex + 1));
  }

  function parseRichMarkupNodes(markup) {
    function parseSeq(source, start, stopOnGt) {
      const nodes = [];
      let index = start;
      while (index < source.length) {
        if (stopOnGt && source[index] === ">") {
          return [nodes, index + 1];
        }
        if (source[index] !== "<") {
          index += 1;
          continue;
        }
        index += 1;
        const nameStart = index;
        while (index < source.length && source[index] !== "<" && source[index] !== ">") {
          index += 1;
        }
        const name = source.slice(nameStart, index);
        let children = [];
        if (index < source.length && source[index] === "<") {
          [children, index] = parseSeq(source, index, true);
        } else if (index < source.length && source[index] === ">") {
          index += 1;
        }
        nodes.push({ name, children });
      }
      return [nodes, index];
    }

    return parseSeq(markup, 0, false)[0];
  }

  function appendRichMarkupLines(target, lines) {
    if (!lines.length) {
      return;
    }
    if (!target.length) {
      target.push(...lines);
      return;
    }
    const [first, ...rest] = lines;
    target[target.length - 1].push(...first);
    target.push(...rest);
  }

  function renderRichMarkupInline(nodes) {
    return renderRichMarkupNodes(nodes)
      .flatMap((line, index) => (index === 0 ? line : [{ kind: "text", text: " " }, ...line]));
  }

  function renderRichMarkupNode(node) {
    const text = decodeRichMarkupText(node.name);
    if (text !== null) {
      return text ? [[{ kind: "text", text }]] : [[]];
    }
    if (!node.name || node.name.startsWith("!") || node.name.startsWith("?1x")) {
      return renderRichMarkupNodes(node.children);
    }
    if (node.name === "VL") {
      return node.children.flatMap((child) => renderRichMarkupNode(child)).filter((line) => line.length);
    }
    if (node.name === "H") {
      return [renderRichMarkupInline(node.children)];
    }
    if (node.name === "/") {
      const [numerator, ...denominator] = node.children;
      if (!numerator || !denominator.length) {
        return [renderRichMarkupInline(node.children)];
      }
      return [[{
        kind: "fraction",
        numerator: renderRichMarkupInline([numerator]),
        denominator: renderRichMarkupInline(denominator),
      }]];
    }
    if (node.name === "R") {
      return [[{
        kind: "radical",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    if (node.name === "SO2") {
      return [[{
        kind: "overline",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    if (node.name === "SO3") {
      return [[{
        kind: "ray",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    if (node.name === "SO4") {
      return [[{
        kind: "arc",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    return renderRichMarkupNodes(node.children);
  }

  function renderRichMarkupNodes(nodes) {
    const lines = [[]];
    nodes.forEach((node) => {
      appendRichMarkupLines(lines, renderRichMarkupNode(node));
    });
    return lines.filter((line) => line.length);
  }

  function appendRichMarkupItems(parent, items) {
    items.forEach((item) => {
      parent.append(renderRichMarkupItem(item));
    });
  }

  function renderRichMarkupItem(item) {
    if (item.kind === "text") {
      const span = document.createElement("span");
      span.textContent = item.text;
      return span;
    }
    if (item.kind === "fraction") {
      const fraction = document.createElement("span");
      fraction.className = "scene-rich-fraction";
      const numerator = document.createElement("span");
      numerator.className = "scene-rich-fraction-part";
      appendRichMarkupItems(numerator, item.numerator);
      const bar = document.createElement("span");
      bar.className = "scene-rich-fraction-bar";
      const denominator = document.createElement("span");
      denominator.className = "scene-rich-fraction-part";
      appendRichMarkupItems(denominator, item.denominator);
      fraction.append(numerator, bar, denominator);
      return fraction;
    }
    const span = document.createElement("span");
    if (item.kind === "radical") {
      span.className = "scene-rich-radical";
      const symbol = document.createElement("span");
      symbol.className = "scene-rich-radical-symbol";
      symbol.textContent = "\u221a";
      const radicand = document.createElement("span");
      radicand.className = "scene-rich-radicand";
      appendRichMarkupItems(radicand, item.children);
      span.append(symbol, radicand);
      return span;
    }
    span.className = `scene-rich-${item.kind}`;
    appendRichMarkupItems(span, item.children);
    return span;
  }

  function renderRichLabel(label) {
    if (!label.richMarkup) {
      return null;
    }
    const lines = renderRichMarkupNodes(parseRichMarkupNodes(label.richMarkup));
    if (!lines.length) {
      return null;
    }
    const root = document.createElement("div");
    root.className = "scene-rich-label";
    lines.forEach((items) => {
      const line = document.createElement("div");
      line.className = "scene-rich-line";
      appendRichMarkupItems(line, items);
      root.append(line);
    });
    return root;
  }

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
      { sourceScene },
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
      visible: line.visible !== false,
      points: line.points.map(attachPointRef),
      binding: line.binding ? { ...line.binding } : null,
    }));
    return {
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
        rows: [],
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
    dynamicsModule.refreshIterationGeometry(viewerEnv, next, new Map(
      viewerEnv.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value]),
    ));
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

  function cloneForDebug(value) {
    if (typeof structuredClone === "function") {
      return structuredClone(value);
    }
    return JSON.parse(JSON.stringify(value));
  }

  function buildDebugJson() {
    return JSON.stringify(sourceScene, null, 2);
  }

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

  function collectReferenceTokens(value) {
    /** @type {string[]} */
    const refs = [];
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
            .map((item) => (typeof item === "number" ? formatReference(key, item) : null))
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

  function summarizeDebugEntity(entity) {
    const parts = [];
    if (typeof entity.text === "string") {
      parts.push(JSON.stringify(entity.text));
    }
    if (typeof entity.name === "string") {
      parts.push(`name=${entity.name}`);
    }
    if (typeof entity.kind === "string") {
      parts.push(`kind=${entity.kind}`);
    }
    if (typeof entity.visible === "boolean") {
      parts.push(entity.visible ? "visible" : "hidden");
    }
    if (typeof entity.depth === "number") {
      parts.push(`depth=${entity.depth}`);
    }
    if (typeof entity.parameterName === "string" && entity.parameterName.length > 0) {
      parts.push(`param=${entity.parameterName}`);
    }
    if (entity.anchor && typeof entity.anchor === "object") {
      if (typeof entity.anchor.x === "number" && typeof entity.anchor.y === "number") {
        parts.push(`anchor @ (${formatNumber(entity.anchor.x)}, ${formatNumber(entity.anchor.y)})`);
      }
      if (entity.screenSpace === true) {
        parts.push("screenSpace");
      }
    }
    if (typeof entity.x === "number" && typeof entity.y === "number" && !entity.kind) {
      parts.push(`@ (${formatNumber(entity.x)}, ${formatNumber(entity.y)})`);
    }
    return parts.join(" ");
  }

  function appendGraphSection(lines, title, itemLabel, items) {
    lines.push(`${title} (${items.length})`);
    items.forEach((item, index) => {
      const summary = summarizeDebugEntity(item);
      const refs = collectReferenceTokens(item);
      lines.push(`  ${itemLabel}[${index}]${summary ? ` ${summary}` : ""}`);
      if (refs.length > 0) {
        lines.push(`    -> ${refs.join(", ")}`);
      }
    });
  }

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
    appendGraphSection(lines, "Line Iterations", "lineIteration", scene.lineIterations || []);
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
      buttons: buttonsState.val,
    });
  }

  function renderDebugOutput() {
    if (!debugOutput) {
      return;
    }
    const activeTab = debugViewState.val === "json" ? "json" : "graph";
    debugOutput.textContent = activeTab === "json"
      ? buildDebugJson()
      : buildDebugGraph(sourceScene);
    debugTabButtons.forEach((button) => {
      const isActive = button.dataset.debugTab === activeTab;
      button.setAttribute("aria-pressed", isActive ? "true" : "false");
      button.classList.toggle("is-active", isActive);
    });
  }

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
    const graph = buildDebugGraph(sourceScene);
    const runtime = buildRuntimeSnapshot();
    console.groupCollapsed("gspDebug");
    console.log(graph);
    console.log("sourceScene", cloneForDebug(sourceScene));
    console.log("runtime", runtime);
    console.groupEnd();
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
    view.zoom = defaultZoom;
    dynamicsModule.syncDynamicScene(viewerEnv);
    updateReadout();
  }

  function renderOverlays() {
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

    currentScene().labels.forEach((label) => {
      if (label.visible === false) {
        return;
      }
      if (label.richMarkup && !label.hotspots?.length) {
        const anchor = label.screenSpace
          ? label.anchor
          : viewerEnv.resolvePoint(label.anchor);
        if (!anchor) {
          return;
        }
        const screen = label.screenSpace ? anchor : viewerEnv.toScreen(anchor);
        const richLabel = renderRichLabel(label);
        if (!screen || !richLabel) {
          return;
        }
        richLabel.style.color = rgba(label.color);
        richLabel.style.left = `${(((screen.x + (label.centeredOnAnchor ? 0 : 2)) / sourceScene.width) * 100)}%`;
        richLabel.style.top = `${(((screen.y + (label.centeredOnAnchor ? -10 : -14)) / sourceScene.height) * 100)}%`;
        if (label.centeredOnAnchor) {
          richLabel.style.transform = "translate(-50%, -50%)";
        }
        buttonOverlays.append(richLabel);
        return;
      }
      if (!label.hotspots?.length) {
        return;
      }
      renderModule.labelHotspotRects(viewerEnv, label).forEach((rect) => {
        const hotspot = document.createElement("button");
        hotspot.className = "scene-hotspot";
        hotspot.type = "button";
        hotspot.setAttribute("aria-label", rect.text);
        hotspot.style.left = `${(rect.left / sourceScene.width) * 100}%`;
        hotspot.style.top = `${(rect.top / sourceScene.height) * 100}%`;
        hotspot.style.width = `${(rect.width / sourceScene.width) * 100}%`;
        hotspot.style.height = `${(rect.height / sourceScene.height) * 100}%`;
        hotspot.addEventListener("click", (event) => {
          event.preventDefault();
          runHotspotAction(rect.action);
        });
        buttonOverlays.append(hotspot);
      });
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

  function visibilityTargetsMatch(action, visible) {
    const scene = currentScene();
    const pointsMatch = (action.pointIndices || []).every((index) => scene.points[index]?.visible === visible);
    const linesMatch = (action.lineIndices || []).every((index) => scene.lines[index]?.visible === visible);
    const circlesMatch = (action.circleIndices || []).every((index) => scene.circles[index]?.visible === visible);
    const polygonsMatch = (action.polygonIndices || []).every((index) => scene.polygons[index]?.visible === visible);
    return pointsMatch && linesMatch && circlesMatch && polygonsMatch;
  }

  function toggledVisibilityText(baseText, targetsVisible) {
    if (typeof baseText !== "string" || !baseText) {
      return baseText;
    }
    if (targetsVisible) {
      if (baseText.includes("显示")) {
        return baseText.replace("显示", "隐藏");
      }
    } else if (baseText.includes("隐藏")) {
      return baseText.replace("隐藏", "显示");
    }
    return baseText;
  }

  function updateLinkedButtonLabels(buttonIndex, nextText) {
    updateScene((scene) => {
      scene.labels.forEach((label) => {
        if (!label.hotspots?.length) {
          return;
        }
        let lines = label.text.split("\n").map((line) => Array.from(line));
        let changed = false;
        const relevantHotspots = label.hotspots
          .filter((hotspot) =>
            hotspot.action?.kind === "button" && hotspot.action.buttonIndex === buttonIndex
          )
          .sort((left, right) => right.line - left.line || right.start - left.start);
        relevantHotspots.forEach((hotspot) => {
          const line = lines[hotspot.line];
          if (!line) {
            return;
          }
          line.splice(hotspot.start, hotspot.end - hotspot.start, ...Array.from(nextText));
          hotspot.end = hotspot.start + Array.from(nextText).length;
          hotspot.text = nextText;
          changed = true;
        });
        if (changed) {
          label.text = lines.map((line) => line.join("")).join("\n");
        }
      });
    });
  }

  function syncVisibilityButtonState(buttonIndex, action) {
    if (typeof buttonIndex !== "number") {
      return;
    }
    let active = false;
    if (action.kind === "toggle-visibility") {
      active = visibilityTargetsMatch(action, true);
    } else if (action.kind === "set-visibility") {
      active = visibilityTargetsMatch(action, !!action.visible);
    } else if (action.kind === "show-hide-visibility") {
      active = visibilityTargetsMatch(action, true);
    } else {
      return;
    }
    updateButtons((buttons) => {
      if (buttons[buttonIndex]) {
        buttons[buttonIndex].active = active;
        if (action.kind === "show-hide-visibility" || action.kind === "toggle-visibility") {
          buttons[buttonIndex].text = toggledVisibilityText(
            buttons[buttonIndex].baseText || buttons[buttonIndex].text,
            active,
          );
        }
      }
    });
    if (action.kind === "show-hide-visibility" || action.kind === "toggle-visibility") {
      const button = buttonsState.val[buttonIndex];
      if (button) {
        updateLinkedButtonLabels(buttonIndex, button.text);
      }
    }
  }

  function toggleTargetsVisibility(action) {
    const scene = currentScene();
    const hiddenPoint = (action.pointIndices || []).some((index) => scene.points[index]?.visible === false);
    const hiddenLine = (action.lineIndices || []).some((index) => scene.lines[index]?.visible === false);
    const hiddenCircle = (action.circleIndices || []).some((index) => scene.circles[index]?.visible === false);
    const hiddenPolygon = (action.polygonIndices || []).some((index) => scene.polygons[index]?.visible === false);
    setTargetsVisibility(action, hiddenPoint || hiddenLine || hiddenCircle || hiddenPolygon);
  }

  function updateHotspotFlashes(mutator) {
    const next = hotspotFlashesState.val.slice();
    mutator(next);
    hotspotFlashesState.val = next;
  }

  function hotspotFlashKey(action) {
    switch (action.kind) {
      case "button":
        return `button:${action.buttonIndex}`;
      case "point":
        return `point:${action.pointIndex}`;
      case "segment":
        return `segment:${action.startPointIndex}:${action.endPointIndex}`;
      case "angle-marker":
        return `angle:${action.startPointIndex}:${action.vertexPointIndex}:${action.endPointIndex}`;
      case "circle":
        return `circle:${action.circleIndex}`;
      case "polygon":
        return `polygon:${action.polygonIndex}`;
      default:
        return JSON.stringify(action);
    }
  }

  function flashHotspotAction(action) {
    const key = hotspotFlashKey(action);
    updateHotspotFlashes((flashes) => {
      const existingIndex = flashes.findIndex((flash) => flash.key === key);
      if (existingIndex >= 0) {
        flashes.splice(existingIndex, 1);
      }
      flashes.push({ key, action });
    });
    window.setTimeout(() => {
      updateHotspotFlashes((flashes) => {
        const index = flashes.findIndex((flash) => flash.key === key);
        if (index >= 0) {
          flashes.splice(index, 1);
        }
      });
    }, 180);
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
        const parameterized = dynamicsModule.parameterValueFromPoint
          ? dynamicsModule.parameterValueFromPoint(draft, pointIndex)
          : null;
        if (parameterized !== null && draftPoint.constraint) {
          const durationMs = mode === "scroll" ? 16000 : 12000;
          const delta = dt / durationMs;
          if (mode === "scroll") {
            dynamicsModule.applyNormalizedParameterToPoint(
              draftPoint,
              draft,
              parameterized + delta,
            );
          } else {
            let next = parameterized + delta * state.direction;
            if (next >= 1) {
              next = 1;
              state.direction = -1;
            } else if (next <= 0) {
              next = 0;
              state.direction = 1;
            }
            dynamicsModule.applyNormalizedParameterToPoint(draftPoint, draft, next);
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
        syncVisibilityButtonState(buttonIndex, action);
        break;
      case "set-visibility":
        setTargetsVisibility(action, !!action.visible);
        syncVisibilityButtonState(buttonIndex, action);
        break;
      case "show-hide-visibility": {
        const nextVisible = !visibilityTargetsMatch(action, true);
        setTargetsVisibility(action, nextVisible);
        syncVisibilityButtonState(buttonIndex, action);
        break;
      }
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

  function runHotspotAction(action) {
    if (!action) {
      return;
    }
    if (action.kind === "button" && typeof action.buttonIndex === "number") {
      runButtonAction(action.buttonIndex);
      return;
    }
    flashHotspotAction(action);
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

  function findHitIterationTable(screenX, screenY) {
    return renderModule.findHitIterationTable(viewerEnv, screenX, screenY);
  }

  function findHitPolygon(screenX, screenY) {
    return renderModule.findHitPolygon ? renderModule.findHitPolygon(viewerEnv, screenX, screenY) : null;
  }

  function beginDrag(pointerId, position, pointIndex, labelIndex, polygonIndex, iterationTableIndex) {
    dragModule.beginDrag(
      viewerEnv,
      pointerId,
      position,
      pointIndex,
      labelIndex,
      polygonIndex,
      iterationTableIndex,
    );
  }

  function updateDraggedPoint(world) {
    dragModule.updateDraggedPoint(viewerEnv, world);
  }

  function updateDraggedLabel(world) {
    dragModule.updateDraggedLabel(viewerEnv, world);
  }

  function updateDraggedIterationTable(position) {
    dragModule.updateDraggedIterationTable(viewerEnv, position);
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
    currentHotspotFlashes: () => hotspotFlashesState.val,
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
      return buildDebugGraph(sourceScene);
    },
    dumpJson() {
      console.log(buildDebugJson());
    },
    dumpGraph() {
      console.log(buildDebugGraph(sourceScene));
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
    renderOverlays();
    return 0;
  });

  canvas.addEventListener("pointerdown", (event) => {
    const position = sceneModule.getCanvasCoords(viewerEnv, event);
    const pointIndex = findHitPoint(position.x, position.y);
    const iterationTableIndex =
      pointIndex === null
        ? findHitIterationTable(position.x, position.y)
        : null;
    const labelIndex = pointIndex === null && iterationTableIndex === null
      ? findHitLabel(position.x, position.y)
      : null;
    const polygonIndex = pointIndex === null && iterationTableIndex === null && labelIndex === null
      ? findHitPolygon(position.x, position.y)
      : null;
    beginDrag(event.pointerId, position, pointIndex, labelIndex, polygonIndex, iterationTableIndex);
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
    } else if (dragState.val.mode === "iteration-table") {
      updateDraggedIterationTable(position);
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
