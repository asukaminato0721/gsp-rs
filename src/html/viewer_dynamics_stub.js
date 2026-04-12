// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  /** @returns {void} */
  function noop() {}

  modules.dynamics = {
    buildParameterControls(env) {
      env?.parameterControls?.replaceChildren?.();
    },
    evaluateExpr: null,
    formatExpr: null,
    parameterValueFromPoint: null,
    applyNormalizedParameterToPoint: noop,
    refreshDerivedPoints: noop,
    refreshDynamicLabels: noop,
    refreshIterationGeometry: noop,
    syncDynamicScene: noop,
  };
})();
