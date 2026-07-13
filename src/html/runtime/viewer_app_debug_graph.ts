(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {} as ViewerModules);

  function createDebugGraphRuntime({
    formatNumber,
  }: {
    formatNumber: (value: number) => string;
  }) {
    type DebugSummaryEntity = {
      text?: string;
      name?: string;
      kind?: string;
      visible?: boolean;
      depth?: number;
      edgeCount?: number;
      parameterName?: string | null;
      anchor?: PointHandle;
      screenSpace?: boolean;
      x?: number;
      y?: number;
    };
    function formatReference(key: string, value: number) {
      if (!Number.isInteger(value)) return null;
      switch (key) {
        case "buttonIndices": return `buttons[${value}]`;
        case "circleIndices":
        case "circleIndex": return `circles[${value}]`;
        case "lineIndices":
        case "lineIndex": return `lines[${value}]`;
        case "polygonIndices":
        case "polygonIndex": return `polygons[${value}]`;
        case "seedLabelIndex":
        case "labelIndex": return `labels[${value}]`;
        case "functionKey": return `functions[${value}]`;
        case "segmentIndex": return null;
        default:
          return new Set([
            "pointIndex", "targetPointIndex", "pointSeedIndex", "seedIndex",
            "sourceIndex", "centerIndex", "originIndex", "radiusIndex",
            "startIndex", "endIndex", "midIndex", "throughIndex",
            "vertexIndex", "lineStartIndex", "lineEndIndex",
          ]).has(key) ? `points[${value}]` : null;
      }
    }

    function collectReferenceTokens(value: unknown) {
      const refs: string[] = [];
      function visit(node: unknown) {
        if (!node || typeof node !== "object") return;
        if (Array.isArray(node)) {
          node.forEach(visit);
          return;
        }
        Object.entries(node).forEach(([key, child]) => {
          if (typeof child === "number") {
            const ref = formatReference(key, child);
            if (ref) refs.push(ref);
            return;
          }
          if (Array.isArray(child)) {
            child.forEach((item) => {
              if (typeof item === "number") {
                const ref = formatReference(key, item);
                if (ref) refs.push(ref);
              }
            });
          }
          visit(child);
        });
      }
      visit(value);
      return [...new Set(refs)];
    }

    function summarizeDebugEntity(entity: unknown) {
      const item = (entity ?? {}) as DebugSummaryEntity;
      const parts: string[] = [];
      if (typeof item.text === "string") parts.push(JSON.stringify(item.text));
      if (typeof item.name === "string") parts.push(`name=${item.name}`);
      if (typeof item.kind === "string") parts.push(`kind=${item.kind}`);
      if (typeof item.visible === "boolean") parts.push(item.visible ? "visible" : "hidden");
      if (typeof item.depth === "number") parts.push(`depth=${item.depth}`);
      if (typeof item.edgeCount === "number") parts.push(`edges=${item.edgeCount}`);
      if (typeof item.parameterName === "string" && item.parameterName.length > 0) {
        parts.push(`param=${item.parameterName}`);
      }
      if (item.anchor && typeof item.anchor === "object") {
        if (typeof item.anchor.x === "number" && typeof item.anchor.y === "number") {
          parts.push(`anchor @ (${formatNumber(item.anchor.x)}, ${formatNumber(item.anchor.y)})`);
        }
        if (item.screenSpace === true) parts.push("screenSpace");
      }
      if (typeof item.x === "number" && typeof item.y === "number" && !item.kind) {
        parts.push(`@ (${formatNumber(item.x)}, ${formatNumber(item.y)})`);
      }
      return parts.join(" ");
    }

    function appendGraphSection(lines: string[], title: string, itemLabel: string, items: unknown[]) {
      lines.push(`${title} (${items.length})`);
      items.forEach((item, index) => {
        const summary = summarizeDebugEntity(item);
        const refs = collectReferenceTokens(item);
        lines.push(`  ${itemLabel}[${index}]${summary ? ` ${summary}` : ""}`);
        if (refs.length > 0) lines.push(`    -> ${refs.join(", ")}`);
      });
    }

    function buildDebugGraph(scene: ViewerSceneData) {
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

    return { collectReferenceTokens, summarizeDebugEntity, buildDebugGraph };
  }

  modules.appDebugGraph = { createDebugGraphRuntime };
})();
