// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {ViewerEnv} env
   * @param {SceneIterationTableJson} table
   */
  modules.render.iterationTableBounds = function iterationTableBounds(env, table) {
    if (table.visible === false || !Array.isArray(table.rows) || table.rows.length === 0) {
      return null;
    }
    const header = ["n", table.exprLabel];
    const body = table.rows.map((/** @type {{ index: number; value: number }} */ row) => [String(row.index), env.formatNumber(row.value)]);
    const rows = [header, ...body];
    const colWidths = [0, 0];
    rows.forEach((/** @type {string[]} */ row) => {
      row.forEach((/** @type {string} */ cell, /** @type {number} */ index) => {
        colWidths[index] = Math.max(colWidths[index], env.measureText(cell, 18) + 18);
      });
    });
    const rowHeight = 28;
    const width = colWidths[0] + colWidths[1];
    const height = rowHeight * rows.length;
    return {
      left: table.x,
      top: table.y - height,
      width,
      height,
      colWidths,
      rowHeight,
      rows,
    };
  };

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   */
  modules.render.findHitIterationTable = function findHitIterationTable(env, screenX, screenY) {
    for (let index = (env.currentScene().iterationTables || []).length - 1; index >= 0; index -= 1) {
      const table = env.currentScene().iterationTables[index];
      const bounds = modules.render.iterationTableBounds(env, table);
      if (!bounds) continue;
      if (
        screenX >= bounds.left &&
        screenX <= bounds.left + bounds.width &&
        screenY >= bounds.top &&
        screenY <= bounds.top + bounds.height
      ) {
        return index;
      }
    }
    return null;
  };

  /** @param {ViewerEnv} env */
  modules.render.drawIterationTables = function drawIterationTables(env) {
    const tables = env.currentScene().iterationTables || [];
    if (!tables.length) return;

    for (const table of tables) {
      if (table.visible === false || !Array.isArray(table.rows) || table.rows.length === 0) continue;
      const bounds = modules.render.iterationTableBounds(env, table);
      if (!bounds) continue;
      const { rows, colWidths, rowHeight, width, height, left, top } = bounds;

      const group = modules.render.appendSceneElement(env, "g", {
        transform: `translate(${left} ${top})`,
      });
      group.append(env.createSvgElement("rect", {
        x: 0,
        y: 0,
        width,
        height,
        fill: "rgba(255,255,255,0.92)",
        stroke: env.rgba([32, 32, 32, 255]),
        "stroke-width": 1,
      }));
      group.append(env.createSvgElement("line", {
        x1: colWidths[0],
        y1: 0,
        x2: colWidths[0],
        y2: height,
        stroke: env.rgba([32, 32, 32, 255]),
        "stroke-width": 1,
      }));
      for (let index = 1; index < rows.length; index += 1) {
        const y = rowHeight * index;
        group.append(env.createSvgElement("line", {
          x1: 0,
          y1: y,
          x2: width,
          y2: y,
          stroke: env.rgba([32, 32, 32, 255]),
          "stroke-width": 1,
        }));
      }

      rows.forEach((row, rowIndex) => {
        let x = 0;
        row.forEach((/** @type {string} */ cell, /** @type {number} */ colIndex) => {
          const cellWidth = colWidths[colIndex];
          const text = env.createSvgElement("text", {
            x: x + cellWidth / 2,
            y: rowHeight * rowIndex + rowHeight / 2,
            fill: env.rgba([32, 32, 32, 255]),
            "font-size": 18,
            "font-family": "\"Noto Sans\", \"Segoe UI\", sans-serif",
            "text-anchor": "middle",
            "dominant-baseline": "middle",
          });
          text.textContent = cell;
          group.append(text);
          x += cellWidth;
        });
      });
    }
  };
})();
