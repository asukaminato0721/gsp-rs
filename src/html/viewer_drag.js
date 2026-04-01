(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function dragModeFor(env, pointIndex, labelIndex) {
    if (pointIndex !== null) {
      const point = env.currentScene().points[pointIndex];
      if (point?.binding?.kind === "coordinate") {
        return "pan";
      }
      return env.currentScene().graphMode && env.isOriginPointIndex(pointIndex) ? "origin-pan" : "point";
    }
    return labelIndex !== null ? "label" : "pan";
  }

  function beginDrag(env, pointerId, position, pointIndex, labelIndex) {
    env.dragState.val = {
      pointerId,
      mode: dragModeFor(env, pointIndex, labelIndex),
      pointIndex,
      labelIndex,
      lastX: position.x,
      lastY: position.y,
    };
    env.hoverPointIndex.val = pointIndex;
    env.canvas.classList.add("is-dragging");
  }

  function updateDraggedPoint(env, world) {
    env.updateScene((draft) => {
      const point = draft.points[env.dragState.val.pointIndex];
      if (point.constraint && point.constraint.kind === "offset") {
        const originPoint = draft.points[point.constraint.originIndex];
        if (originPoint && !originPoint.constraint) {
          originPoint.x = world.x - point.constraint.dx;
          originPoint.y = world.y - point.constraint.dy;
        } else {
          const origin = env.resolveScenePoint(point.constraint.originIndex);
          point.constraint.dx = world.x - origin.x;
          point.constraint.dy = world.y - origin.y;
        }
      } else if (point.constraint && point.constraint.kind === "segment") {
        const start = env.resolveScenePoint(point.constraint.startIndex);
        const end = env.resolveScenePoint(point.constraint.endIndex);
        const projection = window.GspViewerModules.scene.projectToSegment(world, start, end);
        if (projection) {
          point.constraint.t = projection.t;
        }
      } else if (point.constraint && point.constraint.kind === "polyline") {
        const count = point.constraint.points.length;
        let bestSegmentIndex = point.constraint.segmentIndex;
        let bestT = point.constraint.t;
        let bestDistanceSquared = Number.POSITIVE_INFINITY;
        for (let segmentIndex = 0; segmentIndex < count - 1; segmentIndex += 1) {
          const start = point.constraint.points[segmentIndex];
          const end = point.constraint.points[segmentIndex + 1];
          const projection = window.GspViewerModules.scene.projectToSegment(world, start, end);
          if (!projection) {
            continue;
          }
          if (projection.distanceSquared < bestDistanceSquared) {
            bestDistanceSquared = projection.distanceSquared;
            bestSegmentIndex = segmentIndex;
            bestT = projection.t;
          }
        }
        point.constraint.segmentIndex = bestSegmentIndex;
        point.constraint.t = bestT;
      } else if (point.constraint && point.constraint.kind === "polygon-boundary") {
        const count = point.constraint.vertexIndices.length;
        let bestEdgeIndex = point.constraint.edgeIndex;
        let bestT = point.constraint.t;
        let bestDistanceSquared = Number.POSITIVE_INFINITY;
        for (let edgeIndex = 0; edgeIndex < count; edgeIndex += 1) {
          const start = env.resolveScenePoint(point.constraint.vertexIndices[edgeIndex]);
          const end = env.resolveScenePoint(point.constraint.vertexIndices[(edgeIndex + 1) % count]);
          const projection = window.GspViewerModules.scene.projectToSegment(world, start, end);
          if (!projection) {
            continue;
          }
          if (projection.distanceSquared < bestDistanceSquared) {
            bestDistanceSquared = projection.distanceSquared;
            bestEdgeIndex = edgeIndex;
            bestT = projection.t;
          }
        }
        point.constraint.edgeIndex = bestEdgeIndex;
        point.constraint.t = bestT;
      } else if (point.constraint && point.constraint.kind === "circle") {
        const center = env.resolveScenePoint(point.constraint.centerIndex);
        const dx = world.x - center.x;
        const dy = world.y - center.y;
        const length = Math.hypot(dx, dy);
        if (length > 1e-9) {
          point.constraint.unitX = dx / length;
          point.constraint.unitY = dy / length;
        }
      } else {
        point.x = world.x;
        point.y = world.y;
      }
    });
    env.hoverPointIndex.val = env.dragState.val.pointIndex;
  }

  function updateDraggedLabel(env, position) {
    env.updateScene((draft) => {
      const label = draft.labels[env.dragState.val.labelIndex];
      if (label.screenSpace) {
        label.anchor.x = position.x;
        label.anchor.y = position.y;
      } else if (typeof label.anchor.pointIndex === "number" || typeof label.anchor.lineIndex === "number") {
        const base = env.resolveAnchorBase(label.anchor);
        const world = env.toWorld(position.x, position.y);
        label.anchor.dx = world.x - base.x;
        label.anchor.dy = world.y - base.y;
      } else {
        const world = env.toWorld(position.x, position.y);
        label.anchor.x = world.x;
        label.anchor.y = world.y;
      }
    });
  }

  function panFromPointerDelta(env, position) {
    const worldNow = env.toWorld(position.x, position.y);
    const worldLast = env.toWorld(env.dragState.val.lastX, env.dragState.val.lastY);
    env.view.centerX -= worldNow.x - worldLast.x;
    env.view.centerY -= worldNow.y - worldLast.y;
  }

  modules.drag = {
    dragModeFor,
    beginDrag,
    updateDraggedPoint,
    updateDraggedLabel,
    panFromPointerDelta,
  };
})();
