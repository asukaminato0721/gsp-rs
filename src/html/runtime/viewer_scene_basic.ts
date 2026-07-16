(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  );
  const {
    lerpPoint,
  } = window.GspRuntimeCore;

  
  
  function hasPointIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { pointIndex: number }> {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  
  function hasLineIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { lineIndex: number }> {
    return !!handle && typeof handle === "object" && "lineIndex" in handle && typeof handle.lineIndex === "number";
  }

  
  function projectToSegment(point: Point, start: Point, end: Point) {
    return projectToLineLike(point, start, end, "segment");
  }

  
  function projectToLineLike(point: Point, start: Point, end: Point, kind: "segment" | "line" | "ray") {
    return window.GspRuntimeCore.projectToLineLike(point, start, end, kind);
  }

  
  function threePointArcGeometry(start: Point, mid: Point, end: Point) {
    return window.GspRuntimeCore.threePointArcGeometry(start, mid, end);
  }

  
  function circleArcControlPoints(center: Point, start: Point, end: Point, yUp: boolean) {
    const controls = window.GspRuntimeCore.circleArcControlPoints(center, start, end, yUp);
    return controls ? { start: controls[0], mid: controls[1], end: controls[2] } : null;
  }

  
  function pointOnThreePointArc(start: Point, mid: Point, end: Point, t: number) {
    return window.GspRuntimeCore.pointOnThreePointArc(start, mid, end, t, false);
  }

  
  function pointOnThreePointArcComplement(start: Point, mid: Point, end: Point, t: number) {
    return window.GspRuntimeCore.pointOnThreePointArc(start, mid, end, t, true);
  }

  
  function pointOnCircleArc(center: Point, start: Point, end: Point, t: number, yUp: boolean) {
    return window.GspRuntimeCore.pointOnCircleArc(center, start, end, t, yUp);
  }

  
  function projectToThreePointArc(point: Point, start: Point, mid: Point, end: Point) {
    return window.GspRuntimeCore.projectToThreePointArc(point, start, mid, end);
  }

  
  function projectToCircleArc(point: Point, center: Point, start: Point, end: Point, yUp: boolean) {
    return window.GspRuntimeCore.projectToCircleArc(point, center, start, end, yUp);
  }

  
  function getViewBounds(env: ViewerEnv) {
    const spanX = env.baseSpanX / env.view.zoom;
    const spanY = env.baseSpanY / env.view.zoom;
    return {
      minX: env.view.centerX - spanX / 2,
      maxX: env.view.centerX + spanX / 2,
      minY: env.view.centerY - spanY / 2,
      maxY: env.view.centerY + spanY / 2,
      spanX,
      spanY,
    };
  }

  
  function resolveScenePoint(env: ViewerEnv, index: number) {
    const point = env.currentScene().points[index];
    return point && Number.isFinite(point.x) && Number.isFinite(point.y) ? point : null;
  }

  
  function resolvePoint(env: ViewerEnv, handle: PointHandle) {
    if (hasPointIndexHandle(handle)) {
      const point = resolveScenePoint(env, handle.pointIndex);
      if (!point) return null;
      return {
        x: point.x + (handle.dx || 0),
        y: point.y + (handle.dy || 0),
      };
    }
    if (hasLineIndexHandle(handle)) {
      const points = resolveLinePoints(env, handle.lineIndex);
      if (!points || points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      const start = points[segmentIndex];
      const end = points[segmentIndex + 1];
      return {
        x: lerpPoint(start, end, t).x + (handle.dx || 0),
        y: lerpPoint(start, end, t).y + (handle.dy || 0),
      };
    }
    return  (handle);
  }

  
  function resolveAnchorBase(env: ViewerEnv, handle: PointHandle) {
    if (hasPointIndexHandle(handle)) {
      return resolveScenePoint(env, handle.pointIndex);
    }
    if (hasLineIndexHandle(handle)) {
      const points = resolveLinePoints(env, handle.lineIndex);
      if (!points || points.length < 2) {
        return { x: handle.x || 0, y: handle.y || 0 };
      }
      const segmentIndex = Math.max(0, Math.min(points.length - 2, handle.segmentIndex || 0));
      const t = typeof handle.t === "number" ? handle.t : 0.5;
      return lerpPoint(points[segmentIndex], points[segmentIndex + 1], t);
    }
    return  (handle);
  }

  
  function resolveLinePoints(env: ViewerEnv, lineOrIndex: SceneLineJson | number | null | undefined) {
    const line = typeof lineOrIndex === "number" ? env.currentScene().lines[lineOrIndex] : lineOrIndex;
    if (!line) return null;
    const points = line.points.map(( handle) => resolvePoint(env, handle));
    return points.every(Boolean) ? points : null;
  }

  
  function toScreen(env: ViewerEnv, point: Point) {
    const usableWidth = Math.max(1, env.sourceScene.width - env.margin * 2);
    const usableHeight = Math.max(1, env.sourceScene.height - env.margin * 2);
    const bounds = getViewBounds(env);
    const scale = Math.min(usableWidth / bounds.spanX, usableHeight / bounds.spanY);
    return {
      x: env.margin + (point.x - bounds.minX) * scale,
      y: env.sourceScene.yUp
        ? env.sourceScene.height - env.margin - (point.y - bounds.minY) * scale
        : env.margin + (point.y - bounds.minY) * scale,
      scale,
    };
  }

  
  function toWorld(env: ViewerEnv, screenX: number, screenY: number) {
    const usableWidth = Math.max(1, env.sourceScene.width - env.margin * 2);
    const usableHeight = Math.max(1, env.sourceScene.height - env.margin * 2);
    const bounds = getViewBounds(env);
    const scale = Math.min(usableWidth / bounds.spanX, usableHeight / bounds.spanY);
    return {
      x: bounds.minX + (screenX - env.margin) / scale,
      y: env.sourceScene.yUp
        ? bounds.minY + (env.sourceScene.height - env.margin - screenY) / scale
        : bounds.minY + (screenY - env.margin) / scale,
      scale,
    };
  }

  
  function getCanvasCoords(env: ViewerEnv, event: MouseEvent | PointerEvent | WheelEvent) {
    const rect = env.canvas.getBoundingClientRect();
    return {
      x: (event.clientX - rect.left) * (env.sourceScene.width / rect.width),
      y: (event.clientY - rect.top) * (env.sourceScene.height / rect.height),
    };
  }

  
  function appendGridElement(env: ViewerEnv, parent: Element, attrs: Record<string, string | number | boolean | null | undefined>) {
    const tag = String(attrs.tag);
    const nextAttrs = { ...attrs };
    delete nextAttrs.tag;
    const element = env.createSvgElement(tag, nextAttrs);
    parent.append(element);
    return element;
  }

  
  function appendGridLine(env: ViewerEnv, parent: Element, x1: number, y1: number, x2: number, y2: number, color: string) {
    appendGridElement(env, parent, {
      tag: "line",
      x1,
      y1,
      x2,
      y2,
      stroke: color,
      "stroke-width": 1,
      "shape-rendering": "crispEdges",
    });
  }

  
  function appendGridText(env: ViewerEnv, parent: Element, x: number, y: number, text: string, anchor: "start" | "middle" | "end") {
    const label = appendGridElement(env, parent, {
      tag: "text",
      x,
      y,
      fill: "rgb(20,20,20)",
      "font-size": 12,
      "font-family": "\"Noto Sans\", \"Segoe UI\", sans-serif",
      "text-anchor": anchor,
      "dominant-baseline": "middle",
    });
    label.textContent = text;
  }

  
  function chooseGridStep(span: number, targetLines: number) {
    const rough = Math.max(1e-6, span / Math.max(1, targetLines));
    const magnitude = 10 ** Math.floor(Math.log10(rough));
    const normalized = rough / magnitude;
    if (normalized <= 1) return magnitude;
    if (normalized <= 2) return magnitude * 2;
    if (normalized <= 5) return magnitude * 5;
    return magnitude * 10;
  }

  
  function drawGrid(env: ViewerEnv) {
    env.clearSvgChildren(env.gridLayer);
    if (!env.currentScene().graphMode) return;
    const gridLayer = env.gridLayer;
    const snapStroke = ( value) => Math.round(value) + 0.5;
    const bounds = getViewBounds(env);
    const spanX = bounds.maxX - bounds.minX;
    const spanY = bounds.maxY - bounds.minY;
    const xMinorStep = env.savedViewportMode ? 1 : chooseGridStep(spanX, 14);
    const xMajorStep = env.savedViewportMode ? 2 : chooseGridStep(spanX, 7);
    const yMinorStep = env.savedViewportMode ? 1 : chooseGridStep(spanY, 14);
    const yMajorStep = env.savedViewportMode ? 2 : chooseGridStep(spanY, 7);
    const minXIndex = Math.floor(bounds.minX / xMinorStep);
    const maxXIndex = Math.ceil(bounds.maxX / xMinorStep);
    const minYIndex = Math.floor(bounds.minY / yMinorStep);
    const maxYIndex = Math.ceil(bounds.maxY / yMinorStep);

    const xAxisY = bounds.minY <= 0 && 0 <= bounds.maxY
      ? toScreen(env, { x: bounds.minX, y: 0 }).y
      : env.sourceScene.height - 18;
    const yAxisX = bounds.minX <= 0 && 0 <= bounds.maxX
      ? toScreen(env, { x: 0, y: bounds.minY }).x
      : env.sourceScene.width / 2;

    for (let xIndex = minXIndex; xIndex <= maxXIndex; xIndex += 1) {
      const x = xIndex * xMinorStep;
      const screen = toScreen(env, { x, y: bounds.minY });
      const major = Math.abs((x / xMajorStep) - Math.round(x / xMajorStep)) < 1e-6;
      const stroke = Math.abs(x) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      appendGridLine(
        env,
        gridLayer,
        snapStroke(screen.x),
        0,
        snapStroke(screen.x),
        env.sourceScene.height,
        stroke,
      );
      if (bounds.minY <= 0 && 0 <= bounds.maxY) {
        appendGridLine(
          env,
          gridLayer,
          snapStroke(screen.x),
          snapStroke(xAxisY - (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.x),
          snapStroke(xAxisY + (Math.abs(x) < 1e-6 ? 6 : major ? 4 : 2)),
          "rgb(40,40,40)",
        );
      }
      if (major && Math.abs(x) >= 1e-6) {
        const label = env.formatAxisNumber(x);
        appendGridText(
          env,
          gridLayer,
          Math.round(screen.x),
          Math.round(Math.min(env.sourceScene.height - 4, xAxisY + 16)),
          label,
          "middle",
        );
      }
    }

    for (let yIndex = minYIndex; yIndex <= maxYIndex; yIndex += 1) {
      const y = yIndex * yMinorStep;
      const major = Math.abs((y / yMajorStep) - Math.round(y / yMajorStep)) < 1e-6;
      const screen = toScreen(env, { x: bounds.minX, y });
      const stroke = Math.abs(y) < 1e-6
        ? "rgb(40,40,40)"
        : major ? "rgb(200,200,200)" : "rgb(225,225,225)";
      appendGridLine(
        env,
        gridLayer,
        0,
        snapStroke(screen.y),
        env.sourceScene.width,
        snapStroke(screen.y),
        stroke,
      );
      if (bounds.minX <= 0 && 0 <= bounds.maxX) {
        appendGridLine(
          env,
          gridLayer,
          snapStroke(yAxisX - (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.y),
          snapStroke(yAxisX + (Math.abs(y) < 1e-6 ? 6 : major ? 4 : 2)),
          snapStroke(screen.y),
          "rgb(40,40,40)",
        );
      }
      if (major && Math.abs(y) >= 1e-6) {
        const label = env.formatAxisNumber(y);
        appendGridText(
          env,
          gridLayer,
          Math.round(yAxisX - env.measureText(label, 12) - 8),
          Math.round(screen.y - 6),
          label,
          "start",
        );
      }
    }

    const originHandle = env.currentScene().origin;
    if (originHandle) {
      const resolvedOrigin = resolvePoint(env, originHandle);
      if (!resolvedOrigin) return;
      const origin = toScreen(env, resolvedOrigin);
      appendGridElement(env, gridLayer, {
        tag: "circle",
        cx: origin.x,
        cy: origin.y,
        r: 3,
        fill: "rgba(255, 60, 40, 1)",
      });
    }
  }

  
  modules.scene = {
    _threePointArcGeometry: threePointArcGeometry,
    _circleArcControlPoints: circleArcControlPoints,
    _pointOnThreePointArcComplement: pointOnThreePointArcComplement,
    getViewBounds,
    resolveScenePoint,
    resolvePoint,
    resolveAnchorBase,
    resolveLinePoints,
    toScreen,
    toWorld,
    getCanvasCoords,
    chooseGridStep,
    lerpPoint,
    projectToSegment,
    projectToLineLike,
    pointOnCircleArc,
    projectToCircleArc,
    pointOnThreePointArc,
    projectToThreePointArc,
    drawGrid,
  } as unknown as ViewerSceneModule;
})();
