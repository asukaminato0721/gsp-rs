(function() {
  const modules =  (
    window.GspViewerModules || (window.GspViewerModules = {})
  );
  
  const imageCache = new Map();

  
  function loadImage(src: string, env: ViewerEnv | null | undefined) {
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

  
  modules.render.drawImages = function drawImages(env: ViewerEnv) {
    (env.currentScene().images || []).forEach((image, index: number) => {
      if (image.visible === false) return;
      const entry = loadImage(image.src, env);
      if (!entry.loaded) return;

      const topLeft = image.screenSpace ? image.topLeft : env.toScreen(image.topLeft);
      const bottomRight = image.screenSpace ? image.bottomRight : env.toScreen(image.bottomRight);
      if (!topLeft || !bottomRight) return;

      const left = Math.min(topLeft.x, bottomRight.x);
      const top = Math.min(topLeft.y, bottomRight.y);
      const width = Math.abs(bottomRight.x - topLeft.x);
      const height = Math.abs(bottomRight.y - topLeft.y);
      if (width <= 1e-6 || height <= 1e-6) return;

      modules.render.appendSceneElement(env, "image", {
        x: left,
        y: top,
        width,
        height,
        href: image.src,
        preserveAspectRatio: "none",
      }, null, { category: "images", index });
    });
  };

  
  modules.render.findHitImage = function findHitImage(env: ViewerEnv, screenX: number, screenY: number) {
    const images = env.currentScene().images || [];
    for (let index = images.length - 1; index >= 0; index -= 1) {
      const image = images[index];
      if (!image) continue;
      if (image.visible === false) continue;
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
