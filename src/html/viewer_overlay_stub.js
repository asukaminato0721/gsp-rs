// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  modules.overlay = {
    /**
     * @param {ViewerEnv} _env
     * @param {HTMLElement | null} buttonOverlays
     * @returns {ViewerOverlayRuntime}
     */
    init(_env, buttonOverlays) {
      return {
        /** @returns {RuntimeButtonJson[]} */
        currentButtons() {
          return [];
        },
        /** @returns {HotspotFlash[]} */
        currentHotspotFlashes() {
          return [];
        },
        render() {
          buttonOverlays?.replaceChildren?.();
        },
      };
    },
  };
})();
