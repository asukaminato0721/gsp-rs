(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  type DependencyRecipe = NonNullable<SceneData["dependencyGraph"]["nodes"][number]["recipe"]>;
  type DependencyNode = SceneData["dependencyGraph"]["nodes"][number];
  type DependencyGraph = {
    nodes: DependencyNode[];
    nodeMap: Map<string, DependencyNode>;
    topoOrder: string[];
    plan: {
      affected: (dirtyRootIds: string[]) => number[];
    };
  };

  function createDependencyGraphRuntime(dependencies: Record<string, Function>) {
    const {
      applyBaseDynamicUpdates,
      parameterMapForScene,
      refreshDerivedPoints,
      refreshDynamicLabels,
      refreshIterationGeometry,
    } = dependencies;
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

    function ensureDependencyGraph(env: ViewerEnv): DependencyGraph {
      if (dependencyGraphCache) return dependencyGraphCache;
      const exported = env.sourceScene.dependencyGraph;
      if (!exported || !Array.isArray(exported.nodes)) {
        throw new Error("scene-data is missing its Rust-generated dependency graph");
      }
      const nodes = exported.nodes.map((node) => ({
        ...node,
        dependsOn: [...node.dependsOn],
      }));
      const nodeMap = new Map(nodes.map((node) => [node.id, node]));
      const plan = window.GspRuntimeCore.createDependencyPlan(nodes);
      const topoOrder = plan.topoOrder.map((index) => nodes[index].id);
      dependencyGraphCache = { nodes, nodeMap, topoOrder, plan };
      return dependencyGraphCache;
    }

    function describeDependencyGraph(env: ViewerEnv) {
      const graph = ensureDependencyGraph(env);
      return graph.topoOrder
        .map((id) => graph.nodeMap.get(id))
        .filter((node): node is DependencyNode => !!node)
        .map((node) => ({ ...node, dependsOn: [...node.dependsOn] }));
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
      const orderedNodes = graph.plan
        .affected([...rootSet])
        .map((index) => graph.nodes[index])
        .filter((node): node is DependencyNode => !!node);
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
      describeDependencyGraph,
      runDependencyGraph,
    };
  }

  modules.dynamicsDependencyGraph = { createDependencyGraphRuntime };
})();
