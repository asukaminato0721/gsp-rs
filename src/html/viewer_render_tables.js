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
    env.ctx.save();
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    const header = ["n", table.exprLabel];
    const body = table.rows.map((/** @type {{ index: number; value: number }} */ row) => [String(row.index), env.formatNumber(row.value)]);
    const rows = [header, ...body];
    const colWidths = [0, 0];
    rows.forEach((/** @type {string[]} */ row) => {
      row.forEach((/** @type {string} */ cell, /** @type {number} */ index) => {
        colWidths[index] = Math.max(colWidths[index], env.ctx.measureText(cell).width + 18);
      });
    });
    env.ctx.restore();
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

    env.ctx.save();
    env.ctx.font = "18px \"Noto Sans\", \"Segoe UI\", sans-serif";
    env.ctx.textAlign = "center";
    env.ctx.textBaseline = "middle";
    env.ctx.strokeStyle = env.rgba([32, 32, 32, 255]);
    env.ctx.fillStyle = "rgba(255,255,255,0.92)";
    env.ctx.lineWidth = 1;

    for (const table of tables) {
      if (table.visible === false || !Array.isArray(table.rows) || table.rows.length === 0) continue;
      const bounds = modules.render.iterationTableBounds(env, table);
      if (!bounds) continue;
      const { rows, colWidths, rowHeight, width, height, left, top } = bounds;

      env.ctx.fillRect(left, top, width, height);
      env.ctx.strokeRect(left, top, width, height);
      env.ctx.beginPath();
      env.ctx.moveTo(left + colWidths[0], top);
      env.ctx.lineTo(left + colWidths[0], top + height);
      for (let index = 1; index < rows.length; index += 1) {
        const y = top + rowHeight * index;
        env.ctx.moveTo(left, y);
        env.ctx.lineTo(left + width, y);
      }
      env.ctx.stroke();

      rows.forEach((row, rowIndex) => {
        let x = left;
        row.forEach((/** @type {string} */ cell, /** @type {number} */ colIndex) => {
          const cellWidth = colWidths[colIndex];
          env.ctx.fillStyle = env.rgba([32, 32, 32, 255]);
          env.ctx.fillText(cell, x + cellWidth / 2, top + rowHeight * rowIndex + rowHeight / 2);
          x += cellWidth;
        });
      });
    }
    env.ctx.restore();
  };
})();
