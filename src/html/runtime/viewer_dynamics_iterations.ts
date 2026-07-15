(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function createDynamicsIterations(dependencies: Record<string, any>) {
    const {
      buildPlainTextRichMarkup,
      deriveExpressionLabelParameters,
      discreteIterationDepth,
      evaluateExpr,
      formatSequenceValue,
      isFiniteNumber,
      pointAngleValue,
      pointIterationDepth,
    } = dependencies;
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
        const currentValue = parameters.get(family.parameterName);
        if (!seedAnchor || !isFiniteNumber(currentValue)) {
          return;
        }
        const seedAnchorPoint = env.resolvePoint(seedAnchor);
        if (!seedAnchorPoint) {
          return;
        }
        const dx = vectorEnd.x - vectorStart.x;
        const dy = vectorEnd.y - vectorStart.y;
        const values = window.GspRuntimeCore.iterateExpression(
          family.expr,
          family.parameterName,
          currentValue,
          parameters,
          depth + 1,
        );
        values.slice(1).forEach((value, valueIndex) => {
          const step = valueIndex + 1;
          const text = formatSequenceValue(value);
          scene.labels.push({
            ...seedLabel,
            text,
            richMarkup: buildPlainTextRichMarkup(text),
            binding: null,
            anchor: { x: seedAnchorPoint.x + dx * step, y: seedAnchorPoint.y + dy * step },
          });
        });
        return;
      }
      const seedLabel = scene.labels[family.seedLabelIndex];
      const seedAnchor = seedLabel?.anchor;
      const seedPointIndex = seedAnchor && "pointIndex" in seedAnchor && typeof seedAnchor.pointIndex === "number"
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
      const currentValue = parameters.get(family.parameterName);
      if (!isFiniteNumber(currentValue)) {
        return;
      }
      const values = window.GspRuntimeCore.iterateExpression(
        family.expr,
        family.parameterName,
        currentValue,
        parameters,
        depth + 1,
      );
      for (let step = 0; step < values.length; step += 1) {
        const value = values[step];
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
      rebuildIteratedLabels,
      rebuildIterationTables,
    };
  }

  modules.dynamicsIterations = { createDynamicsIterations };
})();
