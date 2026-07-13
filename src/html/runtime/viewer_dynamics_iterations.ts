(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function createDynamicsIterations(dependencies: Record<string, any>) {
    const {
      affineMapFromTriangles,
      applyNormalizedParameterToPoint,
      applySegmentCoefficients,
      buildPlainTextRichMarkup,
      cloneTracePoint,
      deriveExpressionLabelParameters,
      deriveLabelParameters,
      discreteIterationDepth,
      DERIVED_POINT_BINDING_REFRESHERS,
      evaluateExpr,
      evaluateRecursiveExpression,
      formatSequenceValue,
      hasLineIndexHandle,
      hasPointIndexHandle,
      isFiniteNumber,
      pointAngleValue,
      pointIterationDepth,
      refreshDerivedPoints,
      rotateAround,
      samplePointTraceLine,
      segmentPointCoefficients,
      SYNC_DYNAMIC_POINT_BINDING_UPDATERS,
    } = dependencies;
  function rebuildIterationPoints(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    const families = env.sourceScene.pointIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      if (family.kind === "parameterized") {
        return sum;
      }
      return sum + (family.depth || 0);
    }, 0);
    const standaloneParameterPoints = env.sourceScene.points.filter(( point) =>
      point?.binding?.kind === "parameter" && !point.constraint
    );
    const baseCount = Math.max(
      0,
      env.sourceScene.points.length - exportedDepth - standaloneParameterPoints.length,
    );
    scene.points = scene.points.slice(0, baseCount);

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (depth <= 0) {
        return;
      }
      if (family.kind === "offset") {
        let previousIndex = family.seedIndex;
        for (let step = 0; step < depth; step += 1) {
          const origin = scene.points[previousIndex];
          if (!origin) {
            break;
          }
          scene.points.push({
            x: origin.x + family.dx,
            y: origin.y + family.dy,
            color: origin.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: {
              kind: "offset",
              originIndex: previousIndex,
              dx: family.dx,
              dy: family.dy,
            },
            binding: null,
            debug: null,
          });
          previousIndex = scene.points.length - 1;
        }
        return;
      }

      if (family.kind === "rotate-chain") {
        const center = scene.points[family.centerIndex];
        let previousIndex = family.seedIndex;
        if (!center) {
          return;
        }
        for (let step = 0; step < depth; step += 1) {
          const source = scene.points[previousIndex];
          if (!source) {
            break;
          }
          const rotated = rotateAround(source, center, family.angleDegrees * Math.PI / 180);
          scene.points.push({
            x: rotated.x,
            y: rotated.y,
            color: source.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: null,
            binding: {
              kind: "rotate",
              sourceIndex: previousIndex,
              centerIndex: family.centerIndex,
              angleDegrees: family.angleDegrees,
            },
            debug: null,
          });
          previousIndex = scene.points.length - 1;
        }
        return;
      }

      if (family.kind === "rotate") {
        const source = scene.points[family.sourceIndex];
        const center = scene.points[family.centerIndex];
        if (!source || !center) {
          return;
        }
        const angleDegrees = evaluateExpr(family.angleExpr, 0, parameters);
        if (typeof angleDegrees !== "number" || !Number.isFinite(angleDegrees)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const rotated = rotateAround(source, center, (angleDegrees * step) * Math.PI / 180);
          scene.points.push({
            x: rotated.x,
            y: rotated.y,
            color: source.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: null,
            binding: {
              kind: "rotate",
              sourceIndex: family.sourceIndex,
              centerIndex: family.centerIndex,
              angleDegrees: angleDegrees * step,
            },
            debug: null,
          });
        }
        return;
      }

      if (family.kind === "parameterized") {
        let currentValue = parameters.get(family.traceParameterName);
        if (!isFiniteNumber(currentValue)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const nextValue = evaluateRecursiveExpression(
            family.stepExpr,
            family.traceParameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(nextValue)) {
            break;
          }
          currentValue = nextValue;
          const traceParameters = deriveLabelParameters(
            scene,
            new Map<string, number>(parameters).set(family.traceParameterName, currentValue),
          );
          const points = resolvePointsWithParameters(env, scene, traceParameters);
          const source = points[family.pointIndex];
          if (!source) {
            continue;
          }
          scene.points.push({
            x: source.x,
            y: source.y,
            color: source.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: null,
            binding: null,
            debug: null,
          });
        }
      }
    });

    standaloneParameterPoints.forEach(( point) => {
      scene.points.push({
        ...point,
        constraint: point.constraint ? { ...point.constraint } : null,
        binding: point.binding ? { ...point.binding } : null,
      });
    });
  }


  function resolvePointsWithParameters(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    const draft = {
      ...scene,
      lines: scene.lines,
      circles: scene.circles,
      points: scene.points.map(cloneTracePoint),
    };
    const draftEnv = {
      ...env,
      currentScene: () => draft,
      resolveScenePoint: ( index: number) => draft.points[index],
    };

    const refreshDerivedPoints = () => {
      draft.points.forEach(( point) => {
        const refreshBinding = point.binding ? DERIVED_POINT_BINDING_REFRESHERS[point.binding.kind] : null;
        if (refreshBinding) {
          refreshBinding(draftEnv, draft, point, parameters);
        }
      });
    };

    const resolveConstrainedPoints = () => {
      draft.points.forEach(( point,  pointIndex: number) => {
        if (!point.constraint) {
          return;
        }
        const resolved = modules.scene.resolveConstrainedPoint(
          {
            sourceScene: env.sourceScene,
            currentScene: () => draft,
            resolveScenePoint: ( index: number) => draft.points[index],
          },
          point.constraint,
          ( index: number) => draft.points[index],
          point,
        );
        if (resolved) {
          draft.points[pointIndex].x = resolved.x;
          draft.points[pointIndex].y = resolved.y;
        }
      });
    };

    draft.points.forEach(( point) => {
      if (point.binding?.kind === "parameter" && point.constraint) {
        const value = parameters.get(point.binding.name);
        if (isFiniteNumber(value)) {
          applyNormalizedParameterToPoint(point, draft, value);
        }
        return;
      }
      const updatePoint = point.binding ? SYNC_DYNAMIC_POINT_BINDING_UPDATERS[point.binding.kind] : null;
      if (updatePoint) {
        updatePoint(draftEnv, draft, point, parameters);
      }
    });
    for (let pass = 0; pass < 3; pass += 1) {
      refreshDerivedPoints();
      resolveConstrainedPoints();
    }
    refreshDerivedPoints();
    return draft.points;
  }


  function rebuildIteratedLines(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    const families = env.sourceScene.lineIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      const depth = family.depth || 0;
      if (family.kind === "parameterized-point-trace") {
        return sum;
      }
      if (family.kind === "rotate") {
        return sum;
      }
      if (family.kind === "branching") {
        const branchCount = Array.isArray(family.targetSegments) ? family.targetSegments.length : 0;
        let total = 0;
        let width = branchCount;
        for (let step = 0; step < depth; step += 1) {
          total += width;
          width *= branchCount;
        }
        return sum + total;
      }
      if (family.kind === "affine") {
        return sum + depth;
      }
      if (family.bidirectional) {
        if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
          return sum + (2 * depth * (depth + 1));
        }
        return sum + (2 * depth);
      }
      if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
        return sum + (((depth + 1) * (depth + 2)) / 2 - 1);
      }
      return sum + depth;
    }, 0);
    const baseCount = Math.max(0, env.sourceScene.lines.length - exportedDepth);
    scene.lines = scene.lines.slice(0, baseCount);

    const resetControlledTickColors = new Set<string>();

    const emittedControlledTickSeeds = new Set<string>();

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (depth <= 0) {
        return;
      }
      const resolveHandle = ( handle) => {
        if (hasPointIndexHandle(handle)) {
          return env.resolveScenePoint(handle.pointIndex);
        }
        if (hasLineIndexHandle(handle)) {
          const line = scene.lines[handle.lineIndex];
          if (!line?.points || line.points.length < 2) return null;
          const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
          const t = typeof handle.t === "number" ? handle.t : 0.5;
          const p0 = line.points[segmentIndex];
          const p1 = line.points[segmentIndex + 1];
          return {
            x: p0.x + (p1.x - p0.x) * t,
            y: p0.y + (p1.y - p0.y) * t,
          };
        }
        return  (handle);
      };
      if (family.kind === "parameterized-point-trace") {
        const depthParameterName = family.depthParameterName;
        const depthParameterValue = typeof depthParameterName === "string"
          ? parameters.get(depthParameterName)
          : undefined;
        const depth = Math.max(
          0,
          Math.round(
            isFiniteNumber(depthParameterValue)
              ? depthParameterValue
              : family.depth || 0,
          ),
        );
        let currentValue = parameters.get(family.traceParameterName);
        if (!isFiniteNumber(currentValue)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const nextValue = evaluateRecursiveExpression(
            family.stepExpr,
            family.traceParameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(nextValue)) {
            break;
          }
          currentValue = nextValue;
          const traceParameters = deriveLabelParameters(
            scene,
            new Map<string, number>(parameters).set(family.traceParameterName, currentValue),
          );
          const line: RuntimeLineJson = {

            points: [],
            color: family.color,
            dashed: !!family.dashed,
            visible: family.visible !== false,
            binding: {
              kind: "point-trace",
              pointIndex: family.pointIndex,
              driverIndex: family.driverIndex,
              xMin: family.xMin,
              xMax: family.xMax,
              sampleCount: family.sampleCount,
              useMidpoints: true,
            },
          };
          const sampled = samplePointTraceLine(scene, line, traceParameters);
          if (!sampled) {
            continue;
          }
          scene.lines.push({
            points: sampled,
            color: family.color,
            dashed: !!family.dashed,
            visible: family.visible !== false,
            binding: null,
          });
        }
        return;
      }
      if (family.kind === "branching") {
        const start = env.resolveScenePoint(family.startIndex);
        const end = env.resolveScenePoint(family.endIndex);
        if (!start || !end) {
          return;
        }
        const targetSegments = (family.targetSegments || []).map((segment) => [
          resolveHandle(segment[0]),
          resolveHandle(segment[1]),
        ]);
        if (targetSegments.some((segment) => segment.some((point) => !point))) {
          return;
        }
        const coeffs = targetSegments
          .flatMap((segment) => {
            const [targetStart, targetEnd] = segment;
            if (!targetStart || !targetEnd) {
              return [];
            }
            const startCoeffs = segmentPointCoefficients(start, end, targetStart);
            const endCoeffs = segmentPointCoefficients(start, end, targetEnd);
            if (!startCoeffs || !endCoeffs) {
              return [];
            }
            return [{ startCoeffs, endCoeffs }];
          });
        if (coeffs.length === 0) {
          return;
        }

        let frontier = [{ start: { ...start }, end: { ...end } }];
        for (let step = 0; step < depth; step += 1) {

          const next = [];
          frontier.forEach((segment) => {
            coeffs.forEach((coeff) => {
              const childStart = applySegmentCoefficients(segment.start, segment.end, coeff.startCoeffs);
              const childEnd = applySegmentCoefficients(segment.start, segment.end, coeff.endCoeffs);
              scene.lines.push({
                points: [{ ...childStart }, { ...childEnd }],
                color: family.color,
                dashed: !!family.dashed,
                visible: family.visible !== false,
                binding: null,
              });
              next.push({ start: childStart, end: childEnd });
            });
          });
          frontier = next;
        }
        return;
      }
      if (family.kind === "affine") {
        const start = env.resolveScenePoint(family.startIndex);
        const end = env.resolveScenePoint(family.endIndex);
        if (!start || !end) {
          return;
        }
        const sourceTriangle = family.sourceTriangleIndices.map((index: number) => env.resolveScenePoint(index));
        const targetTriangle = family.targetTriangle.map((handle) => resolveHandle(handle));
        if (sourceTriangle.some((point) => !point) || targetTriangle.some((point) => !point)) {
          return;
        }
        const mapPoint = affineMapFromTriangles(
           (sourceTriangle),
           (targetTriangle),
        );
        if (!mapPoint) {
          return;
        }
        let currentStart = { ...start };
        let currentEnd = { ...end };
        for (let step = 0; step < depth; step += 1) {
          currentStart = mapPoint(currentStart);
          currentEnd = mapPoint(currentEnd);
          scene.lines.push({
            points: [{ ...currentStart }, { ...currentEnd }],
            color: family.color,
            dashed: !!family.dashed,
            visible: family.visible !== false,
            binding: null,
          });
        }
        return;
      }
      if (family.kind === "rotate") {
        const source = scene.lines[family.sourceIndex];
        const center = scene.points[family.centerIndex];
        if (!source || !center) {
          return;
        }
        const angleDegrees = evaluateExpr(family.angleExpr, 0, parameters);
        if (typeof angleDegrees !== "number" || !Number.isFinite(angleDegrees)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const radians = (angleDegrees * step) * Math.PI / 180;
          scene.lines.push({
            points: source.points.map(( point) => rotateAround(point, center, radians)),
            color: family.color,
            dashed: !!family.dashed,
            visible: family.visible !== false,
            binding: {
              kind: "derived",
              sourceIndex: family.sourceIndex,
              transform: {
                kind: "rotate",
                centerIndex: family.centerIndex,
                angleDegrees: angleDegrees * step,
                parameterName: null,
                angleStartIndex: null,
                angleVertexIndex: null,
                angleEndIndex: null,
              },
            },
          });
        }
        return;
      }
      if (family.kind !== "translate") return;
      const start = env.resolveScenePoint(family.startIndex);
      const end = env.resolveScenePoint(family.endIndex);
      if (!start || !end) {
        return;
      }
      let primaryDx = family.dx;
      let primaryDy = family.dy;
      if (typeof family.vectorStartIndex === "number" && typeof family.vectorEndIndex === "number") {
        const vectorStart = env.resolveScenePoint(family.vectorStartIndex);
        const vectorEnd = env.resolveScenePoint(family.vectorEndIndex);
        if (vectorStart && vectorEnd) {
          primaryDx = vectorEnd.x - vectorStart.x;
          primaryDy = vectorEnd.y - vectorStart.y;
        }
      }

      const controlledEndpoint = (point, controlIndex: number) => {
        if (typeof controlIndex !== "number" || !Number.isFinite(controlIndex)) return point;
        const control = env.resolveScenePoint(controlIndex);
        if (!control) return point;
        return { x: point.x, y: control.y };
      };
      const liveStart = controlledEndpoint(start, family.startControlIndex);
      const liveEnd = controlledEndpoint(end, family.endControlIndex);
      if (Number.isFinite(family.startControlIndex) || Number.isFinite(family.endControlIndex)) {
        const colorKey = JSON.stringify(family.color || null);
        if (!resetControlledTickColors.has(colorKey)) {
          scene.lines = scene.lines.filter((line) => {
            if (line.binding || !Array.isArray(line.points) || line.points.length !== 2) return true;
            const lineStart = resolveHandle(line.points[0]);
            const lineEnd = resolveHandle(line.points[1]);
            if (!lineStart || !lineEnd) return true;
            const sameColor = JSON.stringify(line.color || null) === colorKey;
            const vertical = Math.abs(lineStart.x - lineEnd.x) < 1e-6;
            return !(sameColor && vertical);
          });
          resetControlledTickColors.add(colorKey);
        }
        const seedKey = `${colorKey}:${family.startIndex}:${family.endIndex}`;
        if (!emittedControlledTickSeeds.has(seedKey)) {
          scene.lines.push({
            points: [
              { x: liveStart.x, y: liveStart.y },
              { x: liveEnd.x, y: liveEnd.y },
            ],
            color: family.color,
            dashed: !!family.dashed,
            visible: family.visible !== false,
            binding: null,
          });
          emittedControlledTickSeeds.add(seedKey);
        }
      }
      const secondaryDx = isFiniteNumber(family.secondaryDx) ? family.secondaryDx : null;
      const secondaryDy = isFiniteNumber(family.secondaryDy) ? family.secondaryDy : null;
      const hasSecondary = secondaryDx !== null && secondaryDy !== null;
      const deltas = [];
      if (family.bidirectional && hasSecondary) {
        for (let primary = -depth; primary <= depth; primary += 1) {
          for (let secondary = -depth; secondary <= depth; secondary += 1) {
            if (primary === 0 && secondary === 0) {
              continue;
            }
            if (Math.abs(primary) + Math.abs(secondary) > depth) {
              continue;
            }
            deltas.push({
              dx: primaryDx * primary + secondaryDx * secondary,
              dy: primaryDy * primary + secondaryDy * secondary,
            });
          }
        }
      } else if (family.bidirectional) {
        for (let step = 1; step <= depth; step += 1) {
          deltas.push(
            { dx: primaryDx * step, dy: primaryDy * step },
            { dx: -primaryDx * step, dy: -primaryDy * step },
          );
        }
      } else if (hasSecondary) {
        for (let primary = 0; primary <= depth; primary += 1) {
          for (let secondary = 0; secondary <= depth - primary; secondary += 1) {
            if (primary === 0 && secondary === 0) {
              continue;
            }
            deltas.push({
              dx: primaryDx * primary + secondaryDx * secondary,
              dy: primaryDy * primary + secondaryDy * secondary,
            });
          }
        }
      } else {
        for (let step = 1; step <= depth; step += 1) {
          deltas.push({
            dx: primaryDx * step,
            dy: primaryDy * step,
          });
        }
      }
      deltas.forEach(({ dx, dy }) => {
        scene.lines.push({
          points: [
            { x: liveStart.x + dx, y: liveStart.y + dy },
            { x: liveEnd.x + dx, y: liveEnd.y + dy },
          ],
          color: family.color,
          dashed: !!family.dashed,
          visible: family.visible !== false,
          binding: null,
        });
      });
    });
  }


  function rebuildIteratedPolygons(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    const families = env.sourceScene.polygonIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      if (family.kind === "coordinate-grid") {
        return sum + Math.max(0, Math.round(family.depth || 0));
      }
      const depth = family.depth || 0;
      if (family.bidirectional) {
        if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
          return sum + (1 + 2 * depth * (depth + 1));
        }
        return sum + (1 + 2 * depth);
      }
      if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
        return sum + (((depth + 1) * (depth + 2)) / 2);
      }
      return sum + (depth + 1);
    }, 0);
    const baseCount = Math.max(0, env.sourceScene.polygons.length - exportedDepth);
    scene.polygons = scene.polygons.slice(0, baseCount);

    families.forEach((family) => {
      if (family.vertexIndices.length < 3) {
        return;
      }
      const sourcePolygon = scene.polygons.find((polygon) =>
        polygon.binding?.kind === "point-polygon"
        && polygon.binding.vertexIndices.length === family.vertexIndices.length
        && polygon.binding.vertexIndices.every((index: number, slot: number) => index === family.vertexIndices[slot])
      );
      const familyColor = sourcePolygon?.color || family.color;
      const seedVertices = family.vertexIndices
        .map((index: number) => env.resolveScenePoint(index));
      if (seedVertices.some((point) => !point)) {
        return;
      }
      const seedPoints =  (seedVertices);
      if (family.kind === "coordinate-grid") {
        const depthValue = family.depthExpr
          ? evaluateExpr(family.depthExpr, 0, parameters)
          : family.depth;
        const depth = Math.max(0, Math.floor(isFiniteNumber(depthValue) ? depthValue : family.depth || 0));
        let currentValue = parameters.get(family.parameterName);
        if (!isFiniteNumber(currentValue)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const nextValue = evaluateRecursiveExpression(
            family.stepExpr,
            family.parameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(nextValue)) {
            break;
          }
          currentValue = nextValue;
          const exprParameters = deriveLabelParameters(
            scene,
            new Map<string, number>(parameters).set(family.parameterName, currentValue),
          );
          const dx = evaluateExpr(family.xExpr, 0, exprParameters);
          const dy = evaluateExpr(family.yExpr, 0, exprParameters);
          if (!isFiniteNumber(dx) || !isFiniteNumber(dy)) {
            continue;
          }
          scene.polygons.push({
            points: seedPoints.map((point) => ({
              x: point.x + dx * family.xRawScale,
              y: point.y - dy * family.yRawScale,
            })),
            color: familyColor,
            visible: family.visible !== false,
            binding: null,
          });
        }
        return;
      }
      const depth = pointIterationDepth(family, parameters);
      if (family.kind !== "translate") {
        return;
      }
      const secondaryDx = isFiniteNumber(family.secondaryDx) ? family.secondaryDx : null;
      const secondaryDy = isFiniteNumber(family.secondaryDy) ? family.secondaryDy : null;
      const hasSecondary = secondaryDx !== null && secondaryDy !== null;
      const vectorStart = isFiniteNumber(family.vectorStartIndex)
        ? env.resolveScenePoint(family.vectorStartIndex)
        : null;
      const vectorEnd = isFiniteNumber(family.vectorEndIndex)
        ? env.resolveScenePoint(family.vectorEndIndex)
        : null;
      const primaryDx = vectorStart && vectorEnd ? vectorEnd.x - vectorStart.x : family.dx;
      const primaryDy = vectorStart && vectorEnd ? vectorEnd.y - vectorStart.y : family.dy;
      const deltas = [];
      if (family.bidirectional && hasSecondary) {
        for (let primary = -depth; primary <= depth; primary += 1) {
          for (let secondary = -depth; secondary <= depth; secondary += 1) {
            if (Math.abs(primary) + Math.abs(secondary) > depth) {
              continue;
            }
            deltas.push({
              dx: primaryDx * primary + secondaryDx * secondary,
              dy: primaryDy * primary + secondaryDy * secondary,
            });
          }
        }
      } else if (family.bidirectional) {
        deltas.push({ dx: 0, dy: 0 });
        for (let step = 1; step <= depth; step += 1) {
          deltas.push(
            { dx: primaryDx * step, dy: primaryDy * step },
            { dx: -primaryDx * step, dy: -primaryDy * step },
          );
        }
      } else if (hasSecondary) {
        for (let primary = 0; primary <= depth; primary += 1) {
          for (let secondary = 0; secondary <= depth - primary; secondary += 1) {
            deltas.push({
              dx: primaryDx * primary + secondaryDx * secondary,
              dy: primaryDy * primary + secondaryDy * secondary,
            });
          }
        }
      } else {
        for (let step = 0; step <= depth; step += 1) {
          deltas.push({
            dx: primaryDx * step,
            dy: primaryDy * step,
          });
        }
      }
      deltas.forEach(({ dx, dy }) => {
        scene.polygons.push({
          points: seedPoints.map((point) => ({ x: point.x + dx, y: point.y + dy })),
          color: familyColor,
          visible: family.visible !== false,
          binding: null,
        });
      });
    });
  }


  function rebuildIteratedLabels(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    const families = env.sourceScene.labelIterations || [];
    if (families.length === 0) {
      return;
    }
    const baseCount = env.sourceScene.labels.length;
    scene.labels = scene.labels.slice(0, baseCount);

    families.forEach((family) => {
      if (family.kind !== "point-expression") {
        if (family.kind !== "translate-expression") {
          return;
        }
        const seedLabel = scene.labels[family.seedLabelIndex];
        const vectorStart = scene.points[family.vectorStartIndex];
        const vectorEnd = scene.points[family.vectorEndIndex];
        if (!seedLabel || !vectorStart || !vectorEnd) {
          return;
        }
        if (isFiniteNumber(family.firstOutputLabelIndex) && isFiniteNumber(family.outputLabelCount)) {
          for (let index = 0; index < family.outputLabelCount; index += 1) {
            const label = scene.labels[family.firstOutputLabelIndex + index];
            if (label) {
              label.visible = false;
            }
          }
        }
        const depth = pointIterationDepth({
          depth: family.depth,
          depthExpr: family.depthExpr,
          depthParameterName: family.depthParameterName,
        }, parameters);
        const seedAnchor = seedLabel.anchor;
        let currentValue = parameters.get(family.parameterName);
        if (!seedAnchor || !isFiniteNumber(currentValue)) {
          return;
        }
        const seedAnchorPoint = env.resolvePoint(seedAnchor);
        if (!seedAnchorPoint) {
          return;
        }
        const dx = vectorEnd.x - vectorStart.x;
        const dy = vectorEnd.y - vectorStart.y;
        const seedValue = evaluateRecursiveExpression(
          family.expr,
          family.parameterName,
          currentValue,
          parameters,
        );
        if (!isFiniteNumber(seedValue)) {
          return;
        }
        currentValue = seedValue;
        for (let step = 1; step <= depth; step += 1) {
          const value = evaluateRecursiveExpression(
            family.expr,
            family.parameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(value)) {
            break;
          }
          currentValue = value;
          const text = formatSequenceValue(value);
          scene.labels.push({
            ...seedLabel,
            text,
            richMarkup: buildPlainTextRichMarkup(text),
            binding: null,
            anchor: { x: seedAnchorPoint.x + dx * step, y: seedAnchorPoint.y + dy * step },
          });
        }
        return;
      }
      const seedLabel = scene.labels[family.seedLabelIndex];
      const seedAnchor = seedLabel?.anchor;
      const seedPointIndex = typeof seedAnchor?.pointIndex === "number"
        ? seedAnchor.pointIndex
        : (seedLabel?.binding?.kind === "point-expression-value"
          && typeof seedLabel.binding.pointIndex === "number"
            ? seedLabel.binding.pointIndex
            : null);
      if (!seedLabel || seedPointIndex === null) {
        return;
      }
      const depth = pointIterationDepth({
        depth: family.depth,
        parameterName: family.depthParameterName,
      }, parameters);
      let currentValue = parameters.get(family.parameterName);
      if (!isFiniteNumber(currentValue)) {
        return;
      }
      for (let step = 0; step <= depth; step += 1) {
        const value = evaluateRecursiveExpression(
          family.expr,
          family.parameterName,
          currentValue,
          parameters,
        );
        if (!isFiniteNumber(value)) {
          break;
        }
        const pointIndex = family.pointSeedIndex + step;
        if (!scene.points[pointIndex]) {
          break;
        }
        if (step === 0) {
          seedLabel.text = formatSequenceValue(value);
          seedLabel.richMarkup = buildPlainTextRichMarkup(seedLabel.text);
          seedLabel.anchor = { ...seedAnchor, pointIndex: seedPointIndex };
        } else {
          scene.labels.push({
            ...seedLabel,
            text: formatSequenceValue(value),
            richMarkup: buildPlainTextRichMarkup(formatSequenceValue(value)),
            binding: null,
            anchor: { ...seedAnchor, pointIndex },
          });
        }
        currentValue = value;
      }
    });
  }


  function rebuildIterationTables(env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) {
    const sourceTables = env.sourceScene.iterationTables || [];
    const currentTables = scene.iterationTables || [];
    scene.iterationTables = sourceTables.map((table, index: number) => {
      const current = currentTables[index];
      const depth = table.depthExpr
        ? discreteIterationDepth(evaluateExpr(table.depthExpr, 0, parameters) ?? table.depth)
        : table.depthParameterName
        ? discreteIterationDepth(parameters.get(table.depthParameterName) ?? table.depth)
        : discreteIterationDepth(table.depth);
      const columns = Array.isArray(table.columns) && table.columns.length > 0
        ? table.columns
        : [{ exprLabel: table.exprLabel, parameterName: table.parameterName, expr: table.expr }];
      const state = new Map<string, number>(parameters);
      const initialDerived = deriveExpressionLabelParameters(scene, state);
      columns.forEach(( column) => {
        const value = initialDerived.get(column.parameterName);
        if (isFiniteNumber(value)) {
          state.set(column.parameterName, value);
        }
      });

      const rows = [];
      if (columns.every(( column) => column.valueBinding || isFiniteNumber(state.get(column.parameterName)))) {
        for (let index = 0; index <= depth; index += 1) {
          const derived = deriveExpressionLabelParameters(scene, state);
          columns.forEach(( column) => {
            const value = state.get(column.parameterName);
            if (isFiniteNumber(value)) {
              derived.set(column.parameterName, value);
            }
          });
          columns.forEach((column) => {
            if (column.valueBinding?.kind !== "angle-marker") return;
            const value = pointAngleValue(scene, column.valueBinding);
            if (isFiniteNumber(value)) {
              derived.set(column.parameterName, value);
            }
          });
          const values = columns.map(( column) => {
            if (column.valueBinding?.kind === "angle-marker") {
              return derived.get(column.parameterName);
            }
            return evaluateExpr(column.expr, 0, derived);
          });
          if (!values.every(isFiniteNumber)) {
            break;
          }
          rows.push({ index, value: values[0], values });
          columns.forEach(( column,  columnIndex: number) => {
            state.set(column.parameterName, values[columnIndex]);
          });
        }
      }
      return {
        ...table,
        x: Number.isFinite(current?.x) ? current.x : table.x,
        y: Number.isFinite(current?.y) ? current.y : table.y,
        rows,
      };
    });
  }



    return {
      rebuildIterationPoints,
      rebuildIteratedLines,
      rebuildIteratedPolygons,
      rebuildIteratedLabels,
      rebuildIterationTables,
    };
  }

  modules.dynamicsIterations = { createDynamicsIterations };
})();
