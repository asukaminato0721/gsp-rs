(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});

  function isDocumentSceneData(data: unknown): data is DocumentSceneData {
    return !!data
      && typeof data === "object"
      && (data as { kind?: string }).kind === "gsp-document"
      && Array.isArray((data as { pages?: unknown }).pages);
  }

  function activeDocumentPageIndex(pages: DocumentScenePage[]) {
    const match = /^#page-(\d+)$/.exec(window.location.hash);
    const index = match ? Number(match[1]) - 1 : 0;
    return Math.min(Math.max(Number.isFinite(index) ? index : 0, 0), pages.length - 1);
  }

  function readSceneData(element: HTMLElement | null) {
    if (!element?.textContent) {
      throw new Error("missing scene-data payload");
    }
    const raw: unknown = JSON.parse(element.textContent);
    const pages = isDocumentSceneData(raw) ? raw.pages : null;
    const activePageIndex = pages ? activeDocumentPageIndex(pages) : 0;
    const sourceScene = pages ? pages[activePageIndex].scene : raw as SceneData;
    return { raw, pages, activePageIndex, sourceScene };
  }

  function installPageNavigation(
    pages: DocumentScenePage[] | null,
    activePageIndex: number,
    buttons: HTMLButtonElement[],
  ) {
    const activate = (index: number) => {
      if (!pages || index === activePageIndex || index < 0 || index >= pages.length) return;
      window.location.hash = `page-${index + 1}`;
      window.location.reload();
    };
    buttons.forEach((button) => {
      const index = Number(button.dataset.pageIndex);
      const selected = index === activePageIndex;
      button.setAttribute("aria-selected", selected ? "true" : "false");
      button.classList.toggle("is-active", selected);
      button.addEventListener("click", () => activate(index));
    });
    window.addEventListener("hashchange", () => {
      if (pages) window.location.reload();
    });
  }

  modules.appDocument = { readSceneData, installPageNavigation };
})();
