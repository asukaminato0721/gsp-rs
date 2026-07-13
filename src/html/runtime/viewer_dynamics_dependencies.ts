(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function createPointDependencyOrder(sourceScene: SceneData | ViewerSceneData) {
    const pointNodes = sourceScene.dependencyGraph.nodes
      .filter((node) => node.id.startsWith("point:"))
      .map((node) => ({
        id: node.id,
        dependsOn: node.dependsOn.filter((dependency) => dependency.startsWith("point:")),
      }));
    const plan = window.GspRuntimeCore.createDependencyPlan(pointNodes);
    return plan.topoOrder.map((index) => Number(pointNodes[index].id.slice("point:".length)));
  }

  modules.dynamicsDependencies = { createPointDependencyOrder };
})();
