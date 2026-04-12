// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {ViewerEnv} env
   * @param {string} text
   */
  modules.render.labelMetrics = function labelMetrics(env, text) {
    const lines = text.split("\n");
    const width = lines.reduce((best, line) => Math.max(best, env.ctx.measureText(line).width), 0);
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
    env.ctx.save();
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.textBaseline = "top";
    const bounds = modules.render.labelBounds(env, label);
    if (!bounds) {
      env.ctx.restore();
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
          left: bounds.left + 4 + env.ctx.measureText(prefix).width,
          top: bounds.top + hotspot.line * 22,
          width: Math.max(1, env.ctx.measureText(text).width),
          height: 22,
          action: hotspot.action,
        };
      })
      .filter(Boolean);
    env.ctx.restore();
    return rects;
  };

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   */
  modules.render.findHitLabel = function findHitLabel(env, screenX, screenY) {
    env.ctx.save();
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.textBaseline = "top";
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
        env.ctx.restore();
        return index;
      }
    }
    env.ctx.restore();
    return null;
  };

  /** @param {ViewerEnv} env */
  modules.render.drawLabels = function drawLabels(env) {
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    for (const label of env.currentScene().labels) {
      if (label.visible === false || (label.richMarkup && !label.hotspots?.length)) continue;
      const bounds = modules.render.labelBounds(env, label);
      if (!bounds) continue;
      env.ctx.fillStyle = env.rgba(label.color);
      if (label.centeredOnAnchor) {
        env.ctx.textAlign = "center";
        env.ctx.textBaseline = "middle";
        const midOffset = (bounds.lines.length - 1) / 2;
        bounds.lines.forEach((line, index) => {
          env.ctx.fillText(line, bounds.screen.x, bounds.screen.y + (index - midOffset) * 22);
        });
      } else {
        env.ctx.textAlign = "left";
        env.ctx.textBaseline = "top";
        bounds.lines.forEach((line, index) => {
          env.ctx.fillText(line, bounds.left + 4, bounds.top + index * 22);
        });
      }
    }
    env.ctx.textAlign = "left";
    env.ctx.textBaseline = "alphabetic";
  };
})();
