(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function createDependencyGraphRuntime(dependencies: Record<string, Function>) {
    const {
      applyBaseDynamicUpdates,
      parameterMapForScene,
      refreshDerivedPoints,
      refreshDynamicLabels,
      refreshIterationGeometry,
    } = dependencies;

  type DependencyNode = { id: string; kind: string; dependsOn: string[]; recipe: string | null };

  let dependencyGraphCache = null;


  function parameterRootId(name: string) {
    return `param:${name}`;
  }


  function sourcePointRootId(index: number) {
    return `source-point:${index}`;
  }


  function sourceLineRootId(index: number) {
    return `source-line:${index}`;
  }


  function sourceCircleRootId(index: number) {
    return `source-circle:${index}`;
  }


  function sourcePolygonRootId(index: number) {
    return `source-polygon:${index}`;
  }


  const GRAPH_RECIPES = {
    "sync-base-dynamics"(env: ViewerEnv, scene: ViewerSceneData) {
      applyBaseDynamicUpdates(env, scene, parameterMapForScene(env, scene));
    },
    "refresh-derived-points"(env: ViewerEnv, scene: ViewerSceneData) {
      refreshDerivedPoints(env, scene);
    },
    "rebuild-iteration-geometry"(env: ViewerEnv, scene: ViewerSceneData) {
      refreshIterationGeometry(env, scene, parameterMapForScene(env, scene));
    },
    "refresh-dynamic-labels"(env: ViewerEnv, scene: ViewerSceneData) {
      refreshDynamicLabels(env, scene);
    },
  };


  function addKnownParameterDep(deps: Set<string>, name: string | null | undefined, knownParameters: Set<string>) {
    if (typeof name === "string" && knownParameters.has(name)) {
      deps.add(parameterRootId(name));
    }
  }


  function addParameterDep(deps: Set<string>, name: string | null | undefined, knownParameters: Set<string>, derivedParameterDeps: Map<string, Set<string>>) {
    addKnownParameterDep(deps, name, knownParameters);
    if (typeof name !== "string") return;
    const derivedDeps = derivedParameterDeps.get(name);
    if (!derivedDeps) return;
    derivedDeps.forEach((dep: string) => deps.add(dep));
  }


  function addExprParameterDeps(deps: Set<string>, expr: FunctionExprJson | FunctionAstJson | null | undefined, knownParameters: Set<string>, derivedParameterDeps: Map<string, Set<string>> = new Map()) {
    const names = new Set<string>();
    collectExprParameterNames(expr, names);
    names.forEach((name: string) => addParameterDep(deps, name, knownParameters, derivedParameterDeps));
  }


  function collectSceneDependencyIds(deps: Set<string>, value: unknown, knownParameters: Set<string>, derivedParameterDeps: Map<string, Set<string>> = new Map(), sourceScene: SceneData | ViewerSceneData | null = null) {
    if (!value || typeof value !== "object") {
      return;
    }
    if (Array.isArray(value)) {
      value.forEach((entry) => collectSceneDependencyIds(deps, entry, knownParameters, derivedParameterDeps, sourceScene));
      return;
    }
    const addPointRefDep = ( index: number) => {
      deps.add(sourcePointRootId(index));
      const point = sourceScene?.points?.[index];
      if (point?.binding || point?.constraint) {
        deps.add(`point:${index}`);
      }
    };
    const addLineRefDep = ( index: number) => {
      deps.add(sourceLineRootId(index));
      const line = sourceScene?.lines?.[index];
      if (line?.binding) {
        deps.add(`line:${index}`);
      }
    };
    const addCircleRefDep = ( index: number) => {
      deps.add(sourceCircleRootId(index));
      const circle = sourceScene?.circles?.[index];
      if (circle?.binding || circle?.fillColorBinding) {
        deps.add(`circle:${index}`);
      }
    };
    const addPolygonRefDep = ( index: number) => {
      deps.add(sourcePolygonRootId(index));
      const polygon = sourceScene?.polygons?.[index];
      if (polygon?.binding) {
        deps.add(`polygon:${index}`);
      }
    };
    Object.entries( (value)).forEach(([key, child]) => {
      if (key === "expr" && child && typeof child === "object") {
        addExprParameterDeps(
          deps,
           (child),
          knownParameters,
          derivedParameterDeps,
        );
        collectSceneDependencyIds(deps, child, knownParameters, derivedParameterDeps, sourceScene);
        return;
      }
      if (typeof child === "number") {
        if (
          key === "pointIndex"
          || key === "targetPointIndex"
          || key === "sourceIndex"
          || key === "centerIndex"
          || key === "originIndex"
          || key === "xUnitIndex"
          || key === "yUnitIndex"
          || key === "denominatorIndex"
          || key === "numeratorIndex"
          || key === "ratioOriginIndex"
          || key === "ratioDenominatorIndex"
          || key === "ratioNumeratorIndex"
          || key === "radiusIndex"
          || key === "startIndex"
          || key === "endIndex"
          || key === "leftIndex"
          || key === "rightIndex"
          || key === "midIndex"
          || key === "throughIndex"
          || key === "vertexIndex"
          || key === "lineStartIndex"
          || key === "lineEndIndex"
          || key === "sourceCenterIndex"
          || key === "sourceNextCenterIndex"
          || key === "reflectionSourceIndex"
          || key === "vectorStartIndex"
          || key === "vectorEndIndex"
          || key === "startControlIndex"
          || key === "endControlIndex"
          || key === "anchorYPointIndex"
          || key === "driverIndex"
          || key === "seedIndex"
          || key === "pointSeedIndex"
          || key === "angleParameterPointIndex"
          || key === "angleParameterStartIndex"
          || key === "angleParameterEndIndex"
          || key === "factorParameterPointIndex"
          || key === "factorParameterStartIndex"
          || key === "factorParameterEndIndex"
          || key === "reflectionFocusIndex"
        ) {
          addPointRefDep(child);
          return;
        }
        if (
          key === "lineIndex"
          || key === "traceLineIndex"
          || key === "reflectionAxisLineIndex"
          || key === "reflectionDirectrixLineIndex"
        ) {
          addLineRefDep(child);
          return;
        }
        if (key === "circleIndex" || key === "sourceCircleIndex") {
          addCircleRefDep(child);
          return;
        }
        if (key === "polygonIndex") {
          addPolygonRefDep(child);
        }
        return;
      }
      if (Array.isArray(child)) {
        if (key === "vertexIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addPointRefDep(entry);
            }
          });
        } else if (
          key === "pointIndices"
          || key === "constrainedPointIndices"
        ) {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addPointRefDep(entry);
            }
          });
        } else if (key === "lineIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addLineRefDep(entry);
            }
          });
        } else if (key === "circleIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addCircleRefDep(entry);
            }
          });
        } else if (key === "polygonIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addPolygonRefDep(entry);
            }
          });
        }
        child.forEach((entry) => collectSceneDependencyIds(deps, entry, knownParameters, derivedParameterDeps, sourceScene));
        return;
      }
      if (typeof child === "string") {
        if (
          key === "parameterName"
          || key === "depthParameterName"
          || key === "traceParameterName"
          || key === "pointName"
          || key === "name"
          || key === "resultName"
        ) {
          addParameterDep(deps, child, knownParameters, derivedParameterDeps);
        }
        return;
      }
      collectSceneDependencyIds(deps, child, knownParameters, derivedParameterDeps, sourceScene);
    });
  }


  function labelDerivedParameterName(label: RuntimeLabelJson) {
    const binding = label.binding;
    if (!binding) return null;
    if (
      (binding.kind === "point-distance-ratio-value"
        || binding.kind === "point-distance-value"
        || binding.kind === "point-angle-value"
        || binding.kind === "polygon-area-value"
        || binding.kind === "parameter-value"
        || binding.kind === "point-axis-value")
      && typeof binding.name === "string"
    ) {
      return binding.name;
    }
    if (
      (binding.kind === "line-projection-parameter"
        || binding.kind === "polyline-parameter"
        || binding.kind === "polygon-boundary-parameter"
        || binding.kind === "circle-parameter")
      && typeof binding.pointName === "string"
    ) {
      return binding.pointName;
    }
    if (
      (binding.kind === "expression-value" || binding.kind === "point-bound-expression-value")
      && typeof binding.resultName === "string"
    ) {
      return binding.resultName;
    }
    return null;
  }


  function collectLabelDerivedParameterDeps(scene: { labels?: RuntimeLabelJson[] }, knownParameters: Set<string>) {

    const defs = [];
    (scene.labels || []).forEach(( label) => {
      const name = labelDerivedParameterName(label);
      if (!name || !label.binding) return;

      const directDeps = new Set<string>();
      collectSceneDependencyIds(directDeps, label.binding, knownParameters);

      const exprNames = new Set<string>();
      if ("expr" in label.binding) {
        collectExprParameterNames(label.binding.expr, exprNames);
      }
      defs.push({ name, directDeps, exprNames });
    });


    const depsByName = new Map();
    defs.forEach((def) => depsByName.set(def.name, new Set<string>(def.directDeps)));
    for (let pass = 0; pass < 4; pass += 1) {
      let changed = false;
      defs.forEach((def) => {

        const deps = new Set<string>(def.directDeps);
        def.exprNames.forEach((name: string) => {
          addParameterDep(deps, name, knownParameters, depsByName);
        });
        const current = depsByName.get(def.name) || new Set<string>();
        deps.forEach((dep: string) => {
          if (!current.has(dep)) {
            current.add(dep);
            changed = true;
          }
        });
        depsByName.set(def.name, current);
      });
      if (!changed) break;
    }
    return depsByName;
  }


  function ensureDependencyGraph(env: ViewerEnv) {
    if (dependencyGraphCache) {
      return dependencyGraphCache;
    }

    const nodes = [];

    const nodeMap = new Map();
    const knownParameters = new Set<string>((env.currentDynamics().parameters || []).map((parameter) => parameter.name));
    const derivedParameterDeps = collectLabelDerivedParameterDeps(env.sourceScene, knownParameters);
    const collectDeps = ( deps,  value) => {
      collectSceneDependencyIds(deps, value, knownParameters, derivedParameterDeps, env.sourceScene);
    };


    const addNode = (node) => {
      const normalized = {
        ...node,
        dependsOn: [...new Set<string>((node.dependsOn || []).filter((dep: string) => dep !== node.id))],
      };
      nodes.push(normalized);
      nodeMap.set(normalized.id, normalized);
    };

    (env.currentDynamics().parameters || []).forEach((parameter) => {
      addNode({
        id: parameterRootId(parameter.name),
        kind: "parameter-root",
        dependsOn: [],
        recipe: null,
      });
      addNode({
        id: `parameter-sync:${parameter.name}`,
        kind: "parameter-sync",
        dependsOn: [parameterRootId(parameter.name)],
        recipe: "sync-base-dynamics",
      });
    });
    (env.sourceScene.points || []).forEach((_, index: number) => {
      addNode({ id: sourcePointRootId(index), kind: "source-point", dependsOn: [], recipe: null });
    });
    (env.sourceScene.lines || []).forEach((_, index: number) => {
      addNode({ id: sourceLineRootId(index), kind: "source-line", dependsOn: [], recipe: null });
    });
    (env.sourceScene.circles || []).forEach((_, index: number) => {
      addNode({ id: sourceCircleRootId(index), kind: "source-circle", dependsOn: [], recipe: null });
    });
    (env.sourceScene.polygons || []).forEach((_, index: number) => {
      addNode({ id: sourcePolygonRootId(index), kind: "source-polygon", dependsOn: [], recipe: null });
    });

    (env.sourceScene.points || []).forEach((point, index: number) => {
      if (!point.binding && !point.constraint) return;
      const deps = new Set<string>();
      collectDeps(deps, point.binding);
      collectDeps(deps, point.constraint);
      addNode({
        id: `point:${index}`,
        kind: "point",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.sourceScene.lines || []).forEach((line, index: number) => {
      if (!line.binding) return;
      const deps = new Set<string>();
      collectDeps(deps, line.binding);
      if (line.binding.kind === "point-trace") {
        [line.binding.pointIndex, line.binding.driverIndex].forEach(( pointIndex: number) => {
          const point = env.sourceScene.points?.[pointIndex];
          collectDeps(deps, point?.binding);
          collectDeps(deps, point?.constraint);
        });
      }
      addNode({
        id: `line:${index}`,
        kind: "line",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.sourceScene.circles || []).forEach((circle, index: number) => {
      if (!circle.binding && !circle.fillColorBinding) return;
      const deps = new Set<string>();
      collectDeps(deps, circle.binding);
      collectDeps(deps, circle.fillColorBinding);
      addNode({
        id: `circle:${index}`,
        kind: "circle",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.sourceScene.polygons || []).forEach((polygon, index: number) => {
      if (!polygon.binding && !polygon.colorBinding) return;
      const deps = new Set<string>();
      collectDeps(deps, polygon.binding);
      collectDeps(deps, polygon.colorBinding);
      addNode({
        id: `polygon:${index}`,
        kind: "polygon",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.currentDynamics().functions || []).forEach((functionDef, index: number) => {
      const deps = new Set<string>();
      addExprParameterDeps(deps, functionDef.expr, knownParameters, derivedParameterDeps);
      collectDeps(deps, functionDef.constrainedPointIndices);
      addNode({
        id: `function:${index}`,
        kind: "function",
        dependsOn: [...deps],
        recipe: "sync-base-dynamics",
      });
    });

    (env.sourceScene.labels || []).forEach((label, index: number) => {
      if (!label.binding) return;
      const deps = new Set<string>();
      collectDeps(deps, label.binding);
      if ("expr" in label.binding) {
        addExprParameterDeps(deps, label.binding.expr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `label:${index}`,
        kind: "label",
        dependsOn: [...deps],
        recipe: "refresh-dynamic-labels",
      });
    });

    (env.sourceScene.pointIterations || []).forEach((family, index: number) => {
      const deps = new Set<string>();
      collectDeps(deps, family);
      if (family.kind === "rotate") {
        addExprParameterDeps(deps, family.angleExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `point-iteration:${index}`,
        kind: "point-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.circleIterations || []).forEach((family, index: number) => {
      const deps = new Set<string>();
      collectDeps(deps, family);
      addNode({
        id: `circle-iteration:${index}`,
        kind: "circle-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.lineIterations || []).forEach((family, index: number) => {
      const deps = new Set<string>();
      collectDeps(deps, family);
      if (family.kind === "rotate") {
        addExprParameterDeps(deps, family.angleExpr, knownParameters, derivedParameterDeps);
      }
      if ("depthExpr" in family) {
        addExprParameterDeps(deps, family.depthExpr, knownParameters, derivedParameterDeps);
      }
      if (family.kind === "parameterized-point-trace") {
        addExprParameterDeps(deps, family.stepExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `line-iteration:${index}`,
        kind: "line-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.polygonIterations || []).forEach((family, index: number) => {
      const deps = new Set<string>();
      collectDeps(deps, family);
      if ("depthExpr" in family) {
        addExprParameterDeps(deps, family.depthExpr, knownParameters, derivedParameterDeps);
      }
      if (family.kind === "coordinate-grid") {
        addExprParameterDeps(deps, family.stepExpr, knownParameters, derivedParameterDeps);
        addExprParameterDeps(deps, family.xExpr, knownParameters, derivedParameterDeps);
        addExprParameterDeps(deps, family.yExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `polygon-iteration:${index}`,
        kind: "polygon-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.labelIterations || []).forEach((family, index: number) => {
      const deps = new Set<string>();
      collectDeps(deps, family);
      addExprParameterDeps(deps, family.expr, knownParameters, derivedParameterDeps);
      if ("depthExpr" in family) {
        addExprParameterDeps(deps, family.depthExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `label-iteration:${index}`,
        kind: "label-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.iterationTables || []).forEach((table, index: number) => {
      const deps = new Set<string>();
      collectDeps(deps, table);
      addExprParameterDeps(deps, table.expr, knownParameters, derivedParameterDeps);
      addNode({
        id: `iteration-table:${index}`,
        kind: "iteration-table",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });


    const indegree = new Map();

    const reverseEdges = new Map();
    nodes.forEach((node) => {
      indegree.set(node.id, 0);
    });
    nodes.forEach((node) => {
      node.dependsOn.forEach((dep: string) => {
        if (!nodeMap.has(dep)) {
          return;
        }
        indegree.set(node.id, (indegree.get(node.id) || 0) + 1);
        const dependents = reverseEdges.get(dep) || [];
        dependents.push(node.id);
        reverseEdges.set(dep, dependents);
      });
    });
    const queue = nodes
      .filter((node) => (indegree.get(node.id) || 0) === 0)
      .map((node) => node.id);

    const topoOrder = [];
    while (queue.length > 0) {
      const id =  (queue.shift());
      topoOrder.push(id);
      (reverseEdges.get(id) || []).forEach((dependentId: number) => {
        const nextDegree = (indegree.get(dependentId) || 0) - 1;
        indegree.set(dependentId, nextDegree);
        if (nextDegree === 0) {
          queue.push(dependentId);
        }
      });
    }
    nodes.forEach((node) => {
      if (!topoOrder.includes(node.id)) {
        topoOrder.push(node.id);
      }
    });

    dependencyGraphCache = { nodes, nodeMap, topoOrder, reverseEdges };
    return dependencyGraphCache;
  }


  function describeDependencyGraph(env: ViewerEnv) {
    const graph = ensureDependencyGraph(env);
    return graph.topoOrder
      .map((id: string) => graph.nodeMap.get(id))
      .filter(( node) => !!node)
      .map((node) => ({
        id: node.id,
        kind: node.kind,
        dependsOn: [...node.dependsOn],
        recipe: node.recipe,
      }));
  }


  function collectExprParameterNames(expr: FunctionExprJson | FunctionAstJson | null | undefined, names: Set<string>) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parsed") {
      collectExprAstParameterNames(expr.expr, names);
    }
  }


  function collectExprAstParameterNames(expr: FunctionExprJson | FunctionAstJson, names: Set<string>) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parameter" && typeof expr.name === "string") {
      names.add(expr.name);
      return;
    }
    if (expr.kind === "unary") {
      collectExprAstParameterNames(expr.expr, names);
      return;
    }
    if (expr.kind === "binary") {
      collectExprAstParameterNames(expr.lhs, names);
      collectExprAstParameterNames(expr.rhs, names);
    }
  }

  function runDependencyGraph(env: ViewerEnv, scene: ViewerSceneData, dirtyRootIds: string[]) {
    const graph = ensureDependencyGraph(env);
    const rootSet = new Set(
      (dirtyRootIds || []).filter((rootId: string) => typeof rootId === "string" && graph.nodeMap.has(rootId)),
    );
    if (rootSet.size === 0) {
      env.currentDynamics().parameters.forEach((parameter) => {
        rootSet.add(parameterRootId(parameter.name));
      });
    }
    if (rootSet.size === 0) {
      (env.sourceScene.points || []).forEach((_, index: number) => rootSet.add(sourcePointRootId(index)));
      (env.sourceScene.lines || []).forEach((_, index: number) => rootSet.add(sourceLineRootId(index)));
      (env.sourceScene.circles || []).forEach((_, index: number) => rootSet.add(sourceCircleRootId(index)));
      (env.sourceScene.polygons || []).forEach((_, index: number) => rootSet.add(sourcePolygonRootId(index)));
    }
    const affected = new Set<string>(rootSet);
    const queue = Array.from(rootSet);
    while (queue.length > 0) {
      const currentId =  (queue.shift());
      (graph.reverseEdges.get(currentId) || []).forEach((dependentId: string) => {
        if (!affected.has(dependentId)) {
          affected.add(dependentId);
          queue.push(dependentId);
        }
      });
    }

    const orderedNodes = graph.topoOrder
      .flatMap((id: string) => {
        const node = graph.nodeMap.get(id);
        return node && affected.has(node.id) ? [node] : [];
      });

    const executedRecipes = [];
    const seenRecipes = new Set<string>();
    orderedNodes.forEach((node) => {
      if (!node.recipe || seenRecipes.has(node.recipe)) {
        return;
      }
      seenRecipes.add(node.recipe);
      executedRecipes.push(node.recipe);
      const runRecipe = GRAPH_RECIPES[node.recipe];
      if (runRecipe) {
        runRecipe(env, scene);
      }
    });
    if (seenRecipes.has("refresh-dynamic-labels") && (env.sourceScene.labelIterations || []).length > 0) {
      refreshIterationGeometry(env, scene, parameterMapForScene(env, scene));
      executedRecipes.push("rebuild-label-iteration-anchors");
    }
    return {
      dirtyRoots: Array.from(rootSet),
      affectedNodes: orderedNodes.map((node) => ({
        id: node.id,
        kind: node.kind,
        dependsOn: [...node.dependsOn],
        recipe: node.recipe,
      })),
      executedRecipes,
    };
  }


    return {
      parameterRootId,
      sourcePointRootId,
      collectExprParameterNames,
      describeDependencyGraph,
      runDependencyGraph,
    };
  }

  modules.dynamicsDependencyGraph = { createDependencyGraphRuntime };
})();
