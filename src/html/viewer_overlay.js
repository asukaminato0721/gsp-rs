// @ts-check

(function() {
  const modules = window.GspViewerModules || (window.GspViewerModules = {});
  /** @typedef {{ name: string; children: RichMarkupNode[] }} RichMarkupNode */
  /** @typedef {{ kind: "text"; text: string } | { kind: "fraction"; numerator: RichMarkupItem[]; denominator: RichMarkupItem[] } | { kind: "radical" | "overline" | "ray" | "arc"; children: RichMarkupItem[] }} RichMarkupItem */
  /** @typedef {{ buttonIndex: number; pointerId: number; startClientX: number; startClientY: number; originX: number; originY: number; scaleX: number; scaleY: number; dragged: boolean }} ButtonPointerState */
  /** @typedef {Extract<ButtonActionJson, { kind: "toggle-visibility" }> | Extract<ButtonActionJson, { kind: "set-visibility" }> | Extract<ButtonActionJson, { kind: "show-hide-visibility" }>} VisibilityButtonAction */

  /**
   * @param {ButtonActionJson} action
   * @returns {action is VisibilityButtonAction}
   */
  function isVisibilityButtonAction(action) {
    return action.kind === "toggle-visibility"
      || action.kind === "set-visibility"
      || action.kind === "show-hide-visibility";
  }

  /** @param {string} text */
  function cleanRichText(text) {
    return text
      .split("\u2013").join("-")
      .split("\u2014").join("-")
      .split("厘米").join("cm");
  }

  /** @param {string} token */
  function decodeRichMarkupText(token) {
    if (!token.startsWith("T")) {
      return null;
    }
    const xIndex = token.indexOf("x");
    if (xIndex < 0) {
      return null;
    }
    return cleanRichText(token.slice(xIndex + 1));
  }

  /**
   * @param {string} markup
   * @returns {RichMarkupNode[]}
   */
  function parseRichMarkupNodes(markup) {
    /**
     * @param {string} source
     * @param {number} start
     * @param {boolean} stopOnGt
     * @returns {[RichMarkupNode[], number]}
     */
    function parseSeq(source, start, stopOnGt) {
      /** @type {RichMarkupNode[]} */
      const nodes = [];
      let index = start;
      while (index < source.length) {
        if (stopOnGt && source[index] === ">") {
          return [nodes, index + 1];
        }
        if (source[index] !== "<") {
          index += 1;
          continue;
        }
        index += 1;
        const nameStart = index;
        while (index < source.length && source[index] !== "<" && source[index] !== ">") {
          index += 1;
        }
        const name = source.slice(nameStart, index);
        /** @type {RichMarkupNode[]} */
        let children = [];
        if (index < source.length && source[index] === "<") {
          [children, index] = parseSeq(source, index, true);
        } else if (index < source.length && source[index] === ">") {
          index += 1;
        }
        nodes.push({ name, children });
      }
      return [nodes, index];
    }

    return parseSeq(markup, 0, false)[0];
  }

  /**
   * @param {RichMarkupItem[][]} target
   * @param {RichMarkupItem[][]} lines
   */
  function appendRichMarkupLines(target, lines) {
    if (!lines.length) {
      return;
    }
    if (!target.length) {
      target.push(...lines);
      return;
    }
    const [first, ...rest] = lines;
    target[target.length - 1].push(...first);
    target.push(...rest);
  }

  /**
   * @param {RichMarkupNode[]} nodes
   * @returns {RichMarkupItem[]}
   */
  function renderRichMarkupInline(nodes) {
    return renderRichMarkupNodes(nodes)
      .flatMap((line, index) => (index === 0 ? line : [{ kind: "text", text: " " }, ...line]));
  }

  /**
   * @param {RichMarkupNode} node
   * @returns {RichMarkupItem[][]}
   */
  function renderRichMarkupNode(node) {
    const text = decodeRichMarkupText(node.name);
    if (text !== null) {
      return text ? [[{ kind: "text", text }]] : [[]];
    }
    if (!node.name || node.name.startsWith("!") || node.name.startsWith("?1x")) {
      return renderRichMarkupNodes(node.children);
    }
    if (node.name === "VL") {
      return node.children.flatMap((/** @type {RichMarkupNode} */ child) => renderRichMarkupNode(child)).filter((/** @type {RichMarkupItem[]} */ line) => line.length);
    }
    if (node.name === "H") {
      return [renderRichMarkupInline(node.children)];
    }
    if (node.name === "/") {
      const [numerator, ...denominator] = node.children;
      if (!numerator || !denominator.length) {
        return [renderRichMarkupInline(node.children)];
      }
      return [[{
        kind: "fraction",
        numerator: renderRichMarkupInline([numerator]),
        denominator: renderRichMarkupInline(denominator),
      }]];
    }
    if (node.name === "R") {
      return [[{
        kind: "radical",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    if (node.name === "SO2") {
      return [[{
        kind: "overline",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    if (node.name === "SO3") {
      return [[{
        kind: "ray",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    if (node.name === "SO4") {
      return [[{
        kind: "arc",
        children: renderRichMarkupInline(node.children),
      }]];
    }
    return renderRichMarkupNodes(node.children);
  }

  /**
   * @param {RichMarkupNode[]} nodes
   * @returns {RichMarkupItem[][]}
   */
  function renderRichMarkupNodes(nodes) {
    /** @type {RichMarkupItem[][]} */
    const lines = [[]];
    nodes.forEach((/** @type {RichMarkupNode} */ node) => {
      appendRichMarkupLines(lines, renderRichMarkupNode(node));
    });
    return lines.filter((line) => line.length);
  }

  /**
   * @param {HTMLElement} parent
   * @param {RichMarkupItem[]} items
   */
  function appendRichMarkupItems(parent, items) {
    items.forEach((/** @type {RichMarkupItem} */ item) => {
      parent.append(renderRichMarkupItem(item));
    });
  }

  /**
   * @param {RichMarkupItem} item
   * @returns {HTMLElement}
   */
  function renderRichMarkupItem(item) {
    if (item.kind === "text") {
      const span = document.createElement("span");
      span.textContent = item.text;
      return span;
    }
    if (item.kind === "fraction") {
      const fraction = document.createElement("span");
      fraction.className = "scene-rich-fraction";
      const numerator = document.createElement("span");
      numerator.className = "scene-rich-fraction-part";
      appendRichMarkupItems(numerator, item.numerator);
      const bar = document.createElement("span");
      bar.className = "scene-rich-fraction-bar";
      const denominator = document.createElement("span");
      denominator.className = "scene-rich-fraction-part";
      appendRichMarkupItems(denominator, item.denominator);
      fraction.append(numerator, bar, denominator);
      return fraction;
    }
    const span = document.createElement("span");
    if (item.kind === "radical") {
      span.className = "scene-rich-radical";
      const symbol = document.createElement("span");
      symbol.className = "scene-rich-radical-symbol";
      symbol.textContent = "\u221a";
      const radicand = document.createElement("span");
      radicand.className = "scene-rich-radicand";
      appendRichMarkupItems(radicand, item.children);
      span.append(symbol, radicand);
      return span;
    }
    span.className = `scene-rich-${item.kind}`;
    appendRichMarkupItems(span, item.children);
    return span;
  }

  /** @param {{ richMarkup?: string | null }} label */
  function renderRichLabel(label) {
    if (!label.richMarkup) {
      return null;
    }
    const lines = renderRichMarkupNodes(parseRichMarkupNodes(label.richMarkup));
    if (!lines.length) {
      return null;
    }
    const root = document.createElement("div");
    root.className = "scene-rich-label";
    lines.forEach((/** @type {RichMarkupItem[]} */ items) => {
      const line = document.createElement("div");
      line.className = "scene-rich-line";
      appendRichMarkupItems(line, items);
      root.append(line);
    });
    return root;
  }

  modules.overlay = {
    /**
     * @param {ViewerEnv} env
     * @param {HTMLElement | null} buttonOverlays
     */
    init(env, buttonOverlays) {
      const sourceScene = env.sourceScene;
      const buttonsState = env.van?.state
        ? env.van.state((sourceScene.buttons || []).map((button) => ({
            ...button,
            baseText: button.text,
            visible: true,
            active: false,
          })))
        : { val: (sourceScene.buttons || []).map((button) => ({
            ...button,
            baseText: button.text,
            visible: true,
            active: false,
          })) };
      const buttonTimers = new Map();
      const buttonAnimations = new Map();
      /** @type {{ val: HotspotFlash[] }} */
      const hotspotFlashesState = env.van?.state ? env.van.state([]) : { val: [] };
      /** @type {ButtonPointerState | null} */
      let buttonPointerState = null;

      /** @param {(buttons: RuntimeButtonJson[]) => void} mutator */
      function updateButtons(mutator) {
        const next = buttonsState.val.slice();
        mutator(next);
        buttonsState.val = next;
      }

      function buttonPointerScale() {
        const rect = env.canvas.getBoundingClientRect();
        return {
          scaleX: rect.width > 0 ? sourceScene.width / rect.width : 1,
          scaleY: rect.height > 0 ? sourceScene.height / rect.height : 1,
        };
      }

      /**
       * @param {VisibilityButtonAction} action
       * @param {boolean} visible
       */
      function setTargetsVisibility(action, visible) {
        env.updateScene((scene) => {
          (action.pointIndices || []).forEach((/** @type {number} */ index) => {
            if (scene.points[index]) scene.points[index].visible = visible;
          });
          (action.lineIndices || []).forEach((/** @type {number} */ index) => {
            if (scene.lines[index]) scene.lines[index].visible = visible;
          });
          (action.circleIndices || []).forEach((/** @type {number} */ index) => {
            if (scene.circles[index]) scene.circles[index].visible = visible;
          });
          (action.polygonIndices || []).forEach((/** @type {number} */ index) => {
            if (scene.polygons[index]) scene.polygons[index].visible = visible;
          });
        });
      }

      /**
       * @param {VisibilityButtonAction} action
       * @param {boolean} visible
       */
      function visibilityTargetsMatch(action, visible) {
        const scene = env.currentScene();
        const pointsMatch = (action.pointIndices || []).every((/** @type {number} */ index) => scene.points[index]?.visible === visible);
        const linesMatch = (action.lineIndices || []).every((/** @type {number} */ index) => scene.lines[index]?.visible === visible);
        const circlesMatch = (action.circleIndices || []).every((/** @type {number} */ index) => scene.circles[index]?.visible === visible);
        const polygonsMatch = (action.polygonIndices || []).every((/** @type {number} */ index) => scene.polygons[index]?.visible === visible);
        return pointsMatch && linesMatch && circlesMatch && polygonsMatch;
      }

      /**
       * @param {string} baseText
       * @param {boolean} targetsVisible
       */
      function toggledVisibilityText(baseText, targetsVisible) {
        if (typeof baseText !== "string" || !baseText) {
          return baseText;
        }
        if (targetsVisible) {
          if (baseText.includes("显示")) {
            return baseText.replace("显示", "隐藏");
          }
        } else if (baseText.includes("隐藏")) {
          return baseText.replace("隐藏", "显示");
        }
        return baseText;
      }

      /**
       * @param {number} buttonIndex
       * @param {string} nextText
       */
      function updateLinkedButtonLabels(buttonIndex, nextText) {
        env.updateScene((scene) => {
          scene.labels.forEach((label) => {
            if (!label.hotspots?.length) {
              return;
            }
            const lines = label.text.split("\n").map((/** @type {string} */ line) => Array.from(line));
            let changed = false;
            const relevantHotspots = label.hotspots
              .filter((/** @type {RuntimeLabelHotspotJson} */ hotspot) =>
                hotspot.action?.kind === "button" && hotspot.action.buttonIndex === buttonIndex
              )
              .sort((/** @type {RuntimeLabelHotspotJson} */ left, /** @type {RuntimeLabelHotspotJson} */ right) => right.line - left.line || right.start - left.start);
            relevantHotspots.forEach((/** @type {RuntimeLabelHotspotJson} */ hotspot) => {
              const line = lines[hotspot.line];
              if (!line) {
                return;
              }
              line.splice(hotspot.start, hotspot.end - hotspot.start, ...Array.from(nextText));
              hotspot.end = hotspot.start + Array.from(nextText).length;
              hotspot.text = nextText;
              changed = true;
            });
            if (changed) {
              label.text = lines.map((/** @type {string[]} */ line) => line.join("")).join("\n");
            }
          });
        });
      }

      /**
       * @param {number} buttonIndex
       * @param {VisibilityButtonAction} action
       */
      function syncVisibilityButtonState(buttonIndex, action) {
        let active = false;
        if (action.kind === "toggle-visibility") {
          active = visibilityTargetsMatch(action, true);
        } else if (action.kind === "set-visibility") {
          active = visibilityTargetsMatch(action, !!action.visible);
        } else if (action.kind === "show-hide-visibility") {
          active = visibilityTargetsMatch(action, true);
        } else {
          return;
        }
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = active;
            if (action.kind === "show-hide-visibility" || action.kind === "toggle-visibility") {
              buttons[buttonIndex].text = toggledVisibilityText(
                buttons[buttonIndex].baseText || buttons[buttonIndex].text,
                active,
              );
            }
          }
        });
        if (action.kind === "show-hide-visibility" || action.kind === "toggle-visibility") {
          const button = buttonsState.val[buttonIndex];
          if (button) {
            updateLinkedButtonLabels(buttonIndex, button.text);
          }
        }
      }

      /** @param {VisibilityButtonAction} action */
      function toggleTargetsVisibility(action) {
        const scene = env.currentScene();
        const hiddenPoint = (action.pointIndices || []).some((/** @type {number} */ index) => scene.points[index]?.visible === false);
        const hiddenLine = (action.lineIndices || []).some((/** @type {number} */ index) => scene.lines[index]?.visible === false);
        const hiddenCircle = (action.circleIndices || []).some((/** @type {number} */ index) => scene.circles[index]?.visible === false);
        const hiddenPolygon = (action.polygonIndices || []).some((/** @type {number} */ index) => scene.polygons[index]?.visible === false);
        setTargetsVisibility(action, hiddenPoint || hiddenLine || hiddenCircle || hiddenPolygon);
      }

      /** @param {(flashes: HotspotFlash[]) => void} mutator */
      function updateHotspotFlashes(mutator) {
        const next = hotspotFlashesState.val.slice();
        mutator(next);
        hotspotFlashesState.val = next;
      }

      /** @param {LabelHotspotActionJson} action */
      function hotspotFlashKey(action) {
        switch (action.kind) {
          case "button":
            return `button:${action.buttonIndex}`;
          case "point":
            return `point:${action.pointIndex}`;
          case "segment":
            return `segment:${action.startPointIndex}:${action.endPointIndex}`;
          case "angle-marker":
            return `angle:${action.startPointIndex}:${action.vertexPointIndex}:${action.endPointIndex}`;
          case "circle":
            return `circle:${action.circleIndex}`;
          case "polygon":
            return `polygon:${action.polygonIndex}`;
          default:
            return JSON.stringify(action);
        }
      }

      /** @param {LabelHotspotActionJson} action */
      function flashHotspotAction(action) {
        const key = hotspotFlashKey(action);
        updateHotspotFlashes((flashes) => {
          const existingIndex = flashes.findIndex((flash) => flash.key === key);
          if (existingIndex >= 0) {
            flashes.splice(existingIndex, 1);
          }
          flashes.push({ key, action });
        });
        window.setTimeout(() => {
          updateHotspotFlashes((flashes) => {
            const index = flashes.findIndex((flash) => flash.key === key);
            if (index >= 0) {
              flashes.splice(index, 1);
            }
          });
        }, 180);
      }

      /** @param {number} buttonIndex */
      function stopButtonAnimation(buttonIndex) {
        const handle = buttonAnimations.get(buttonIndex);
        if (handle?.rafId) {
          window.cancelAnimationFrame(handle.rafId);
        }
        if (handle) {
          handle.stop = true;
        }
        buttonAnimations.delete(buttonIndex);
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = false;
          }
        });
      }

      /**
       * @param {number} buttonIndex
       * @param {number} pointIndex
       * @param {"move" | "animate" | "scroll"} mode
       * @param {number | null} [targetPointIndex]
       */
      function toggleAnimatedPoint(buttonIndex, pointIndex, mode, targetPointIndex = null) {
        if (buttonsState.val[buttonIndex]?.active) {
          stopButtonAnimation(buttonIndex);
          return;
        }
        const scene = env.currentScene();
        const point = scene.points[pointIndex];
        if (!point) {
          return;
        }
        const base = { x: point.x, y: point.y };
        let initialDirection = 1;
        if (point.constraint?.kind === "segment") {
          if (targetPointIndex === point.constraint.startIndex) {
            initialDirection = -1;
          } else if (targetPointIndex === point.constraint.endIndex) {
            initialDirection = 1;
          } else {
            initialDirection = point.constraint.t < 0.5 ? 1 : -1;
          }
        }
        const state = {
          stop: false,
          direction: initialDirection,
          t: 0,
          vx: (Math.random() - 0.5) * 0.003,
          vy: (Math.random() - 0.5) * 0.003,
          nextTurnAt: 500 + Math.random() * 700,
          elapsedMs: 0,
          rafId: 0,
        };
        buttonAnimations.set(buttonIndex, state);
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = true;
          }
        });
        /** @type {number | null} */
        let lastTime = null;
        /** @param {number} timestamp */
        const step = (timestamp) => {
          if (state.stop) {
            return;
          }
          if (lastTime === null) {
            lastTime = timestamp;
          }
          const dt = Math.min(64, timestamp - lastTime);
          lastTime = timestamp;
          env.updateScene((draft) => {
            const draftPoint = draft.points[pointIndex];
            if (!draftPoint) {
              return;
            }
            const parameterized = modules.dynamics.parameterValueFromPoint
              ? modules.dynamics.parameterValueFromPoint(draft, pointIndex)
              : null;
            if (parameterized !== null && draftPoint.constraint) {
              const durationMs = mode === "scroll" ? 16000 : 12000;
              const delta = dt / durationMs;
              if (mode === "scroll") {
                modules.dynamics.applyNormalizedParameterToPoint(
                  draftPoint,
                  draft,
                  parameterized + delta,
                );
              } else {
                let next = parameterized + delta * state.direction;
                if (next >= 1) {
                  next = 1;
                  state.direction = -1;
                } else if (next <= 0) {
                  next = 0;
                  state.direction = 1;
                }
                modules.dynamics.applyNormalizedParameterToPoint(draftPoint, draft, next);
              }
            } else if (mode === "scroll") {
              state.t += dt * 0.004;
              draftPoint.x = base.x + Math.sin(state.t) * 36;
            } else {
              state.elapsedMs += dt;
              if (state.elapsedMs >= state.nextTurnAt) {
                state.elapsedMs = 0;
                state.nextTurnAt = 500 + Math.random() * 700;
                state.vx += (Math.random() - 0.5) * 0.0016;
                state.vy += (Math.random() - 0.5) * 0.0016;
              }
              state.vx += (base.x - draftPoint.x) * 0.00008;
              state.vy += (base.y - draftPoint.y) * 0.00008;
              const speed = Math.hypot(state.vx, state.vy);
              if (speed > 0.005) {
                state.vx = (state.vx / speed) * 0.005;
                state.vy = (state.vy / speed) * 0.005;
              } else if (speed < 0.0008) {
                const angle = Math.random() * Math.PI * 2;
                state.vx = Math.cos(angle) * 0.0015;
                state.vy = Math.sin(angle) * 0.0015;
              }

              draftPoint.x += state.vx * dt;
              draftPoint.y += state.vy * dt;

              const maxDx = 0.8;
              const maxDy = 0.6;
              if (draftPoint.x < base.x - maxDx || draftPoint.x > base.x + maxDx) {
                state.vx *= -0.7;
                draftPoint.x = Math.max(base.x - maxDx, Math.min(base.x + maxDx, draftPoint.x));
              }
              if (draftPoint.y < base.y - maxDy || draftPoint.y > base.y + maxDy) {
                state.vy *= -0.7;
                draftPoint.y = Math.max(base.y - maxDy, Math.min(base.y + maxDy, draftPoint.y));
              }
            }
          });
          state.rafId = window.requestAnimationFrame(step);
        };
        state.rafId = window.requestAnimationFrame(step);
      }

      /** @param {number} buttonIndex */
      function runButtonAction(buttonIndex) {
        const button = buttonsState.val[buttonIndex];
        if (!button) {
          return;
        }
        /** @type {ButtonActionJson} */
        const action = button.action;
        switch (action.kind) {
          case "link":
            if (action.href) {
              window.open(action.href, "_blank", "noopener,noreferrer");
            }
            break;
          case "toggle-visibility":
            toggleTargetsVisibility(action);
            syncVisibilityButtonState(buttonIndex, action);
            break;
          case "set-visibility":
            setTargetsVisibility(action, !!action.visible);
            syncVisibilityButtonState(buttonIndex, action);
            break;
          case "show-hide-visibility": {
            const nextVisible = !visibilityTargetsMatch(action, true);
            setTargetsVisibility(action, nextVisible);
            syncVisibilityButtonState(buttonIndex, action);
            break;
          }
          case "move-point":
            if (typeof action.pointIndex === "number") {
              toggleAnimatedPoint(
                buttonIndex,
                action.pointIndex,
                "move",
                action.targetPointIndex ?? null,
              );
            }
            break;
          case "animate-point":
            if (typeof action.pointIndex === "number") {
              toggleAnimatedPoint(buttonIndex, action.pointIndex, "animate");
            }
            break;
          case "scroll-point":
            if (typeof action.pointIndex === "number") {
              toggleAnimatedPoint(buttonIndex, action.pointIndex, "scroll");
            }
            break;
          case "sequence": {
            const intervalMs = Math.max(0, action.intervalMs || 0);
            (action.buttonIndices || []).forEach((/** @type {number} */ childButtonIndex, /** @type {number} */ offset) => {
              const timer = window.setTimeout(() => {
                runButtonAction(childButtonIndex);
                buttonTimers.delete(timer);
              }, offset * intervalMs);
              buttonTimers.set(timer, true);
            });
            break;
          }
          default:
            if (isVisibilityButtonAction(action)) {
              syncVisibilityButtonState(buttonIndex, action);
            }
            break;
        }
      }

      /** @param {LabelHotspotActionJson | null} action */
      function runHotspotAction(action) {
        if (!action) {
          return;
        }
        if (action.kind === "button" && typeof action.buttonIndex === "number") {
          runButtonAction(action.buttonIndex);
          return;
        }
        flashHotspotAction(action);
      }

      /**
       * @param {number} buttonIndex
       * @param {PointerEvent} event
       */
      function beginButtonPointer(buttonIndex, event) {
        const button = buttonsState.val[buttonIndex];
        if (!button) {
          return;
        }
        const { scaleX, scaleY } = buttonPointerScale();
        buttonPointerState = {
          buttonIndex,
          pointerId: event.pointerId,
          startClientX: event.clientX,
          startClientY: event.clientY,
          originX: button.x,
          originY: button.y,
          scaleX,
          scaleY,
          dragged: false,
        };
        window.addEventListener("pointermove", handleButtonPointerMove);
        window.addEventListener("pointerup", handleButtonPointerUp);
        window.addEventListener("pointercancel", handleButtonPointerUp);
        event.preventDefault();
      }

      /** @param {PointerEvent} event */
      function handleButtonPointerMove(event) {
        if (!buttonPointerState || event.pointerId !== buttonPointerState.pointerId) {
          return;
        }
        const dx = (event.clientX - buttonPointerState.startClientX) * buttonPointerState.scaleX;
        const dy = (event.clientY - buttonPointerState.startClientY) * buttonPointerState.scaleY;
        if (!buttonPointerState.dragged && Math.hypot(dx, dy) >= 4) {
          buttonPointerState.dragged = true;
        }
        if (!buttonPointerState.dragged) {
          return;
        }
        updateButtons((buttons) => {
          const button = buttons[buttonPointerState.buttonIndex];
          if (!button) {
            return;
          }
          button.x = buttonPointerState.originX + dx;
          button.y = buttonPointerState.originY + dy;
        });
      }

      function clearButtonPointer() {
        window.removeEventListener("pointermove", handleButtonPointerMove);
        window.removeEventListener("pointerup", handleButtonPointerUp);
        window.removeEventListener("pointercancel", handleButtonPointerUp);
        buttonPointerState = null;
      }

      /** @param {PointerEvent} event */
      function handleButtonPointerUp(event) {
        if (!buttonPointerState || event.pointerId !== buttonPointerState.pointerId) {
          return;
        }
        const { buttonIndex, dragged } = buttonPointerState;
        clearButtonPointer();
        if (!dragged) {
          runButtonAction(buttonIndex);
        }
      }

      function render() {
        if (!buttonOverlays) {
          return;
        }
        buttonOverlays.replaceChildren();
        const stackedOffsets = new Map();
        buttonsState.val.forEach((/** @type {RuntimeButtonJson} */ buttonDef, /** @type {number} */ buttonIndex) => {
          if (buttonDef.visible === false) {
            return;
          }
          const anchor = document.createElement("button");
          anchor.className = "scene-link-button";
          anchor.setAttribute("aria-pressed", buttonDef.active ? "true" : "false");
          if (buttonDef.active) {
            anchor.classList.add("is-active");
          }
          anchor.type = "button";
          anchor.textContent = buttonDef.text;
          const key = `${Math.round(buttonDef.x)}:${Math.round(buttonDef.y)}`;
          const stackedOffset = stackedOffsets.get(key) || 0;
          stackedOffsets.set(key, stackedOffset + 1);
          anchor.style.left = `${(buttonDef.x / sourceScene.width) * 100}%`;
          anchor.style.top = `${((buttonDef.y + stackedOffset * 34) / sourceScene.height) * 100}%`;
          if (buttonDef.width) {
            anchor.style.width = `${(buttonDef.width / sourceScene.width) * 100}%`;
          }
          if (buttonDef.height) {
            anchor.style.height = `${(buttonDef.height / sourceScene.height) * 100}%`;
          }
          anchor.addEventListener("pointerdown", (event) => {
            beginButtonPointer(buttonIndex, event);
          });
          buttonOverlays.append(anchor);
        });

        env.currentScene().labels.forEach((/** @type {RuntimeLabelJson} */ label) => {
          if (label.visible === false) {
            return;
          }
          if (label.richMarkup && !label.hotspots?.length) {
            const anchor = label.screenSpace
              ? label.anchor
              : env.resolvePoint(label.anchor);
            if (!anchor) {
              return;
            }
            const screen = label.screenSpace ? anchor : env.toScreen(anchor);
            const richLabel = renderRichLabel(label);
            if (!screen || !richLabel) {
              return;
            }
            richLabel.style.color = env.rgba(label.color);
            richLabel.style.left = `${(((screen.x + (label.centeredOnAnchor ? 0 : 2)) / sourceScene.width) * 100)}%`;
            richLabel.style.top = `${(((screen.y + (label.centeredOnAnchor ? -10 : -14)) / sourceScene.height) * 100)}%`;
            if (label.centeredOnAnchor) {
              richLabel.style.transform = "translate(-50%, -50%)";
            }
            buttonOverlays.append(richLabel);
            return;
          }
          if (!label.hotspots?.length) {
            return;
          }
          modules.render.labelHotspotRects(env, label).forEach((rect) => {
            const hotspot = document.createElement("button");
            hotspot.className = "scene-hotspot";
            hotspot.type = "button";
            hotspot.setAttribute("aria-label", rect.text);
            hotspot.style.left = `${(rect.left / sourceScene.width) * 100}%`;
            hotspot.style.top = `${(rect.top / sourceScene.height) * 100}%`;
            hotspot.style.width = `${(rect.width / sourceScene.width) * 100}%`;
            hotspot.style.height = `${(rect.height / sourceScene.height) * 100}%`;
            hotspot.addEventListener("click", (event) => {
              event.preventDefault();
              runHotspotAction(rect.action);
            });
            buttonOverlays.append(hotspot);
          });
        });
      }

      return {
        currentButtons() {
          return buttonsState.val;
        },
        currentHotspotFlashes() {
          return hotspotFlashesState.val;
        },
        render,
      };
    },
  };
})();
