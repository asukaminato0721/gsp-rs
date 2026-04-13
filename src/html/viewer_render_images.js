// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  /** @type {Map<string, { img: HTMLImageElement; loaded: boolean }>} */
  const imageCache = new Map();

  /**
   * @param {string} src
   * @param {ViewerEnv | null | undefined} env
   */
  function loadImage(src, env) {
    let entry = imageCache.get(src);
    if (entry) return entry;
    const img = new Image();
    entry = { img, loaded: false };
    img.onload = () => {
      entry.loaded = true;
      if (env?.sceneLayer) {
        requestAnimationFrame(() => modules.render.draw(env));
      }
    };
    img.src = src;
    imageCache.set(src, entry);
    return entry;
  }

  /** @param {ViewerEnv} env */
  modules.render.drawImages = function drawImages(env) {
    for (const image of env.currentScene().images || []) {
      const entry = loadImage(image.src, env);
      if (!entry.loaded) continue;

      const topLeft = image.screenSpace ? image.topLeft : env.toScreen(image.topLeft);
      const bottomRight = image.screenSpace ? image.bottomRight : env.toScreen(image.bottomRight);
      if (!topLeft || !bottomRight) continue;

      const left = Math.min(topLeft.x, bottomRight.x);
      const top = Math.min(topLeft.y, bottomRight.y);
      const width = Math.abs(bottomRight.x - topLeft.x);
      const height = Math.abs(bottomRight.y - topLeft.y);
      if (width <= 1e-6 || height <= 1e-6) continue;

      modules.render.appendSceneElement(env, "image", {
        x: left,
        y: top,
        width,
        height,
        href: image.src,
        preserveAspectRatio: "none",
      });
    }
  };

  /**
   * @param {ViewerEnv} env
   * @param {number} screenX
   * @param {number} screenY
   * @returns {number | null}
   */
  modules.render.findHitImage = function findHitImage(env, screenX, screenY) {
    const images = env.currentScene().images || [];
    for (let index = images.length - 1; index >= 0; index -= 1) {
      const image = images[index];
      const topLeft = image.screenSpace ? image.topLeft : env.toScreen(image.topLeft);
      const bottomRight = image.screenSpace ? image.bottomRight : env.toScreen(image.bottomRight);
      if (!topLeft || !bottomRight) continue;

      const left = Math.min(topLeft.x, bottomRight.x);
      const top = Math.min(topLeft.y, bottomRight.y);
      const width = Math.abs(bottomRight.x - topLeft.x);
      const height = Math.abs(bottomRight.y - topLeft.y);
      if (width <= 1e-6 || height <= 1e-6) continue;

      if (
        screenX >= left
        && screenX <= left + width
        && screenY >= top
        && screenY <= top + height
      ) {
        return index;
      }
    }
    return null;
  };
})();
