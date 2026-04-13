// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  /**
   * @param {ViewerEnv} env
   * @param {number | null} pointIndex
   * @returns {"pan" | "origin-pan"}
   */
  function dragModeFor(env, pointIndex) {
    return pointIndex !== null && env.currentScene().graphMode && env.isOriginPointIndex(pointIndex)
      ? "origin-pan"
      : "pan";
  }

  /**
   * @param {ViewerEnv} env
   * @param {number} pointerId
   * @param {Point} position
   * @param {number | null} pointIndex
   */
  function beginDrag(env, pointerId, position, pointIndex) {
    env.dragState.val = {
      pointerId,
      mode: dragModeFor(env, pointIndex),
      pointIndex: null,
      labelIndex: null,
      polygonIndex: null,
      iterationTableIndex: null,
      imageIndex: null,
      lastX: position.x,
      lastY: position.y,
    };
    env.hoverPointIndex.val = null;
    env.canvas.classList.add("is-dragging");
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} position
   */
  function panFromPointerDelta(env, position) {
    const worldNow = env.toWorld(position.x, position.y);
    const worldLast = env.toWorld(env.dragState.val.lastX, env.dragState.val.lastY);
    env.view.centerX -= worldNow.x - worldLast.x;
    env.view.centerY -= worldNow.y - worldLast.y;
  }

  /** @returns {void} */
  function noop() {}

  modules.drag = {
    dragModeFor,
    beginDrag,
    updateDraggedPoint: noop,
    updateDraggedLabel: noop,
    updateDraggedImage: noop,
    updateDraggedPolygon: noop,
    updateDraggedIterationTable: noop,
    panFromPointerDelta,
  };
})();
