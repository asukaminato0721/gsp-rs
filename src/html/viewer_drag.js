// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  /** @type {Set<string>} */
  const PAN_ONLY_POINT_BINDINGS = new Set([
    "graph-calibration",
    "midpoint",
    "coordinate",
    "coordinate-source",
    "coordinate-source-2d",
  ]);

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { pointIndex: number }>}
   */
  function hasPointIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "offset" }>}
   */
  function isOffsetConstraint(constraint) {
    return !!constraint && constraint.kind === "offset";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "segment" }>}
   */
  function isSegmentConstraint(constraint) {
    return !!constraint && constraint.kind === "segment";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "segment" | "line" | "ray" }>}
   */
  function isLineLikeConstraint(constraint) {
    return !!constraint
      && (constraint.kind === "segment" || constraint.kind === "line" || constraint.kind === "ray");
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "polyline" }>}
   */
  function isPolylineConstraint(constraint) {
    return !!constraint && constraint.kind === "polyline";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "polygon-boundary" }>}
   */
  function isPolygonBoundaryConstraint(constraint) {
    return !!constraint && constraint.kind === "polygon-boundary";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "circle" }>}
   */
  function isCircleConstraint(constraint) {
    return !!constraint && constraint.kind === "circle";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "circle-arc" }>}
   */
  function isCircleArcConstraint(constraint) {
    return !!constraint && constraint.kind === "circle-arc";
  }

  /**
   * @param {RuntimeScenePointJson["constraint"]} constraint
   * @returns {constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "arc" }>}
   */
  function isArcConstraint(constraint) {
    return !!constraint && constraint.kind === "arc";
  }

  /**
   * @param {RuntimePointRef} anchor
   * @returns {anchor is Extract<RuntimePointRef, { pointIndex: number } | { lineIndex: number }>}
   */
  function isBoundAnchor(anchor) {
    return !!anchor && typeof anchor === "object" && (
      ("pointIndex" in anchor && typeof anchor.pointIndex === "number")
      || ("lineIndex" in anchor && typeof anchor.lineIndex === "number")
    );
  }

  /** @typedef {(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) => void} DraggedPointConstraintUpdater */
  /** @type {Record<string, DraggedPointConstraintUpdater>} */
  const DRAGGED_POINT_CONSTRAINT_UPDATERS = {
    offset(env, draft, point, world) {
      const constraint = point.constraint;
      if (!isOffsetConstraint(constraint)) return;
      const originPoint = draft.points[constraint.originIndex];
      if (originPoint && !originPoint.constraint) {
        originPoint.x = world.x - constraint.dx;
        originPoint.y = world.y - constraint.dy;
        return;
      }
      const origin = env.resolveScenePoint(constraint.originIndex);
      constraint.dx = world.x - origin.x;
      constraint.dy = world.y - origin.y;
    },
    segment(env, _draft, point, world) {
      const constraint = point.constraint;
      if (!isLineLikeConstraint(constraint)) return;
      const start = env.resolveScenePoint(constraint.startIndex);
      const end = env.resolveScenePoint(constraint.endIndex);
      const projection = window.GspViewerModules.scene.projectToLineLike(
        world,
        start,
        end,
        constraint.kind,
      );
      if (projection) {
        constraint.t = projection.t;
      }
    },
    line(env, draft, point, world) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.segment(env, draft, point, world);
    },
    ray(env, draft, point, world) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.segment(env, draft, point, world);
    },
    polyline(env, _draft, point, world) {
      const constraint = point.constraint;
      if (!isPolylineConstraint(constraint)) return;
      const points = typeof constraint.functionKey === "number"
        ? window.GspViewerModules.scene.resolveLinePoints(
            env,
            env.currentScene().lines.find((/** @type {SceneLineJson} */ line) =>
              line?.binding?.kind === "arc-boundary" && line.binding.hostKey === constraint.functionKey
            ),
          ) || constraint.points
        : constraint.points;
      const count = points.length;
      let bestSegmentIndex = constraint.segmentIndex;
      let bestT = constraint.t;
      let bestDistanceSquared = Number.POSITIVE_INFINITY;
      for (let segmentIndex = 0; segmentIndex < count - 1; segmentIndex += 1) {
        const start = window.GspViewerModules.scene.resolvePoint(env, points[segmentIndex]);
        const end = window.GspViewerModules.scene.resolvePoint(env, points[segmentIndex + 1]);
        if (!start || !end) {
          continue;
        }
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
      constraint.segmentIndex = bestSegmentIndex;
      constraint.t = bestT;
    },
    "polygon-boundary"(env, _draft, point, world) {
      const constraint = point.constraint;
      if (!isPolygonBoundaryConstraint(constraint)) return;
      const count = constraint.vertexIndices.length;
      let bestEdgeIndex = constraint.edgeIndex;
      let bestT = constraint.t;
      let bestDistanceSquared = Number.POSITIVE_INFINITY;
      for (let edgeIndex = 0; edgeIndex < count; edgeIndex += 1) {
        const start = env.resolveScenePoint(constraint.vertexIndices[edgeIndex]);
        const end = env.resolveScenePoint(constraint.vertexIndices[(edgeIndex + 1) % count]);
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
      constraint.edgeIndex = bestEdgeIndex;
      constraint.t = bestT;
    },
    circle(env, _draft, point, world) {
      const constraint = point.constraint;
      if (!isCircleConstraint(constraint)) return;
      const center = env.resolveScenePoint(constraint.centerIndex);
      const dx = world.x - center.x;
      const dy = world.y - center.y;
      const length = Math.hypot(dx, dy);
      if (length > 1e-9) {
        constraint.unitX = dx / length;
        constraint.unitY = dy / length;
      }
    },
    "circle-arc"(env, _draft, point, world) {
      const constraint = point.constraint;
      if (!isCircleArcConstraint(constraint)) return;
      const center = env.resolveScenePoint(constraint.centerIndex);
      const start = env.resolveScenePoint(constraint.startIndex);
      const end = env.resolveScenePoint(constraint.endIndex);
      const projection = window.GspViewerModules.scene.projectToCircleArc(
        world,
        center,
        start,
        end,
        !!env.sourceScene.yUp,
      );
      if (projection) {
        constraint.t = projection.t;
      }
    },
    arc(env, _draft, point, world) {
      const constraint = point.constraint;
      if (!isArcConstraint(constraint)) return;
      const start = env.resolveScenePoint(constraint.startIndex);
      const mid = env.resolveScenePoint(constraint.midIndex);
      const end = env.resolveScenePoint(constraint.endIndex);
      const projection = window.GspViewerModules.scene.projectToThreePointArc(
        world,
        start,
        mid,
        end,
      );
      if (projection) {
        constraint.t = projection.t;
      }
    },
  };

  /**
   * @param {ViewerEnv} env
   * @param {number | null} pointIndex
   * @param {number | null} labelIndex
   * @param {number | null} polygonIndex
   * @param {number | null} iterationTableIndex
   * @param {number | null} imageIndex
   */
  function dragModeFor(env, pointIndex, labelIndex, polygonIndex, iterationTableIndex, imageIndex) {
    if (pointIndex !== null) {
      const point = env.currentScene().points[pointIndex];
      if (PAN_ONLY_POINT_BINDINGS.has(point?.binding?.kind)) {
        return "pan";
      }
      return env.currentScene().graphMode && env.isOriginPointIndex(pointIndex) ? "origin-pan" : "point";
    }
    if (imageIndex !== null) {
      return "image";
    }
    if (polygonIndex !== null) {
      return "polygon";
    }
    if (iterationTableIndex !== null) {
      return "iteration-table";
    }
    return labelIndex !== null ? "label" : "pan";
  }

  /**
   * @param {ViewerEnv} env
   * @param {number} pointerId
   * @param {Point} position
   * @param {number | null} pointIndex
   * @param {number | null} labelIndex
   * @param {number | null} polygonIndex
   * @param {number | null} iterationTableIndex
   * @param {number | null} imageIndex
   */
  function beginDrag(env, pointerId, position, pointIndex, labelIndex, polygonIndex, iterationTableIndex, imageIndex) {
    env.dragState.val = {
      pointerId,
      mode: dragModeFor(env, pointIndex, labelIndex, polygonIndex, iterationTableIndex, imageIndex),
      pointIndex,
      labelIndex,
      polygonIndex,
      iterationTableIndex,
      imageIndex,
      lastX: position.x,
      lastY: position.y,
    };
    env.hoverPointIndex.val = pointIndex;
    env.canvas.classList.add("is-dragging");
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} world
   */
  function updateDraggedPoint(env, world) {
    env.updateScene((/** @type {ViewerSceneData} */ draft) => {
      const point = draft.points[env.dragState.val.pointIndex];
      const constraintKind = point.constraint?.kind;
      const updateConstraint = typeof constraintKind === "string"
        ? DRAGGED_POINT_CONSTRAINT_UPDATERS[constraintKind]
        : null;
      if (updateConstraint) {
        updateConstraint(env, draft, point, world);
      } else {
        point.x = world.x;
        point.y = world.y;
      }
    });
    env.hoverPointIndex.val = env.dragState.val.pointIndex;
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} position
   */
  function updateDraggedLabel(env, position) {
    env.updateScene((/** @type {ViewerSceneData} */ draft) => {
      const label = draft.labels[env.dragState.val.labelIndex];
      const anchor = label.anchor;
      if (label.screenSpace) {
        anchor.x = position.x;
        anchor.y = position.y;
      } else if (isBoundAnchor(anchor)) {
        const base = env.resolveAnchorBase(anchor);
        const world = env.toWorld(position.x, position.y);
        anchor.dx = world.x - base.x;
        anchor.dy = world.y - base.y;
      } else {
        const world = env.toWorld(position.x, position.y);
        anchor.x = world.x;
        anchor.y = world.y;
      }
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} world
   */
  function updateDraggedPolygon(env, world) {
    const previous = env.toWorld(env.dragState.val.lastX, env.dragState.val.lastY);
    const dx = world.x - previous.x;
    const dy = world.y - previous.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return;
    env.updateScene((/** @type {ViewerSceneData} */ draft) => {
      const polygon = draft.polygons[env.dragState.val.polygonIndex];
      if (!polygon) return;
      polygon.points.forEach((/** @type {PointHandle} */ handle) => {
        if (!hasPointIndexHandle(handle)) return;
        const point = draft.points[handle.pointIndex];
        if (!point || point.constraint || point.binding) return;
        point.x += dx;
        point.y += dy;
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} position
   */
  function updateDraggedIterationTable(env, position) {
    env.updateScene((/** @type {ViewerSceneData} */ draft) => {
      const table = draft.iterationTables?.[env.dragState.val.iterationTableIndex];
      if (!table) return;
      table.x = position.x;
      table.y = position.y;
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {Point} position
   */
  function updateDraggedImage(env, position) {
    env.updateScene((draft) => {
      const image = draft.images?.[env.dragState.val.imageIndex];
      if (!image) return;
      const dxScreen = position.x - env.dragState.val.lastX;
      const dyScreen = position.y - env.dragState.val.lastY;
      if (Math.abs(dxScreen) <= 1e-9 && Math.abs(dyScreen) <= 1e-9) return;
      if (image.screenSpace) {
        image.topLeft.x += dxScreen;
        image.topLeft.y += dyScreen;
        image.bottomRight.x += dxScreen;
        image.bottomRight.y += dyScreen;
        return;
      }
      const worldNow = env.toWorld(position.x, position.y);
      const worldLast = env.toWorld(env.dragState.val.lastX, env.dragState.val.lastY);
      const dx = worldNow.x - worldLast.x;
      const dy = worldNow.y - worldLast.y;
      image.topLeft.x += dx;
      image.topLeft.y += dy;
      image.bottomRight.x += dx;
      image.bottomRight.y += dy;
    });
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

  modules.drag = {
    dragModeFor,
    beginDrag,
    updateDraggedPoint,
    updateDraggedLabel,
    updateDraggedImage,
    updateDraggedPolygon,
    updateDraggedIterationTable,
    panFromPointerDelta,
  };
})();
