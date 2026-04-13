// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {ViewerEnv} env
   * @param {string} text
   */
  modules.render.labelMetrics = function labelMetrics(env, text) {
    const lines = text.split("\n");
    const width = lines.reduce((best, line) => Math.max(best, env.measureText(line, 18)), 0);
    return {
      lines,
      width,
      height: lines.length * 22,
    };
  };

  /**
   * @param {ViewerEnv} env
   * @param {SceneLabelJson} label
   */
  modules.render.labelBounds = function labelBounds(env, label) {
    const worldAnchor = label.screenSpace
      ? { x: /** @type {Point} */ (label.anchor).x, y: /** @type {Point} */ (label.anchor).y }
      : label.binding?.kind === "point-bound-expression-value"
        ? (() => {
            const point = env.resolveScenePoint(label.binding.pointIndex);
            return point
              ? {
                  x: point.x + label.binding.anchorDx,
                  y: point.y + label.binding.anchorDy,
                }
              : null;
          })()
        : env.resolvePoint(label.anchor);
    if (!worldAnchor) return null;
    const screen = label.screenSpace ? worldAnchor : env.toScreen(worldAnchor);
    const metrics = modules.render.labelMetrics(env, label.text);
    if (label.centeredOnAnchor) {
      return {
        screen,
        lines: metrics.lines,
        width: metrics.width,
        height: metrics.height,
        left: screen.x - metrics.width / 2,
        top: screen.y - metrics.height / 2,
      };
    }
    return {
      screen,
      lines: metrics.lines,
      width: metrics.width,
      height: metrics.height,
      left: screen.x + 2,
      top: screen.y - 14,
    };
  };

  /**
   * @param {ViewerEnv} env
   * @param {SceneLabelJson} label
   */
  modules.render.labelHotspotRects = function labelHotspotRects(env, label) {
    if (!label.hotspots?.length) {
      return [];
    }
    const bounds = modules.render.labelBounds(env, label);
    if (!bounds) {
      return [];
    }
    const rects = label.hotspots
      .map((/** @type {RuntimeLabelHotspotJson} */ hotspot) => {
        const line = bounds.lines[hotspot.line];
        if (typeof line !== "string") return null;
        const glyphs = Array.from(line);
        const start = Math.max(0, Math.min(glyphs.length, hotspot.start));
        const end = Math.max(start, Math.min(glyphs.length, hotspot.end));
        const prefix = glyphs.slice(0, start).join("");
        const text = glyphs.slice(start, end).join("");
        if (!text) return null;
        return {
          line: hotspot.line,
          start,
          end,
          text: hotspot.text || text,
          left: bounds.left + 4 + env.measureText(prefix, 18),
          top: bounds.top + hotspot.line * 22,
          width: Math.max(1, env.measureText(text, 18)),
          height: 22,
          action: hotspot.action,
        };
      })
      .filter(Boolean);
    return rects;
  };

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   */
  modules.render.findHitLabel = function findHitLabel(env, screenX, screenY) {
    for (let index = env.currentScene().labels.length - 1; index >= 0; index -= 1) {
      const label = env.currentScene().labels[index];
      if (label.visible === false) continue;
      const bounds = modules.render.labelBounds(env, label);
      if (!bounds) continue;
      if (
        screenX >= bounds.left &&
        screenX <= bounds.left + bounds.width + 8 &&
        screenY >= bounds.top &&
        screenY <= bounds.top + bounds.height
      ) {
        return index;
      }
    }
    return null;
  };

  /** @param {ViewerEnv} env */
  modules.render.drawLabels = function drawLabels(env) {
    for (const label of env.currentScene().labels) {
      if (label.visible === false || (label.richMarkup && !label.hotspots?.length)) continue;
      const bounds = modules.render.labelBounds(env, label);
      if (!bounds) continue;
      const group = modules.render.appendSceneElement(env, "g", {
        fill: env.rgba(label.color),
        "font-size": 18,
        "font-family": "\"Noto Sans\", \"Segoe UI\", sans-serif",
      });
      if (label.centeredOnAnchor) {
        const midOffset = (bounds.lines.length - 1) / 2;
        bounds.lines.forEach((line, index) => {
          const text = env.createSvgElement("text", {
            x: bounds.screen.x,
            y: bounds.screen.y + (index - midOffset) * 22,
            "text-anchor": "middle",
            "dominant-baseline": "middle",
          });
          text.textContent = line;
          group.append(text);
        });
      } else {
        bounds.lines.forEach((line, index) => {
          const text = env.createSvgElement("text", {
            x: bounds.left + 4,
            y: bounds.top + index * 22,
            "text-anchor": "start",
            "dominant-baseline": "hanging",
          });
          text.textContent = line;
          group.append(text);
        });
      }
    }
  };
})();
