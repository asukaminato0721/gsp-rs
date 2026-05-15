// @ts-check

(function() {
  const modules = /** @type {Partial<ViewerModules> & { render: ViewerRenderModule }} */ (
    window.GspViewerModules || (window.GspViewerModules = {})
  );

  /**
   * @param {ViewerEnv} env
   * @param {SceneIterationTableJson} table
   */
  modules.render.iterationTableBounds = function iterationTableBounds(env, table) {
    if (table.visible === false || !Array.isArray(table.rows) || table.rows.length === 0) {
      return null;
    }
    const columns = Array.isArray(table.columns) && table.columns.length > 0
      ? table.columns
      : [{ exprLabel: table.exprLabel }];
    const header = ["n", ...columns.map((/** @type {{ exprLabel: string }} */ column) => column.exprLabel)];
    const body = table.rows.map((/** @type {{ index: number; value?: number; values?: number[] }} */ row) => {
      const values = Array.isArray(row.values) ? row.values : [row.value ?? Number.NaN];
      return [String(row.index), ...values.map((value) => env.formatNumber(value))];
    });
    const rows = [header, ...body];
    /** @type {number[]} */
    const colWidths = Array.from({ length: header.length }, () => 0);
    rows.forEach((/** @type {string[]} */ row) => {
      row.forEach((/** @type {string} */ cell, /** @type {number} */ index) => {
        const width = colWidths[index];
        if (typeof width !== "number") return;
        colWidths[index] = Math.max(width, env.measureText(cell, 18) + 18);
      });
    });
    const rowHeight = 28;
    const width = colWidths.reduce((sum, colWidth) => sum + colWidth, 0);
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
    const tables = env.currentScene().iterationTables || [];
    for (let index = tables.length - 1; index >= 0; index -= 1) {
      const table = tables[index];
      if (!table) continue;
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

    tables.forEach((table, index) => {
      if (table.visible === false || !Array.isArray(table.rows) || table.rows.length === 0) return;
      const bounds = modules.render.iterationTableBounds(env, table);
      if (!bounds) return;
      const { rows, colWidths, rowHeight, width, height, left, top } = bounds;

      const group = modules.render.appendSceneElement(env, "g", {
        transform: `translate(${left} ${top})`,
      }, null, { category: "iterationTables", index });
      group.append(env.createSvgElement("rect", {
        x: 0,
        y: 0,
        width,
        height,
        fill: "rgba(255,255,255,0.92)",
        stroke: env.rgba([32, 32, 32, 255]),
        "stroke-width": 1,
      }));
      let columnX = 0;
      for (let columnIndex = 0; columnIndex < colWidths.length - 1; columnIndex += 1) {
        columnX += colWidths[columnIndex];
        group.append(env.createSvgElement("line", {
          x1: columnX,
          y1: 0,
          x2: columnX,
          y2: height,
          stroke: env.rgba([32, 32, 32, 255]),
          "stroke-width": 1,
        }));
      }
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
          if (typeof cellWidth !== "number") return;
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
    });
  };
})();
