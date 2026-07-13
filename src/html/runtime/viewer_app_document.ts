(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function isRecord(value: unknown): value is Record<string, unknown> {
    return !!value && typeof value === "object" && !Array.isArray(value);
  }

  function isFiniteNumber(value: unknown): value is number {
    return typeof value === "number" && Number.isFinite(value);
  }

  function isPoint(value: unknown) {
    return isRecord(value) && isFiniteNumber(value.x) && isFiniteNumber(value.y);
  }

  function requireRecordArray(scene: Record<string, unknown>, field: string) {
    const value = scene[field];
    if (!Array.isArray(value) || !value.every(isRecord)) {
      throw new Error(`invalid scene-data payload: ${field} must be an object array`);
    }
    return value;
  }

  function assertPointArray(value: unknown, field: string) {
    if (!Array.isArray(value) || !value.every(isPoint)) {
      throw new Error(`invalid scene-data payload: ${field} must be a point array`);
    }
  }

  function assertSceneData(value: unknown): asserts value is SceneData {
    if (!isRecord(value) || !isFiniteNumber(value.width) || !isFiniteNumber(value.height)) {
      throw new Error("invalid scene-data payload: missing numeric width or height");
    }
    if (!isRecord(value.bounds)
      || ![value.bounds.minX, value.bounds.maxX, value.bounds.minY, value.bounds.maxY].every(isFiniteNumber)) {
      throw new Error("invalid scene-data payload: invalid bounds");
    }
    for (const field of ["graphMode", "piMode", "savedViewport", "yUp"] as const) {
      if (typeof value[field] !== "boolean") {
        throw new Error(`invalid scene-data payload: ${field} must be boolean`);
      }
    }
    if (value.origin !== null && !isPoint(value.origin)) {
      throw new Error("invalid scene-data payload: origin must be a point or null");
    }
    const lines = requireRecordArray(value, "lines");
    const polygons = requireRecordArray(value, "polygons");
    const circles = requireRecordArray(value, "circles");
    const arcs = requireRecordArray(value, "arcs");
    const labels = requireRecordArray(value, "labels");
    const points = requireRecordArray(value, "points");
    lines.forEach((line, index) => assertPointArray(line.points, `lines[${index}].points`));
    polygons.forEach((polygon, index) => assertPointArray(polygon.points, `polygons[${index}].points`));
    arcs.forEach((arc, index) => assertPointArray(arc.points, `arcs[${index}].points`));
    circles.forEach((circle, index) => {
      if (!isPoint(circle.center) || !isPoint(circle.radiusPoint)) {
        throw new Error(`invalid scene-data payload: circles[${index}] has invalid endpoints`);
      }
    });
    labels.forEach((label, index) => {
      if (!isPoint(label.anchor) || typeof label.text !== "string") {
        throw new Error(`invalid scene-data payload: labels[${index}] is invalid`);
      }
    });
    points.forEach((point, index) => {
      if (!isFiniteNumber(point.x) || !isFiniteNumber(point.y)) {
        throw new Error(`invalid scene-data payload: points[${index}] is invalid`);
      }
    });
    for (const field of [
      "images", "pointIterations", "circleIterations", "lineIterations",
      "polygonIterations", "labelIterations", "iterationTables", "buttons",
      "parameters", "functions", "functionDefinitions",
    ]) {
      requireRecordArray(value, field);
    }
  }

  function isDocumentSceneData(data: unknown): data is { kind: "gsp-document"; pages: unknown[] } {
    return isRecord(data) && data.kind === "gsp-document" && Array.isArray(data.pages);
  }

  function assertDocumentSceneData(value: unknown): asserts value is DocumentSceneData {
    if (!isDocumentSceneData(value) || value.pages.length === 0) {
      throw new Error("invalid scene-data payload: document pages must be non-empty");
    }
    value.pages.forEach((page, index) => {
      if (!isRecord(page) || !isFiniteNumber(page.index) || typeof page.title !== "string") {
        throw new Error(`invalid scene-data payload: pages[${index}] is invalid`);
      }
      assertSceneData(page.scene);
    });
  }

  function activeDocumentPageIndex(pages: DocumentScenePage[]) {
    const match = /^#page-(\d+)$/.exec(window.location.hash);
    const index = match ? Number(match[1]) - 1 : 0;
    return Math.min(Math.max(Number.isFinite(index) ? index : 0, 0), pages.length - 1);
  }

  function readSceneData(element: HTMLElement | null) {
    if (!element?.textContent) {
      throw new Error("missing scene-data payload");
    }
    const raw: unknown = JSON.parse(element.textContent);
    let pages: DocumentScenePage[] | null;
    let sourceScene: SceneData;
    if (isDocumentSceneData(raw)) {
      assertDocumentSceneData(raw);
      pages = raw.pages;
      const activePageIndex = activeDocumentPageIndex(pages);
      sourceScene = pages[activePageIndex].scene;
      return { raw, pages, activePageIndex, sourceScene };
    } else {
      assertSceneData(raw);
      pages = null;
      sourceScene = raw;
    }
    return { raw, pages, activePageIndex: 0, sourceScene };
  }

  function installPageNavigation(
    pages: DocumentScenePage[] | null,
    activePageIndex: number,
    buttons: HTMLButtonElement[],
  ) {
    const activate = (index: number) => {
      if (!pages || index === activePageIndex || index < 0 || index >= pages.length) return;
      window.location.hash = `page-${index + 1}`;
      window.location.reload();
    };
    buttons.forEach((button) => {
      const index = Number(button.dataset.pageIndex);
      const selected = index === activePageIndex;
      button.setAttribute("aria-selected", selected ? "true" : "false");
      button.classList.toggle("is-active", selected);
      button.addEventListener("click", () => activate(index));
    });
    window.addEventListener("hashchange", () => {
      if (pages) window.location.reload();
    });
  }

  modules.appDocument = { readSceneData, installPageNavigation };
})();
