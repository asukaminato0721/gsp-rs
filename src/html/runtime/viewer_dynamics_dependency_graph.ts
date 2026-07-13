(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  type DependencyRecipe =
    | "sync-base-dynamics"
    | "refresh-derived-points"
    | "rebuild-iteration-geometry"
    | "refresh-dynamic-labels";
  type DependencyNode = {
    id: string;
    kind: string;
    dependsOn: string[];
    recipe: DependencyRecipe | null;
  };
  type DependencyGraph = {
    nodes: DependencyNode[];
    nodeMap: Map<string, DependencyNode>;
    topoOrder: string[];
    reverseEdges: Map<string, string[]>;
  };

  function createDependencyGraphRuntime(dependencies: Record<string, Function>) {
    const {
      applyBaseDynamicUpdates,
      parameterMapForScene,
      refreshDerivedPoints,
      refreshDynamicLabels,
      refreshIterationGeometry,
    } = dependencies;
    const dependencyCollectorModule = modules.dynamicsDependencies;
    if (!dependencyCollectorModule) {
      throw new Error("viewer dynamics dependency collector is unavailable");
    }
    const { createSceneDependencyCollector } = dependencyCollectorModule;

    let dependencyGraphCache: DependencyGraph | null = null;

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

    const GRAPH_RECIPES: Record<DependencyRecipe, (env: ViewerEnv, scene: ViewerSceneData) => void> = {
      "sync-base-dynamics"(env, scene) {
        applyBaseDynamicUpdates(env, scene, parameterMapForScene(env, scene));
      },
      "refresh-derived-points"(env, scene) {
        refreshDerivedPoints(env, scene);
      },
      "rebuild-iteration-geometry"(env, scene) {
        refreshIterationGeometry(env, scene, parameterMapForScene(env, scene));
      },
      "refresh-dynamic-labels"(env, scene) {
        refreshDynamicLabels(env, scene);
      },
    };

    function collectExprParameterNames(
      expr: FunctionExprJson | FunctionAstJson | null | undefined,
      names: Set<string>,
    ) {
      if (!expr || typeof expr !== "object") return;
      if (expr.kind === "parsed") collectExprAstParameterNames(expr.expr, names);
      else if (expr.kind === "parameter" || expr.kind === "unary" || expr.kind === "binary") {
        collectExprAstParameterNames(expr, names);
      }
    }

    function collectExprAstParameterNames(expr: FunctionAstJson, names: Set<string>) {
      if (expr.kind === "parameter") {
        names.add(expr.name);
      } else if (expr.kind === "unary") {
        collectExprAstParameterNames(expr.expr, names);
      } else if (expr.kind === "binary") {
        collectExprAstParameterNames(expr.lhs, names);
        collectExprAstParameterNames(expr.rhs, names);
      }
    }

    function labelDerivedParameterName(label: RuntimeLabelJson) {
      const binding = label.binding;
      if (!binding) return null;
      switch (binding.kind) {
        case "point-distance-ratio-value":
        case "point-distance-value":
        case "point-angle-value":
        case "polygon-area-value":
        case "point-axis-value":
          return binding.name;
        case "line-projection-parameter":
        case "polyline-parameter":
        case "polygon-boundary-parameter":
        case "circle-parameter":
          return binding.pointName;
        case "expression-value":
        case "point-bound-expression-value":
          return binding.resultName;
        default:
          return null;
      }
    }

    function collectLabelDerivedParameterDeps(
      scene: SceneData | ViewerSceneData,
      knownParameters: Set<string>,
    ) {
      type Definition = {
        directDeps: Set<string>;
        referencedNames: Set<string>;
      };
      const definitions = new Map<string, Definition>();
      const directCollector = createSceneDependencyCollector({
        sourceScene: scene,
        knownParameters,
        collectExprParameterNames,
      });

      (scene.labels || []).forEach((label) => {
        const name = labelDerivedParameterName(label);
        if (!name || !label.binding) return;
        const definition = definitions.get(name) || {
          directDeps: new Set<string>(),
          referencedNames: new Set<string>(),
        };
        directCollector.labelBinding(definition.directDeps, label.binding);
        directCollector.labelReferencedParameterNames(label.binding, definition.referencedNames);
        definitions.set(name, definition);
      });

      const resolved = new Map<string, Set<string>>();
      const resolving: string[] = [];
      function resolve(name: string): Set<string> {
        const cached = resolved.get(name);
        if (cached) return cached;
        const cycleStart = resolving.indexOf(name);
        if (cycleStart >= 0) {
          const cycle = [...resolving.slice(cycleStart), name];
          throw new Error(`cyclic derived parameter dependency: ${cycle.join(" -> ")}`);
        }
        const definition = definitions.get(name);
        if (!definition) return new Set<string>();
        resolving.push(name);
        const deps = new Set(definition.directDeps);
        definition.referencedNames.forEach((referencedName) => {
          if (knownParameters.has(referencedName)) deps.add(parameterRootId(referencedName));
          resolve(referencedName).forEach((dep) => deps.add(dep));
        });
        resolving.pop();
        resolved.set(name, deps);
        return deps;
      }
      definitions.forEach((_, name) => resolve(name));
      return resolved;
    }

    function ensureDependencyGraph(env: ViewerEnv): DependencyGraph {
      if (dependencyGraphCache) return dependencyGraphCache;

      const nodes: DependencyNode[] = [];
      const nodeMap = new Map<string, DependencyNode>();
      const knownParameters = new Set(
        (env.currentDynamics().parameters || []).map((parameter) => parameter.name),
      );
      const derivedParameterDeps = collectLabelDerivedParameterDeps(env.sourceScene, knownParameters);
      const collect = createSceneDependencyCollector({
        sourceScene: env.sourceScene,
        knownParameters,
        derivedParameterDeps,
        collectExprParameterNames,
      });

      function addNode(node: DependencyNode) {
        const normalized = {
          ...node,
          dependsOn: [...new Set(node.dependsOn.filter((dep) => dep !== node.id))],
        };
        nodes.push(normalized);
        nodeMap.set(normalized.id, normalized);
      }

      (env.currentDynamics().parameters || []).forEach((parameter) => {
        addNode({ id: parameterRootId(parameter.name), kind: "parameter-root", dependsOn: [], recipe: null });
        addNode({
          id: `parameter-sync:${parameter.name}`,
          kind: "parameter-sync",
          dependsOn: [parameterRootId(parameter.name)],
          recipe: "sync-base-dynamics",
        });
      });
      (env.sourceScene.points || []).forEach((_, index) =>
        addNode({ id: sourcePointRootId(index), kind: "source-point", dependsOn: [], recipe: null }));
      (env.sourceScene.lines || []).forEach((_, index) =>
        addNode({ id: sourceLineRootId(index), kind: "source-line", dependsOn: [], recipe: null }));
      (env.sourceScene.circles || []).forEach((_, index) =>
        addNode({ id: sourceCircleRootId(index), kind: "source-circle", dependsOn: [], recipe: null }));
      (env.sourceScene.polygons || []).forEach((_, index) =>
        addNode({ id: sourcePolygonRootId(index), kind: "source-polygon", dependsOn: [], recipe: null }));

      (env.sourceScene.points || []).forEach((point, index) => {
        if (!point.binding && !point.constraint) return;
        const deps = new Set<string>();
        collect.pointBinding(deps, point.binding);
        collect.pointConstraint(deps, point.constraint);
        addNode({ id: `point:${index}`, kind: "point", dependsOn: [...deps], recipe: "refresh-derived-points" });
      });
      (env.sourceScene.lines || []).forEach((line, index) => {
        if (!line.binding) return;
        const deps = new Set<string>();
        collect.lineBinding(deps, line.binding);
        if (line.binding.kind === "point-trace") {
          [line.binding.pointIndex, line.binding.driverIndex].forEach((pointIndex) => {
            const source = env.sourceScene.points?.[pointIndex];
            collect.pointBinding(deps, source?.binding);
            collect.pointConstraint(deps, source?.constraint);
          });
        }
        addNode({ id: `line:${index}`, kind: "line", dependsOn: [...deps], recipe: "refresh-derived-points" });
      });
      (env.sourceScene.circles || []).forEach((circle, index) => {
        if (!circle.binding && !circle.fillColorBinding) return;
        const deps = new Set<string>();
        collect.shapeBinding(deps, circle.binding, "circle");
        collect.colorBinding(deps, circle.fillColorBinding);
        addNode({ id: `circle:${index}`, kind: "circle", dependsOn: [...deps], recipe: "refresh-derived-points" });
      });
      (env.sourceScene.polygons || []).forEach((polygon, index) => {
        if (!polygon.binding && !polygon.colorBinding) return;
        const deps = new Set<string>();
        collect.shapeBinding(deps, polygon.binding, "polygon");
        collect.colorBinding(deps, polygon.colorBinding);
        addNode({ id: `polygon:${index}`, kind: "polygon", dependsOn: [...deps], recipe: "refresh-derived-points" });
      });
      (env.currentDynamics().functions || []).forEach((functionDef, index) => {
        const deps = new Set<string>();
        collect.expr(deps, functionDef.expr);
        collect.points(deps, functionDef.constrainedPointIndices);
        addNode({ id: `function:${index}`, kind: "function", dependsOn: [...deps], recipe: "sync-base-dynamics" });
      });
      (env.sourceScene.labels || []).forEach((label, index) => {
        if (!label.binding) return;
        const deps = new Set<string>();
        collect.labelBinding(deps, label.binding);
        addNode({ id: `label:${index}`, kind: "label", dependsOn: [...deps], recipe: "refresh-dynamic-labels" });
      });
      (env.sourceScene.pointIterations || []).forEach((family, index) => {
        const deps = new Set<string>();
        collect.pointIteration(deps, family);
        addNode({ id: `point-iteration:${index}`, kind: "point-iteration", dependsOn: [...deps], recipe: "rebuild-iteration-geometry" });
      });
      (env.sourceScene.circleIterations || []).forEach((family, index) => {
        const deps = new Set<string>();
        collect.circleIteration(deps, family);
        addNode({ id: `circle-iteration:${index}`, kind: "circle-iteration", dependsOn: [...deps], recipe: "rebuild-iteration-geometry" });
      });
      (env.sourceScene.lineIterations || []).forEach((family, index) => {
        const deps = new Set<string>();
        collect.lineIteration(deps, family);
        addNode({ id: `line-iteration:${index}`, kind: "line-iteration", dependsOn: [...deps], recipe: "rebuild-iteration-geometry" });
      });
      (env.sourceScene.polygonIterations || []).forEach((family, index) => {
        const deps = new Set<string>();
        collect.polygonIteration(deps, family);
        addNode({ id: `polygon-iteration:${index}`, kind: "polygon-iteration", dependsOn: [...deps], recipe: "rebuild-iteration-geometry" });
      });
      (env.sourceScene.labelIterations || []).forEach((family, index) => {
        const deps = new Set<string>();
        collect.labelIteration(deps, family);
        addNode({ id: `label-iteration:${index}`, kind: "label-iteration", dependsOn: [...deps], recipe: "rebuild-iteration-geometry" });
      });
      (env.sourceScene.iterationTables || []).forEach((table, index) => {
        const deps = new Set<string>();
        collect.iterationTable(deps, table);
        addNode({ id: `iteration-table:${index}`, kind: "iteration-table", dependsOn: [...deps], recipe: "rebuild-iteration-geometry" });
      });

      const indegree = new Map(nodes.map((node) => [node.id, 0]));
      const reverseEdges = new Map<string, string[]>();
      nodes.forEach((node) => {
        node.dependsOn.forEach((dep) => {
          if (!nodeMap.has(dep)) return;
          indegree.set(node.id, (indegree.get(node.id) || 0) + 1);
          const dependents = reverseEdges.get(dep) || [];
          dependents.push(node.id);
          reverseEdges.set(dep, dependents);
        });
      });
      const queue = nodes.filter((node) => indegree.get(node.id) === 0).map((node) => node.id);
      const topoOrder: string[] = [];
      while (queue.length > 0) {
        const id = queue.shift();
        if (!id) break;
        topoOrder.push(id);
        (reverseEdges.get(id) || []).forEach((dependentId) => {
          const nextDegree = (indegree.get(dependentId) || 0) - 1;
          indegree.set(dependentId, nextDegree);
          if (nextDegree === 0) queue.push(dependentId);
        });
      }
      if (topoOrder.length !== nodes.length) {
        const cyclicNodes = nodes
          .filter((node) => (indegree.get(node.id) || 0) > 0)
          .map((node) => node.id);
        throw new Error(`cyclic scene dependency graph: ${cyclicNodes.join(", ")}`);
      }

      dependencyGraphCache = { nodes, nodeMap, topoOrder, reverseEdges };
      return dependencyGraphCache;
    }

    function describeDependencyGraph(env: ViewerEnv) {
      const graph = ensureDependencyGraph(env);
      return graph.topoOrder.map((id) => graph.nodeMap.get(id)).filter((node): node is DependencyNode => !!node).map((node) => ({
        id: node.id,
        kind: node.kind,
        dependsOn: [...node.dependsOn],
        recipe: node.recipe,
      }));
    }

    function runDependencyGraph(env: ViewerEnv, scene: ViewerSceneData, dirtyRootIds: string[]) {
      const graph = ensureDependencyGraph(env);
      const rootSet = new Set(dirtyRootIds.filter((rootId) => graph.nodeMap.has(rootId)));
      if (rootSet.size === 0) {
        env.currentDynamics().parameters.forEach((parameter) => rootSet.add(parameterRootId(parameter.name)));
      }
      if (rootSet.size === 0) {
        env.sourceScene.points.forEach((_, index) => rootSet.add(sourcePointRootId(index)));
        env.sourceScene.lines.forEach((_, index) => rootSet.add(sourceLineRootId(index)));
        env.sourceScene.circles.forEach((_, index) => rootSet.add(sourceCircleRootId(index)));
        env.sourceScene.polygons.forEach((_, index) => rootSet.add(sourcePolygonRootId(index)));
      }
      const affected = new Set(rootSet);
      const queue = [...rootSet];
      while (queue.length > 0) {
        const currentId = queue.shift();
        if (!currentId) break;
        (graph.reverseEdges.get(currentId) || []).forEach((dependentId) => {
          if (affected.has(dependentId)) return;
          affected.add(dependentId);
          queue.push(dependentId);
        });
      }
      const orderedNodes = graph.topoOrder.flatMap((id) => {
        const node = graph.nodeMap.get(id);
        return node && affected.has(id) ? [node] : [];
      });
      const executedRecipes: string[] = [];
      const seenRecipes = new Set<DependencyRecipe>();
      orderedNodes.forEach((node) => {
        if (!node.recipe || seenRecipes.has(node.recipe)) return;
        seenRecipes.add(node.recipe);
        executedRecipes.push(node.recipe);
        GRAPH_RECIPES[node.recipe](env, scene);
      });
      if (seenRecipes.has("refresh-dynamic-labels") && env.sourceScene.labelIterations.length > 0) {
        refreshIterationGeometry(env, scene, parameterMapForScene(env, scene));
        executedRecipes.push("rebuild-label-iteration-anchors");
      }
      return {
        dirtyRoots: [...rootSet],
        affectedNodes: orderedNodes.map((node) => ({ ...node, dependsOn: [...node.dependsOn] })),
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
