(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  ) as Partial<ViewerModules> & {
    scene: ViewerSceneModule;
    dynamics: ViewerDynamicsModule;
    geometry: ViewerGeometryModule;
  };
  
  const PAN_ONLY_POINT_BINDINGS = new Set<string>([
    "midpoint",
    "coordinate",
    "coordinate-source",
    "coordinate-source-2d",
  ]);

  
  function hasPointIndexHandle(handle: PointHandle): handle is Extract<PointHandle, { pointIndex: number }> {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  
  function dependencyRootsForDraggedPoint(env: ViewerEnv, pointIndex: number) {
    const point = env.currentScene().points?.[pointIndex];
    if (!point) {
      return [];
    }
    const rootId = modules.dynamics.sourcePointRootId;
    if (typeof rootId !== "function") {
      return [];
    }
    const roots = new Set<string>();
    const addPointRoots = (index: number) => {
      roots.add(rootId(index));
      const constraint = env.currentScene().points?.[index]?.constraint;
      if (isOffsetConstraint(constraint)) {
        roots.add(rootId(constraint.originIndex));
      }
    };
    addPointRoots(pointIndex);
    if (point.binding?.kind === "derived" && typeof point.binding.sourceIndex === "number") {
      addPointRoots(point.binding.sourceIndex);
    }
    return Array.from(roots);
  }

  
  function dependencyRootsForDraggedPolygon(env: ViewerEnv, polygonIndex: number) {
    const polygon = env.currentScene().polygons?.[polygonIndex];
    if (!polygon) {
      return [];
    }
    const rootId = modules.dynamics.sourcePointRootId;
    if (typeof rootId !== "function") {
      return [];
    }
    const roots = new Set<string>();
    polygon.points.forEach((handle: PointHandle) => {
      if (hasPointIndexHandle(handle)) {
        roots.add(rootId(handle.pointIndex));
      }
    });
    return Array.from(roots);
  }

  
  function isOffsetConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "offset" }> {
    return !!constraint && constraint.kind === "offset";
  }

  
  function isLineLikeConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "segment" | "line" | "line-constraint" | "ray" | "ray-constraint" }> {
    return !!constraint
      && (
        constraint.kind === "segment"
        || constraint.kind === "line"
        || constraint.kind === "line-constraint"
        || constraint.kind === "ray"
        || constraint.kind === "ray-constraint"
      );
  }

  
  function isPolylineConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is StrictUnion<RuntimePolylineConstraintJson> {
    return !!constraint && constraint.kind === "polyline";
  }

  
  function isPolygonBoundaryConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "polygon-boundary" }> {
    return !!constraint && constraint.kind === "polygon-boundary";
  }

  function isPolygonBoundaryParameterConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "polygon-boundary-parameter" }> {
    return !!constraint && constraint.kind === "polygon-boundary-parameter";
  }

  function isPolygonShapeBoundaryConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "polygon-shape-boundary" }> {
    return !!constraint && constraint.kind === "polygon-shape-boundary";
  }

  
  function isCircleConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "circle" | "circular-constraint" }> {
    return !!constraint && (constraint.kind === "circle" || constraint.kind === "circular-constraint");
  }

  
  function isCircleArcConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "circle-arc" }> {
    return !!constraint && constraint.kind === "circle-arc";
  }

  
  function isArcConstraint(
    constraint: RuntimeScenePointJson["constraint"],
  ): constraint is Extract<NonNullable<RuntimeScenePointJson["constraint"]>, { kind: "arc" }> {
    return !!constraint && constraint.kind === "arc";
  }

  
  function isBoundAnchor(
    anchor: RuntimePointRef,
  ): anchor is Extract<RuntimePointRef, { pointIndex: number }> | Extract<RuntimePointRef, { lineIndex: number }> {
    return !!anchor && typeof anchor === "object" && (
      ("pointIndex" in anchor && typeof anchor.pointIndex === "number")
      || ("lineIndex" in anchor && typeof anchor.lineIndex === "number")
    );
  }

  
  function isCoordinateAnchor(anchor: RuntimePointRef): anchor is Point {
    return !!anchor && typeof anchor === "object" && "x" in anchor && "y" in anchor;
  }

  type ConstraintUpdater = (
    env: ViewerEnv,
    draft: ViewerSceneData,
    point: RuntimeScenePointJson,
    world: Point,
  ) => void;

  const DRAGGED_POINT_CONSTRAINT_UPDATERS: Partial<
    Record<NonNullable<RuntimeScenePointJson["constraint"]>["kind"], ConstraintUpdater>
  > = {
    offset(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isOffsetConstraint(constraint)) return;
      if (point.binding?.kind === "graph-calibration") {
        const origin = env.resolveScenePoint(constraint.originIndex);
        if (!origin) return;
        const baseDx = constraint.dx;
        const baseDy = constraint.dy;
        const projection = window.GspRuntimeCore.projectToLineLike(
          world,
          origin,
          { x: origin.x + baseDx, y: origin.y + baseDy },
          "line",
        );
        if (!projection) return;
        constraint.dx = baseDx * projection.t;
        constraint.dy = baseDy * projection.t;
        return;
      }
      const originPoint = draft.points[constraint.originIndex];
      if (originPoint) {
        const originWorld = {
          x: world.x - constraint.dx,
          y: world.y - constraint.dy,
        };
        if (!originPoint.constraint) {
          originPoint.x = originWorld.x;
          originPoint.y = originWorld.y;
          return;
        }
        updatePointToWorld(env, draft, constraint.originIndex, originWorld);
        return;
      }
      const origin = env.resolveScenePoint(constraint.originIndex);
      if (!origin) return;
      constraint.dx = world.x - origin.x;
      constraint.dy = world.y - origin.y;
    },
    segment(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isLineLikeConstraint(constraint)) return;
      const line = "line" in constraint
        ? constraint.line
          ? modules.dynamics.resolveLineConstraintParameterPoints(
              (index: number) => env.resolveScenePoint(index),
              constraint.line,
            )
          : null
        : typeof constraint.startIndex === "number" && typeof constraint.endIndex === "number"
          ? [
              env.resolveScenePoint(constraint.startIndex),
              env.resolveScenePoint(constraint.endIndex),
            ]
          : null;
      const [start, end] = line || [];
      if (!start || !end) return;
      const projection = modules.scene.projectToLineLike(
        world,
        start,
        end,
        constraint.kind === "ray-constraint"
          ? "ray"
          : constraint.kind === "line-constraint"
            ? constraint.line?.kind === "segment" ? "segment" : "line"
            : constraint.kind,
      );
      if (projection) {
        constraint.t = projection.t;
        point.x = projection.projected.x;
        point.y = projection.projected.y;
      }
    },
    line(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.segment?.(env, draft, point, world);
    },
    "line-constraint"(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.segment?.(env, draft, point, world);
    },
    ray(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.segment?.(env, draft, point, world);
    },
    "ray-constraint"(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.segment?.(env, draft, point, world);
    },
    polyline(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isPolylineConstraint(constraint)) return;
      const points = modules.scene.resolveLinePoints(
        env,
        env.currentScene().lines.find((line: RuntimeLineJson) =>
          line?.binding?.kind === "arc-boundary" && line.binding.hostKey === constraint.functionKey
          || line?.debug?.groupOrdinal === constraint.functionKey
            && (
              line?.binding?.kind === "point-trace"
              || line?.binding?.kind === "coordinate-trace"
              || line?.binding?.kind === "custom-transform-trace"
            )
        ),
      ) || constraint.points;
      const count = points.length;
      let bestSegmentIndex = constraint.segmentIndex;
      let bestT = constraint.t;
      let bestDistanceSquared = Number.POSITIVE_INFINITY;
      for (let segmentIndex = 0; segmentIndex < count - 1; segmentIndex += 1) {
        const start = modules.scene.resolvePoint(env, points[segmentIndex]);
        const end = modules.scene.resolvePoint(env, points[segmentIndex + 1]);
        if (!start || !end) {
          continue;
        }
        const projection = modules.scene.projectToSegment(world, start, end);
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
      constraint.parameter = (bestSegmentIndex + bestT) / (count - 1);
    },
    "polygon-boundary"(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isPolygonBoundaryConstraint(constraint)) return;
      const count = constraint.vertexIndices.length;
      let bestEdgeIndex = constraint.edgeIndex;
      let bestT = constraint.t;
      let bestDistanceSquared = Number.POSITIVE_INFINITY;
      for (let edgeIndex = 0; edgeIndex < count; edgeIndex += 1) {
        const start = env.resolveScenePoint(constraint.vertexIndices[edgeIndex]);
        const end = env.resolveScenePoint(constraint.vertexIndices[(edgeIndex + 1) % count]);
        if (!start || !end) {
          continue;
        }
        const projection = modules.scene.projectToSegment(world, start, end);
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
    "polygon-boundary-parameter"(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isPolygonBoundaryParameterConstraint(constraint)) return;
      const count = constraint.vertexIndices.length;
      if (count < 2) return;
      const lengths: number[] = [];
      let perimeter = 0;
      let bestEdgeIndex = 0;
      let bestT = 0;
      let bestDistanceSquared = Number.POSITIVE_INFINITY;
      for (let edgeIndex = 0; edgeIndex < count; edgeIndex += 1) {
        const start = env.resolveScenePoint(constraint.vertexIndices[edgeIndex]);
        const end = env.resolveScenePoint(constraint.vertexIndices[(edgeIndex + 1) % count]);
        if (!start || !end) return;
        const length = Math.hypot(end.x - start.x, end.y - start.y);
        lengths.push(length);
        perimeter += length;
        const projection = modules.scene.projectToSegment(world, start, end);
        if (projection && projection.distanceSquared < bestDistanceSquared) {
          bestDistanceSquared = projection.distanceSquared;
          bestEdgeIndex = edgeIndex;
          bestT = projection.t;
        }
      }
      if (perimeter <= 1e-9) return;
      const traveled = lengths
        .slice(0, bestEdgeIndex)
        .reduce((sum, length) => sum + length, 0)
        + lengths[bestEdgeIndex] * bestT;
      constraint.parameter = traveled / perimeter;
    },
    "polygon-shape-boundary"(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isPolygonShapeBoundaryConstraint(constraint)) return;
      const polygon = draft.polygons?.[constraint.polygonIndex];
      if (!polygon || polygon.points.length < 2) return;
      let bestEdgeIndex = constraint.edgeIndex;
      let bestT = constraint.t;
      let bestDistanceSquared = Number.POSITIVE_INFINITY;
      for (let edgeIndex = 0; edgeIndex < polygon.points.length; edgeIndex += 1) {
        const start = modules.scene.resolvePoint(env, polygon.points[edgeIndex]);
        const end = modules.scene.resolvePoint(
          env,
          polygon.points[(edgeIndex + 1) % polygon.points.length],
        );
        if (!start || !end) continue;
        const projection = modules.scene.projectToSegment(world, start, end);
        if (projection && projection.distanceSquared < bestDistanceSquared) {
          bestDistanceSquared = projection.distanceSquared;
          bestEdgeIndex = edgeIndex;
          bestT = projection.t;
        }
      }
      constraint.edgeIndex = bestEdgeIndex;
      constraint.t = bestT;
    },
    circle(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isCircleConstraint(constraint)) return;
      const center = constraint.kind === "circle"
        ? env.resolveScenePoint(constraint.centerIndex)
        : modules.scene._circleFromConstraint?.(
            env,
            constraint.circle,
            (index: number) => env.resolveScenePoint(index),
          )?.center;
      if (!center) return;
      const dx = world.x - center.x;
      const dy = world.y - center.y;
      const length = Math.hypot(dx, dy);
      if (length > 1e-9) {
        constraint.unitX = dx / length;
        constraint.unitY = dy / length;
      }
    },
    "circular-constraint"(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      DRAGGED_POINT_CONSTRAINT_UPDATERS.circle?.(env, draft, point, world);
    },
    "circle-arc"(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isCircleArcConstraint(constraint)) return;
      const center = env.resolveScenePoint(constraint.centerIndex);
      const start = env.resolveScenePoint(constraint.startIndex);
      const end = env.resolveScenePoint(constraint.endIndex);
      if (!center || !start || !end) return;
      const projection = modules.scene.projectToCircleArc(
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
    arc(env: ViewerEnv, _draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
      const constraint = point.constraint;
      if (!isArcConstraint(constraint)) return;
      const start = env.resolveScenePoint(constraint.startIndex);
      const mid = env.resolveScenePoint(constraint.midIndex);
      const end = env.resolveScenePoint(constraint.endIndex);
      if (!start || !mid || !end) return;
      const projection = modules.scene.projectToThreePointArc(
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

  
  function updatePointToWorld(env: ViewerEnv, draft: ViewerSceneData, pointIndex: number, world: Point) {
    const point = draft.points[pointIndex];
    if (!point) return;
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
  }

  
  function updateDerivedPointSourceToWorld(env: ViewerEnv, draft: ViewerSceneData, point: RuntimeScenePointJson, world: Point) {
    if (point.binding?.kind !== "derived") return false;
    const parameters = modules.dynamics.parameterMapForScene?.(env, draft) ?? new Map<string, number>();
    const sourceWorld = window.GspRuntimeCore.inversePointTransform(
      world,
      point.binding.matrixApply,
      draft.points,
      parameters,
    );
    if (!sourceWorld) return false;
    updatePointToWorld(env, draft, point.binding.sourceIndex, sourceWorld);
    return true;
  }

  
  function dragModeFor(env: ViewerEnv, pointIndex: number | null, labelIndex: number | null, polygonIndex: number | null, iterationTableIndex: number | null, imageIndex: number | null) {
    if (pointIndex !== null) {
      const point = env.currentScene().points[pointIndex];
      const constraintKind = point?.constraint?.kind;
      if (constraintKind && !DRAGGED_POINT_CONSTRAINT_UPDATERS[constraintKind]) {
        return "pan";
      }
      if (typeof point?.binding?.kind === "string" && PAN_ONLY_POINT_BINDINGS.has(point.binding.kind)) {
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

  
  function beginDrag(env: ViewerEnv, pointerId: number, position: Point, pointIndex: number | null, labelIndex: number | null, polygonIndex: number | null, iterationTableIndex: number | null, imageIndex: number | null) {
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

  
  function updateDraggedPoint(env: ViewerEnv, world: Point) {
    const drag = env.dragState.val;
    if (!drag || drag.pointIndex === null) return;
    const pointIndex = drag.pointIndex;
    env.markDependencyRootsDirty?.(dependencyRootsForDraggedPoint(env, pointIndex));
    env.updateScene((draft: ViewerSceneData) => {
      const point = draft.points[pointIndex];
      if (!point) return;
      if (!updateDerivedPointSourceToWorld(env, draft, point, world)) {
        updatePointToWorld(env, draft, pointIndex, world);
      }
    }, "graph");
    env.hoverPointIndex.val = pointIndex;
  }

  
  function updateDraggedLabel(env: ViewerEnv, position: Point) {
    const drag = env.dragState.val;
    if (!drag || drag.labelIndex === null) return;
    const labelIndex = drag.labelIndex;
    env.updateScene((draft: ViewerSceneData) => {
      const label = draft.labels[labelIndex];
      if (!label) return;
      const anchor = label.anchor;
      if (!anchor) return;
      if (label.screenSpace && isCoordinateAnchor(anchor)) {
        anchor.x = position.x;
        anchor.y = position.y;
      } else if (isBoundAnchor(anchor)) {
        const base = env.resolveAnchorBase(anchor);
        if (!base) return;
        const world = env.toWorld(position.x, position.y);
        anchor.dx = world.x - base.x;
        anchor.dy = world.y - base.y;
      } else if (isCoordinateAnchor(anchor)) {
        const world = env.toWorld(position.x, position.y);
        anchor.x = world.x;
        anchor.y = world.y;
      }
    }, "none");
  }

  
  function updateDraggedPolygon(env: ViewerEnv, world: Point) {
    const drag = env.dragState.val;
    if (!drag || drag.polygonIndex === null) return;
    const polygonIndex = drag.polygonIndex;
    const previous = env.toWorld(drag.lastX, drag.lastY);
    const dx = world.x - previous.x;
    const dy = world.y - previous.y;
    if (Math.abs(dx) <= 1e-9 && Math.abs(dy) <= 1e-9) return;
    env.markDependencyRootsDirty?.(
      dependencyRootsForDraggedPolygon(env, polygonIndex),
    );
    env.updateScene(( draft: ViewerSceneData) => {
      const polygon = draft.polygons[polygonIndex];
      if (!polygon) return;
      polygon.points.forEach(( handle) => {
        if (!hasPointIndexHandle(handle)) return;
        const point = draft.points[handle.pointIndex];
        if (!point || point.constraint || point.binding) return;
        point.x += dx;
        point.y += dy;
      });
    }, "graph");
  }

  
  function updateDraggedIterationTable(env: ViewerEnv, position: Point) {
    const drag = env.dragState.val;
    if (!drag || drag.iterationTableIndex === null) return;
    const iterationTableIndex = drag.iterationTableIndex;
    env.updateScene(( draft: ViewerSceneData) => {
      const table = draft.iterationTables?.[iterationTableIndex];
      if (!table) return;
      table.x = position.x;
      table.y = position.y;
    }, "none");
  }

  
  function updateDraggedImage(env: ViewerEnv, position: Point) {
    const drag = env.dragState.val;
    if (!drag || drag.imageIndex === null) return;
    const imageIndex = drag.imageIndex;
    env.updateScene((draft: ViewerSceneData) => {
      const image = draft.images?.[imageIndex];
      if (!image) return;
      const dxScreen = position.x - drag.lastX;
      const dyScreen = position.y - drag.lastY;
      if (Math.abs(dxScreen) <= 1e-9 && Math.abs(dyScreen) <= 1e-9) return;
      if (image.screenSpace) {
        image.topLeft.x += dxScreen;
        image.topLeft.y += dyScreen;
        image.bottomRight.x += dxScreen;
        image.bottomRight.y += dyScreen;
        return;
      }
      const worldNow = env.toWorld(position.x, position.y);
      const worldLast = env.toWorld(drag.lastX, drag.lastY);
      const dx = worldNow.x - worldLast.x;
      const dy = worldNow.y - worldLast.y;
      image.topLeft.x += dx;
      image.topLeft.y += dy;
      image.bottomRight.x += dx;
      image.bottomRight.y += dy;
    }, "none");
  }

  
  function panFromPointerDelta(env: ViewerEnv, position: Point) {
    const drag = env.dragState.val;
    if (!drag) return;
    const worldNow = env.toWorld(position.x, position.y);
    const worldLast = env.toWorld(drag.lastX, drag.lastY);
    env.view.centerX -= worldNow.x - worldLast.x;
    env.view.centerY -= worldNow.y - worldLast.y;
  }

  modules.drag = {
    dragModeFor,
    beginDrag,
    updatePointToWorld,
    updateDraggedPoint,
    updateDraggedLabel,
    updateDraggedImage,
    updateDraggedPolygon,
    updateDraggedIterationTable,
    panFromPointerDelta,
  };
})();
