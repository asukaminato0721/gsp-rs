(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  type DependencyCollectorOptions = {
    sourceScene: SceneData | ViewerSceneData;
    knownParameters: Set<string>;
    derivedParameterDeps?: Map<string, Set<string>>;
    collectExprParameterNames: (
      expr: FunctionExprJson | FunctionAstJson | null | undefined,
      names: Set<string>,
    ) => void;
  };

  function createSceneDependencyCollector(options: DependencyCollectorOptions) {
    const {
      sourceScene,
      knownParameters,
      derivedParameterDeps = new Map<string, Set<string>>(),
      collectExprParameterNames,
    } = options;

    function parameter(deps: Set<string>, name: string | null | undefined) {
      if (typeof name !== "string") return;
      if (knownParameters.has(name)) deps.add(`param:${name}`);
      derivedParameterDeps.get(name)?.forEach((dep) => deps.add(dep));
    }

    function expr(deps: Set<string>, value: FunctionExprJson | FunctionAstJson | null | undefined) {
      const names = new Set<string>();
      collectExprParameterNames(value, names);
      names.forEach((name) => parameter(deps, name));
    }

    function point(deps: Set<string>, index: number | null | undefined) {
      if (typeof index !== "number") return;
      deps.add(`source-point:${index}`);
      const source = sourceScene.points?.[index];
      if (source?.binding || source?.constraint) deps.add(`point:${index}`);
    }

    function points(
      deps: Set<string>,
      indices: readonly (number | null | undefined)[] | null | undefined,
    ) {
      indices?.forEach((index) => point(deps, index));
    }

    function line(deps: Set<string>, index: number | null | undefined) {
      if (typeof index !== "number") return;
      deps.add(`source-line:${index}`);
      if (sourceScene.lines?.[index]?.binding) deps.add(`line:${index}`);
    }

    function circle(deps: Set<string>, index: number | null | undefined) {
      if (typeof index !== "number") return;
      deps.add(`source-circle:${index}`);
      const source = sourceScene.circles?.[index];
      if (source?.binding || source?.fillColorBinding) deps.add(`circle:${index}`);
    }

    function polygon(deps: Set<string>, index: number | null | undefined) {
      if (typeof index !== "number") return;
      deps.add(`source-polygon:${index}`);
      const source = sourceScene.polygons?.[index];
      if (source?.binding || source?.colorBinding) deps.add(`polygon:${index}`);
    }

    function pointHandle(deps: Set<string>, handle: IterationPointHandleJson) {
      if ("pointIndex" in handle) point(deps, handle.pointIndex);
      else if ("lineIndex" in handle) line(deps, handle.lineIndex);
    }

    function lineConstraint(deps: Set<string>, value: LineConstraintJson) {
      switch (value.kind) {
        case "segment":
        case "line":
        case "ray":
          points(deps, [value.startIndex, value.endIndex]);
          break;
        case "perpendicular-line":
        case "parallel-line":
          points(deps, [value.throughIndex, value.lineStartIndex, value.lineEndIndex]);
          break;
        case "angle-bisector-ray":
          points(deps, [value.startIndex, value.vertexIndex, value.endIndex]);
          break;
        case "translated":
          lineConstraint(deps, value.line);
          points(deps, [value.vectorStartIndex, value.vectorEndIndex]);
          break;
      }
    }

    function circularConstraint(deps: Set<string>, value: CircularConstraintJson) {
      switch (value.kind) {
        case "circle":
          points(deps, [value.centerIndex, value.radiusIndex]);
          break;
        case "segment-radius-circle":
          points(deps, [value.centerIndex, value.lineStartIndex, value.lineEndIndex]);
          break;
        case "parameter-radius-circle":
          point(deps, value.centerIndex);
          parameter(deps, value.parameterName);
          break;
        case "expression-radius-circle":
          point(deps, value.centerIndex);
          expr(deps, value.expr);
          break;
        case "derived":
          circularConstraint(deps, value.source);
          transform(deps, value.transform);
          break;
        case "circle-arc":
          points(deps, [value.centerIndex, value.startIndex, value.endIndex]);
          break;
        case "three-point-arc":
          points(deps, [value.startIndex, value.midIndex, value.endIndex]);
          break;
      }
    }

    function transform(deps: Set<string>, value: TransformJson) {
      switch (value.kind) {
        case "translate":
          points(deps, [value.vectorStartIndex, value.vectorEndIndex]);
          break;
        case "translate-delta":
          break;
        case "rotate":
          points(deps, [value.centerIndex, value.angleStartIndex, value.angleVertexIndex, value.angleEndIndex]);
          parameter(deps, value.parameterName);
          break;
        case "scale":
          point(deps, value.centerIndex);
          break;
        case "reflect":
          points(deps, [value.lineStartIndex, value.lineEndIndex]);
          line(deps, value.lineIndex);
          break;
      }
    }

    function pointTransform(deps: Set<string>, value: PointTransformJson) {
      switch (value.kind) {
        case "translate":
          points(deps, [value.vectorStartIndex, value.vectorEndIndex]);
          break;
        case "reflect":
          points(deps, [value.lineStartIndex, value.lineEndIndex]);
          break;
        case "reflect-constraint":
          lineConstraint(deps, value.line);
          break;
        case "rotate":
          points(deps, [
            value.centerIndex,
            value.angleStartIndex,
            value.angleVertexIndex,
            value.angleEndIndex,
            value.angleParameterPointIndex,
            value.angleParameterStartIndex,
            value.angleParameterEndIndex,
          ]);
          parameter(deps, value.parameterName);
          expr(deps, value.angleExpr);
          break;
        case "scale":
          points(deps, [
            value.centerIndex,
            value.factorParameterPointIndex,
            value.factorParameterStartIndex,
            value.factorParameterEndIndex,
          ]);
          parameter(deps, value.parameterName);
          expr(deps, value.factorExpr);
          break;
      }
    }

    function pointBinding(deps: Set<string>, value: RuntimePointBindingJson | null | undefined) {
      if (!value) return;
      switch (value.kind) {
        case "graph-calibration":
          break;
        case "parameter":
          parameter(deps, value.name);
          break;
        case "derived-parameter":
          points(deps, [value.sourceIndex, value.parameterStartIndex, value.parameterEndIndex]);
          break;
        case "constraint-parameter-expr":
          expr(deps, value.expr);
          break;
        case "constraint-parameter-from-point-expr":
          point(deps, value.sourceIndex);
          parameter(deps, value.parameterName);
          expr(deps, value.expr);
          break;
        case "derived":
          point(deps, value.sourceIndex);
          pointTransform(deps, value.transform);
          break;
        case "scale-by-ratio":
          points(deps, [
            value.sourceIndex,
            value.centerIndex,
            value.ratioOriginIndex,
            value.ratioDenominatorIndex,
            value.ratioNumeratorIndex,
          ]);
          break;
        case "midpoint":
          points(deps, [value.startIndex, value.endIndex]);
          break;
        case "circumcenter":
          points(deps, [value.startIndex, value.midIndex, value.endIndex]);
          break;
        case "coordinate":
          expr(deps, value.expr);
          break;
        case "coordinate-source":
          point(deps, value.sourceIndex);
          expr(deps, value.expr);
          break;
        case "coordinate-source-2d":
          point(deps, value.sourceIndex);
          expr(deps, value.xExpr);
          expr(deps, value.yExpr);
          break;
        case "polar-offset":
          point(deps, value.sourceIndex);
          expr(deps, value.distanceExpr);
          break;
        case "custom-transform":
          points(deps, [value.sourceIndex, value.originIndex, value.axisEndIndex]);
          expr(deps, value.distanceExpr);
          expr(deps, value.angleExpr);
          break;
        case "rotate":
          points(deps, [value.sourceIndex, value.centerIndex]);
          expr(deps, value.angleExpr);
          break;
      }
    }

    function pointConstraint(deps: Set<string>, value: RuntimePointConstraintJson | null | undefined) {
      if (!value) return;
      switch (value.kind) {
        case "offset":
          point(deps, value.originIndex);
          break;
        case "segment":
        case "line":
        case "ray":
          points(deps, [value.startIndex, value.endIndex]);
          break;
        case "line-constraint":
        case "ray-constraint":
          lineConstraint(deps, value.line);
          break;
        case "polyline":
          break;
        case "polygon-boundary":
          points(deps, value.vertexIndices);
          break;
        case "translated-polygon-boundary":
          points(deps, value.vertexIndices);
          points(deps, [value.vectorStartIndex, value.vectorEndIndex]);
          break;
        case "circle":
          points(deps, [value.centerIndex, value.radiusIndex]);
          break;
        case "circular-constraint":
          circularConstraint(deps, value.circle);
          break;
        case "circle-arc":
          points(deps, [value.centerIndex, value.startIndex, value.endIndex]);
          break;
        case "arc":
          points(deps, [value.startIndex, value.midIndex, value.endIndex]);
          break;
        case "line-intersection":
          lineConstraint(deps, value.left);
          lineConstraint(deps, value.right);
          break;
        case "line-trace-intersection":
          lineConstraint(deps, value.line);
          point(deps, value.pointIndex);
          break;
        case "line-function-intersection":
          lineConstraint(deps, value.line);
          expr(deps, value.expr);
          break;
        case "point-circular-tangent":
          point(deps, value.pointIndex);
          circularConstraint(deps, value.circle);
          break;
        case "line-circular-intersection":
          lineConstraint(deps, value.line);
          circularConstraint(deps, value.circle);
          break;
        case "line-circle-intersection":
          lineConstraint(deps, value.line);
          points(deps, [value.centerIndex, value.radiusIndex]);
          break;
        case "circle-circle-intersection":
          points(deps, [
            value.leftCenterIndex,
            value.leftRadiusIndex,
            value.rightCenterIndex,
            value.rightRadiusIndex,
          ]);
          break;
        case "circular-intersection":
          circularConstraint(deps, value.left);
          circularConstraint(deps, value.right);
          break;
      }
    }

    function lineBinding(deps: Set<string>, value: RuntimeLineBindingJson | null | undefined) {
      if (!value) return;
      switch (value.kind) {
        case "graph-helper-line":
        case "segment":
        case "segment-marker":
        case "line":
        case "ray":
          points(deps, [value.startIndex, value.endIndex]);
          break;
        case "angle-marker":
        case "angle-bisector-ray":
          points(deps, [value.startIndex, value.vertexIndex, value.endIndex]);
          break;
        case "perpendicular-line":
        case "parallel-line":
          point(deps, value.throughIndex);
          points(deps, [value.lineStartIndex, value.lineEndIndex]);
          line(deps, value.lineIndex);
          break;
        case "derived":
          line(deps, value.sourceIndex);
          transform(deps, value.transform);
          break;
        case "custom-transform-trace":
        case "point-trace":
          points(deps, [value.pointIndex, value.driverIndex]);
          break;
        case "coordinate-trace":
          point(deps, value.pointIndex);
          break;
        case "segment-trace":
          points(deps, [value.startIndex, value.endIndex, value.driverIndex]);
          break;
        case "colorized-spectrum":
          lines(deps, [value.lineIndex, value.traceLineIndex, value.reflectionAxisLineIndex, value.reflectionDirectrixLineIndex]);
          points(deps, [value.pointIndex, value.traceEndpointIndex, value.reflectionSourceIndex, value.reflectionFocusIndex]);
          parameter(deps, value.depthParameterName);
          break;
        case "parametric-curve":
          expr(deps, value.xExpr);
          expr(deps, value.yExpr);
          break;
        case "arc-boundary":
          points(deps, [value.centerIndex, value.startIndex, value.midIndex, value.endIndex]);
          break;
      }
    }

    function lines(deps: Set<string>, indices: readonly (number | null | undefined)[]) {
      indices.forEach((index) => line(deps, index));
    }

    function shapeBinding(
      deps: Set<string>,
      value: RuntimeShapeBindingJson | null | undefined,
      sourceKind: "circle" | "polygon",
    ) {
      if (!value) return;
      switch (value.kind) {
        case "point-radius-circle":
          points(deps, [value.centerIndex, value.radiusIndex]);
          break;
        case "point-polygon":
          points(deps, value.vertexIndices);
          break;
        case "arc-boundary-polygon":
          points(deps, [value.centerIndex, value.startIndex, value.midIndex, value.endIndex]);
          break;
        case "segment-radius-circle":
          points(deps, [value.centerIndex, value.lineStartIndex, value.lineEndIndex]);
          break;
        case "parameter-radius-circle":
          point(deps, value.centerIndex);
          parameter(deps, value.parameterName);
          break;
        case "expression-radius-circle":
          point(deps, value.centerIndex);
          expr(deps, value.expr);
          break;
        case "derived":
          if (sourceKind === "circle") circle(deps, value.sourceIndex);
          else polygon(deps, value.sourceIndex);
          transform(deps, value.transform);
          break;
      }
    }

    function colorBinding(deps: Set<string>, value: ColorBindingJson | null | undefined) {
      if (!value) return;
      switch (value.kind) {
        case "spectrum":
          point(deps, value.pointIndex);
          break;
        case "rgb":
          points(deps, [value.redPointIndex, value.greenPointIndex, value.bluePointIndex]);
          break;
        case "hsb":
          points(deps, [value.huePointIndex, value.saturationPointIndex, value.brightnessPointIndex]);
          break;
      }
    }

    function richTextRef(deps: Set<string>, value: RichTextExpressionRefJson) {
      switch (value.kind) {
        case "expression":
          expr(deps, value.expr);
          break;
        case "parameter":
          parameter(deps, value.name);
          break;
        case "iteration-state":
          value.stateParameterNames.forEach((name) => parameter(deps, name));
          value.stateExprs.forEach((stateExpr) => expr(deps, stateExpr));
          expr(deps, value.depthExpr);
          break;
      }
    }

    function labelBinding(deps: Set<string>, value: RuntimeLabelBindingJson | null | undefined) {
      if (!value) return;
      switch (value.kind) {
        case "parameter-value":
          parameter(deps, value.name);
          break;
        case "expression-value":
          parameter(deps, value.parameterName);
          expr(deps, value.expr);
          break;
        case "point-bound-expression-value":
          point(deps, value.pointIndex);
          parameter(deps, value.parameterName);
          expr(deps, value.expr);
          break;
        case "point-anchor":
          points(deps, [value.pointIndex, value.anchorYPointIndex]);
          break;
        case "point-expression-value":
          points(deps, [value.pointIndex, value.anchorYPointIndex]);
          parameter(deps, value.parameterName);
          expr(deps, value.expr);
          break;
        case "sequence-expression-value":
          parameter(deps, value.parameterName);
          parameter(deps, value.depthParameterName);
          expr(deps, value.expr);
          break;
        case "rich-text-expression-values":
          value.refs?.forEach((ref) => richTextRef(deps, ref));
          break;
        case "point-coordinate-value":
          points(deps, [value.pointIndex, value.originIndex, value.xUnitIndex, value.yUnitIndex]);
          break;
        case "point-distance-value":
          points(deps, [value.leftIndex, value.rightIndex]);
          break;
        case "point-angle-value":
        case "angle-marker-value":
          points(deps, [value.startIndex, value.vertexIndex, value.endIndex]);
          break;
        case "polygon-area-value":
          points(deps, value.pointIndices);
          break;
        case "point-distance-ratio-value":
          points(deps, [value.originIndex, value.denominatorIndex, value.numeratorIndex]);
          break;
        case "point-axis-value":
          points(deps, [value.pointIndex, value.originIndex, value.xUnitIndex, value.yUnitIndex]);
          break;
        case "polygon-boundary-parameter":
        case "polyline-parameter":
        case "circle-parameter":
          point(deps, value.pointIndex);
          break;
        case "line-projection-parameter":
          points(deps, [value.pointIndex, value.startIndex, value.endIndex]);
          break;
        case "custom-transform-value":
          point(deps, value.pointIndex);
          expr(deps, value.expr);
          break;
      }
    }

    function labelReferencedParameterNames(
      value: RuntimeLabelBindingJson | null | undefined,
      names: Set<string>,
    ) {
      if (!value) return;
      const addExprNames = (expression: FunctionExprJson | FunctionAstJson | null | undefined) =>
        collectExprParameterNames(expression, names);
      switch (value.kind) {
        case "parameter-value":
          names.add(value.name);
          break;
        case "expression-value":
        case "point-bound-expression-value":
        case "point-expression-value":
          names.add(value.parameterName);
          addExprNames(value.expr);
          break;
        case "sequence-expression-value":
          names.add(value.parameterName);
          if (value.depthParameterName) names.add(value.depthParameterName);
          addExprNames(value.expr);
          break;
        case "rich-text-expression-values":
          value.refs?.forEach((ref) => {
            if (ref.kind === "parameter") names.add(ref.name);
            if (ref.kind === "expression") addExprNames(ref.expr);
            if (ref.kind === "iteration-state") {
              ref.stateParameterNames.forEach((name) => names.add(name));
              ref.stateExprs.forEach(addExprNames);
              addExprNames(ref.depthExpr);
            }
          });
          break;
        case "custom-transform-value":
          addExprNames(value.expr);
          break;
        default:
          break;
      }
    }

    function pointIteration(deps: Set<string>, value: PointIterationJson) {
      switch (value.kind) {
        case "offset":
          point(deps, value.seedIndex);
          parameter(deps, value.parameterName);
          break;
        case "rotate-chain":
          points(deps, [value.seedIndex, value.centerIndex]);
          break;
        case "rotate":
          points(deps, [value.sourceIndex, value.centerIndex]);
          parameter(deps, value.parameterName);
          expr(deps, value.angleExpr);
          break;
        case "parameterized":
          point(deps, value.pointIndex);
          parameter(deps, value.depthParameterName);
          parameter(deps, value.traceParameterName);
          expr(deps, value.stepExpr);
          break;
      }
    }

    function lineIteration(deps: Set<string>, value: LineIterationJson) {
      switch (value.kind) {
        case "rotate":
          line(deps, value.sourceIndex);
          point(deps, value.centerIndex);
          parameter(deps, value.parameterName);
          parameter(deps, value.depthParameterName);
          expr(deps, value.angleExpr);
          break;
        case "translate":
          points(deps, [
            value.startIndex,
            value.endIndex,
            value.startControlIndex,
            value.endControlIndex,
            value.vectorStartIndex,
            value.vectorEndIndex,
          ]);
          parameter(deps, value.parameterName);
          expr(deps, value.depthExpr);
          break;
        case "affine":
          points(deps, [value.startIndex, value.endIndex, ...value.sourceTriangleIndices]);
          value.targetTriangle.forEach((handle) => pointHandle(deps, handle));
          break;
        case "branching":
          points(deps, [value.startIndex, value.endIndex]);
          value.targetSegments.forEach(([start, end]) => {
            pointHandle(deps, start);
            pointHandle(deps, end);
          });
          parameter(deps, value.parameterName);
          break;
        case "parameterized-point-trace":
          points(deps, [value.pointIndex, value.driverIndex]);
          parameter(deps, value.depthParameterName);
          parameter(deps, value.traceParameterName);
          expr(deps, value.stepExpr);
          break;
      }
    }

    function circleIteration(deps: Set<string>, value: CircleIterationJson) {
      circle(deps, value.sourceCircleIndex);
      points(deps, [value.sourceCenterIndex, value.sourceNextCenterIndex, ...value.vertexIndices]);
      parameter(deps, value.depthParameterName);
    }

    function polygonIteration(deps: Set<string>, value: PolygonIterationJson) {
      switch (value.kind) {
        case "translate":
          points(deps, value.vertexIndices);
          points(deps, [value.vectorStartIndex, value.vectorEndIndex]);
          parameter(deps, value.parameterName);
          expr(deps, value.depthExpr);
          break;
        case "coordinate-grid":
          points(deps, value.vertexIndices);
          parameter(deps, value.parameterName);
          expr(deps, value.stepExpr);
          expr(deps, value.xExpr);
          expr(deps, value.yExpr);
          expr(deps, value.depthExpr);
          break;
      }
    }

    function labelIteration(deps: Set<string>, value: LabelIterationJson) {
      deps.add(`label:${value.seedLabelIndex}`);
      parameter(deps, value.parameterName);
      parameter(deps, value.depthParameterName);
      expr(deps, value.expr);
      if (value.kind === "point-expression") {
        point(deps, value.pointSeedIndex);
      } else {
        points(deps, [value.vectorStartIndex, value.vectorEndIndex]);
        expr(deps, value.depthExpr);
      }
    }

    function iterationTable(deps: Set<string>, value: IterationTableJson) {
      parameter(deps, value.parameterName);
      parameter(deps, value.depthParameterName);
      expr(deps, value.expr);
      expr(deps, value.depthExpr);
      value.columns.forEach((column) => {
        parameter(deps, column.parameterName);
        expr(deps, column.expr);
        if (column.valueBinding?.kind === "angle-marker") {
          points(deps, [
            column.valueBinding.startIndex,
            column.valueBinding.vertexIndex,
            column.valueBinding.endIndex,
          ]);
        }
      });
    }

    return {
      expr,
      points,
      pointBinding,
      pointConstraint,
      lineBinding,
      shapeBinding,
      colorBinding,
      labelBinding,
      labelReferencedParameterNames,
      pointIteration,
      lineIteration,
      circleIteration,
      polygonIteration,
      labelIteration,
      iterationTable,
    };
  }

  function createPointDependencyOrder(
    sourceScene: SceneData | ViewerSceneData,
    knownParameters: Set<string>,
  ) {
    const collect = createSceneDependencyCollector({
      sourceScene,
      knownParameters,
      collectExprParameterNames(expr, names) {
        if (!expr) return;
        window.GspRuntimeCore.expressionParameterNames(expr).forEach((name) => names.add(name));
      },
    });
    const nodes = (sourceScene.points || []).map((point, pointIndex) => {
      const deps = new Set<string>();
      collect.pointBinding(deps, point.binding);
      collect.pointConstraint(deps, point.constraint);
      return {
        id: `point:${pointIndex}`,
        dependsOn: [...deps].filter((dependency) => dependency.startsWith("point:")),
      };
    });
    return window.GspRuntimeCore.createDependencyPlan(nodes).topoOrder;
  }

  modules.dynamicsDependencies = { createSceneDependencyCollector, createPointDependencyOrder };
})();
