(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  function evaluateExpr(expr: FunctionExprJson | FunctionAstJson, x: number, parameters: Map<string, number>): number | null {
    return window.GspRuntimeCore.evaluateExpr(expr, x, parameters);
  }
  modules.dynamicsExpression = { evaluateExpr };
})();
