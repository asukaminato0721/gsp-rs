(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function createDynamicsParameters(dependencies: RuntimeDynamicsParameterDependencies) {
    const {
      discreteIterationDepth,
      evaluateExpr,
      formatExpr,
      formatSequenceValue,
      isDiscreteIterationParameterName,
      labelParameterValueFromBinding,
      pointAngleValue,
      pointDistanceRatioValue,
      pointDistanceValue,
      pointIterationDepth,
      polygonAreaValue,
    } = dependencies;

    const derivedLabelOrderCache = new WeakMap<object, number[]>();

    function derivedParameterNames(binding: RuntimeLabelBindingJson) {
      switch (binding.kind) {
        case "line-projection-parameter":
        case "polyline-parameter":
        case "polygon-boundary-parameter":
        case "circle-parameter":
          return typeof binding.pointName === "string" ? [binding.pointName] : [];
        case "point-distance-value":
        case "point-angle-value":
        case "polygon-area-value":
        case "point-distance-ratio-value":
        case "point-axis-value":
          return [binding.name];
        case "expression-value":
        case "point-bound-expression-value":
          return [...new Set([
            binding.resultName,
            binding.exprLabel,
            formatExpr(binding.expr, formatSequenceValue),
          ].filter((name): name is string => typeof name === "string" && name.length > 0))];
        default:
          return [];
      }
    }

    function orderedDerivedLabelIndices(scene: ViewerSceneData) {
      const cached = derivedLabelOrderCache.get(scene);
      if (cached) return cached;
      const derivedLabels = scene.labels.flatMap((label, labelIndex) => {
        if (!label.binding) return [];
        const outputNames = derivedParameterNames(label.binding);
        return outputNames.length > 0 ? [{ labelIndex, binding: label.binding, outputNames }] : [];
      });
      const producers = new Map<string, string>();
      derivedLabels.forEach((entry, index) => {
        entry.outputNames.forEach((name) => {
          // Payload order is authoritative: when labels export the same alias,
          // the later label is the value visible after one complete evaluation.
          producers.set(name, `derived-label:${index}`);
        });
      });
      const nodes = derivedLabels.map((entry, index) => {
        const referencedNames = entry.binding.kind === "expression-value"
          || entry.binding.kind === "point-bound-expression-value"
          ? window.GspRuntimeCore.expressionParameterNames(entry.binding.expr)
          : [];
        return {
          id: `derived-label:${index}`,
          dependsOn: referencedNames.flatMap((name) => {
            const producer = producers.get(name);
            return producer ? [producer] : [];
          }),
        };
      });
      const plan = window.GspRuntimeCore.createDependencyPlan(nodes);
      const ordered = plan.topoOrder.map((index) => derivedLabels[index].labelIndex);
      derivedLabelOrderCache.set(scene, ordered);
      return ordered;
    }

    function deriveExpressionLabelParameters(
      scene: ViewerSceneData | null | undefined,
      seedParameters: Map<string, number>,
    ) {
      const parameters = new Map(seedParameters);
      if (!scene?.labels?.length) return parameters;

      orderedDerivedLabelIndices(scene).forEach((labelIndex) => {
          const label = scene.labels[labelIndex];
          const binding = label.binding;
          if (!binding) return;
          if (
            (binding.kind === "line-projection-parameter"
              || binding.kind === "polyline-parameter"
              || binding.kind === "polygon-boundary-parameter"
              || binding.kind === "circle-parameter")
            && typeof binding.pointName === "string"
          ) {
            const value = labelParameterValueFromBinding(scene, binding);
            const nextValue = isDiscreteIterationParameterName(scene, binding.pointName)
              ? discreteIterationDepth(value)
              : value;
            if (typeof nextValue === "number" && Number.isFinite(nextValue) && parameters.get(binding.pointName) !== nextValue) {
              parameters.set(binding.pointName, nextValue);
            }
            return;
          }
          if (binding.kind === "point-distance-value") {
            const value = pointDistanceValue(scene, binding);
            if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
              parameters.set(binding.name, value);
            }
            return;
          }
          if (binding.kind === "point-angle-value") {
            const value = pointAngleValue(scene, binding);
            if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
              parameters.set(binding.name, value);
            }
            return;
          }
          if (binding.kind === "polygon-area-value") {
            const value = polygonAreaValue(scene, binding);
            if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
              parameters.set(binding.name, value);
            }
            return;
          }
          if (binding.kind === "point-distance-ratio-value") {
            const value = pointDistanceRatioValue(scene, binding);
            if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
              parameters.set(binding.name, value);
            }
            return;
          }
          if (binding.kind === "point-axis-value") {
            const point = scene.points[binding.pointIndex];
            if (!point) return;
            const value = binding.axis === "vertical" ? point.y : point.x;
            if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
              parameters.set(binding.name, value);
            }
            return;
          }
          if (binding.kind === "expression-value" || binding.kind === "point-bound-expression-value") {
            const value = evaluateExpr(binding.expr, 0, parameters);
            if (typeof value !== "number" || !Number.isFinite(value)) return;
            const resultNames = new Set<string>();
            if (binding.resultName) resultNames.add(binding.resultName);
            if (binding.exprLabel) resultNames.add(binding.exprLabel);
            resultNames.add(formatExpr(binding.expr, formatSequenceValue));
            resultNames.forEach((resultName) => {
              if (resultName && parameters.get(resultName) !== value) {
                parameters.set(resultName, value);
              }
            });
          }
        });
      return parameters;
    }

    function deriveSequenceLabelParameters(
      scene: ViewerSceneData | null | undefined,
      seedParameters: Map<string, number>,
    ) {
      const sequenceLabels = (scene?.labels || []).filter(
        (label): label is RuntimeLabelJson & {
          binding: Extract<RuntimeLabelBindingJson, { kind: "sequence-expression-value" }>;
        } => label.binding?.kind === "sequence-expression-value",
      );
      if (sequenceLabels.length === 0) return seedParameters;
      const parameters = new Map(seedParameters);
      const maxDepth = Math.max(...sequenceLabels.map((label) => pointIterationDepth({
        depth: label.binding.depth,
        parameterName: label.binding.depthParameterName,
      }, parameters)));
      for (let step = 0; step <= maxDepth; step += 1) {
        const derived = deriveExpressionLabelParameters(scene, parameters);
        const updates: Array<[string, number]> = [];
        sequenceLabels.forEach((label) => {
          const binding = label.binding;
          const depth = pointIterationDepth({
            depth: binding.depth,
            parameterName: binding.depthParameterName,
          }, derived);
          if (step > depth) return;
          const value = evaluateExpr(binding.expr, 0, derived);
          if (typeof value === "number" && Number.isFinite(value)) {
            updates.push([binding.parameterName, value]);
          }
        });
        if (updates.length === 0) break;
        updates.forEach(([name, value]) => parameters.set(name, value));
      }
      return deriveExpressionLabelParameters(scene, parameters);
    }

    function deriveLabelParameters(
      scene: ViewerSceneData | null | undefined,
      seedParameters: Map<string, number>,
    ) {
      return deriveSequenceLabelParameters(scene, deriveExpressionLabelParameters(scene, seedParameters));
    }

    function parameterMapForScene(env: ViewerEnv, scene: ViewerSceneData) {
      return deriveLabelParameters(
        scene,
        new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value])),
      );
    }

    return {
      deriveExpressionLabelParameters,
      deriveLabelParameters,
      parameterMapForScene,
    };
  }

  modules.dynamicsParameters = { createDynamicsParameters };
})();
