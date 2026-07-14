(function() {
  const modules = (
    window.GspViewerModules || (window.GspViewerModules = {})
  ) as Partial<ViewerModules>;
  type RichMarkupNode = { name: string; children: RichMarkupNode[] };
  type RichMarkupStyle = { color?: string, fontSize?: string };
  type RichMarkupItem = { kind: "text"; text: string; style?: RichMarkupStyle } | { kind: "fraction"; numerator: RichMarkupItem[]; denominator: RichMarkupItem[]; style?: RichMarkupStyle } | { kind: "radical" | "overline" | "ray" | "arc"; children: RichMarkupItem[]; style?: RichMarkupStyle };
  type VisibilityButtonAction = Extract<ButtonActionJson, { kind: "show-hide-visibility" }>;
  type OverlayButtonElement = HTMLButtonElement & { __gspButtonIndex?: number, __gspHotspotAction?: LabelHotspotActionJson | null };

  
  function isFiniteNumber(value: unknown) {
    return typeof value === "number" && Number.isFinite(value);
  }

  
  function isVisibilityButtonAction(action: ButtonActionJson) {
    return action.kind === "show-hide-visibility";
  }

  
  function cleanRichText(text: string) {
    return text
      .split("\u2013").join("-")
      .split("\u2014").join("-")
      .split("厘米").join("cm");
  }

  
  function decodeRichMarkupText(token: string) {
    if (!token.startsWith("T")) {
      return null;
    }
    const xIndex = token.indexOf("x");
    if (xIndex < 0) {
      return null;
    }
    return cleanRichText(token.slice(xIndex + 1));
  }

  
  function parseRichMarkupNodes(markup: string) {
    
    function parseSeq(source: string, start: number, stopOnGt: boolean): [RichMarkupNode[], number] {
      
      const nodes: RichMarkupNode[] = [];
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
        
        let children: RichMarkupNode[] = [];
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

  
  function appendRichMarkupLines(target: RichMarkupItem[][], lines: RichMarkupItem[][]) {
    if (!lines.length) {
      return;
    }
    if (!target.length) {
      target.push(...lines);
      return;
    }
    const first = lines[0];
    if (!first) {
      return;
    }
    const rest = lines.slice(1);
    const lastTargetLine = target[target.length - 1];
    if (!lastTargetLine) {
      target.push(...lines);
      return;
    }
    lastTargetLine.push(...first);
    target.push(...rest);
  }

  
  function renderRichMarkupInline(nodes: RichMarkupNode[]) {
    return renderRichMarkupNodes(nodes)
      .flatMap((line, index: number) => (index === 0 ? line : [{ kind: "text", text: " " }, ...line]));
  }

  function richMarkupStyle(token: string): RichMarkupStyle | null {
    const fontMatch = token.match(/#([0-9a-f]+)/i);
    const colorMatch = token.match(/R([0-9a-f]+)G([0-9a-f]+)L([0-9a-f]+)/i);
    const style: RichMarkupStyle = {};
    if (fontMatch?.[1]) {
      const size = Number.parseInt(fontMatch[1], 16);
      if (Number.isFinite(size) && size > 0) style.fontSize = `${size}px`;
    }
    if (colorMatch?.[1] && colorMatch[2] && colorMatch[3]) {
      const component = (value: string) => Math.max(0, Math.min(255, Number.parseInt(value, 16) - 1));
      style.color = `rgb(${component(colorMatch[1])},${component(colorMatch[2])},${component(colorMatch[3])})`;
    }
    return Object.keys(style).length ? style : null;
  }

  function applyRichMarkupStyle(items: RichMarkupItem[][], style: RichMarkupStyle | null) {
    if (!style) return items;
    items.flat().forEach((item) => { item.style = { ...item.style, ...style }; });
    return items;
  }

  
  function renderRichMarkupNode(node: RichMarkupNode) {
    const text = decodeRichMarkupText(node.name);
    if (text !== null) {
      return text ? [[{ kind: "text", text }]] : [[]];
    }
    if (!node.name || node.name.startsWith("!") || node.name.startsWith("?1x")) {
      return renderRichMarkupNodes(node.children);
    }
    if (node.name === "VL") {
      return node.children.flatMap(( child) => renderRichMarkupNode(child)).filter(( line) => line.length);
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
    return applyRichMarkupStyle(renderRichMarkupNodes(node.children), richMarkupStyle(node.name));
  }

  
  function renderRichMarkupNodes(nodes: RichMarkupNode[]) {
    
    const lines = [[]];
    nodes.forEach(( node) => {
      appendRichMarkupLines(lines, renderRichMarkupNode(node));
    });
    return lines.filter((line) => line.length);
  }

  
  function appendRichMarkupItems(parent: HTMLElement, items: RichMarkupItem[]) {
    items.forEach(( item) => {
      parent.append(renderRichMarkupItem(item));
    });
  }

  
  function renderRichMarkupItem(item: RichMarkupItem) {
    if (item.kind === "text") {
      const span = document.createElement("span");
      span.textContent = item.text;
      Object.assign(span.style, item.style || {});
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
      Object.assign(fraction.style, item.style || {});
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
    Object.assign(span.style, item.style || {});
    appendRichMarkupItems(span, item.children);
    return span;
  }

  
  function renderRichLabel(label: RuntimeLabelJson) {
    if (!label.richMarkup) {
      return null;
    }
    const lines = renderRichMarkupNodes(parseRichMarkupNodes(label.richMarkup));
    if (!lines.length) {
      return null;
    }
    const root = document.createElement("div");
    root.className = "scene-rich-label";
    lines.forEach(( items) => {
      const line = document.createElement("div");
      line.className = "scene-rich-line";
      appendRichMarkupItems(line, items);
      root.append(line);
    });
    return root;
  }

  modules.overlay = {
    
    init(env: ViewerEnv, buttonOverlays: HTMLElement | null) {
      const sourceScene = env.sourceScene;
      const buttonsState = env.van?.state
        ? env.van.state((sourceScene.buttons || []).map((button) => ({
            ...button,
            baseText: button.text,
            visible: button.visible !== false,
            active: false,
          })))
        : { val: (sourceScene.buttons || []).map((button) => ({
            ...button,
            baseText: button.text,
            visible: button.visible !== false,
            active: false,
          })) };
      const buttonTimers = new Map();
      const buttonAnimations = new Map();
      const buttonAudio = new Map();
      
      let sharedAudioContext = null;
      
      const overlayNodeCache = new Map();
      
      const hotspotFlashesState = env.van?.state ? env.van.state([]) : { val: [] };
      
      let buttonPointerState = null;


      function updateButtons(mutator: (buttons: RuntimeButtonJson[]) => void) {
        const next = buttonsState.val.slice();
        mutator(next);
        buttonsState.val = next;
      }

      
      function forEachVisibilityTarget(action: VisibilityButtonAction, scene: ViewerSceneData, buttons: RuntimeButtonJson[], callback: (target: VisibilityTarget | undefined) => void) {
        (action.buttonIndices || []).forEach(( index: number) => {
          callback(buttons[index]);
        });
        (action.labelIndices || []).forEach(( index: number) => {
          callback(scene.labels[index]);
        });
        (action.imageIndices || []).forEach(( index: number) => {
          callback(scene.images[index]);
        });
        (action.pointIndices || []).forEach(( index: number) => {
          callback(scene.points[index]);
        });
        (action.lineIndices || []).forEach(( index: number) => {
          callback(scene.lines[index]);
        });
        (action.circleIndices || []).forEach(( index: number) => {
          callback(scene.circles[index]);
        });
        (action.polygonIndices || []).forEach(( index: number) => {
          callback(scene.polygons[index]);
        });
        (action.lineIterationIndices || []).forEach(( index: number) => {
          callback(env.sourceScene.lineIterations[index]);
        });
        (action.polygonIterationIndices || []).forEach(( index: number) => {
          callback(env.sourceScene.polygonIterations[index]);
        });
      }

      function buttonPointerScale() {
        const rect = env.canvas.getBoundingClientRect();
        return {
          scaleX: rect.width > 0 ? sourceScene.width / rect.width : 1,
          scaleY: rect.height > 0 ? sourceScene.height / rect.height : 1,
        };
      }

      
      function setTargetsVisibility(action: VisibilityButtonAction, visible: boolean) {
        const nextButtons = buttonsState.val.slice();
        env.updateScene((scene: ViewerSceneData) => {
          forEachVisibilityTarget(action, scene, nextButtons, (target) => {
            if (target) target.visible = visible;
          });
        }, "none");
        buttonsState.val = nextButtons;
        if ((action.lineIterationIndices || []).length > 0
          || (action.polygonIterationIndices || []).length > 0) {
          env.syncDynamicScene();
        }
      }

      
      function visibilityTargetsMatch(action: VisibilityButtonAction, visible: boolean) {
        const scene = env.currentScene();
        let matched = true;
        forEachVisibilityTarget(action, scene, buttonsState.val, (target) => {
          matched = matched && target?.visible === visible;
        });
        return matched;
      }

      
      function toggledVisibilityText(baseText: string, targetsVisible: boolean) {
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

      
      function updateLinkedButtonLabels(buttonIndex: number, nextText: string) {
        env.updateScene((scene: ViewerSceneData) => {
          scene.labels.forEach((label) => {
            if (!label.hotspots?.length) {
              return;
            }
            const lines = label.text.split("\n").map(( line) => Array.from(line));
            let changed = false;
            const relevantHotspots = label.hotspots
              .filter(( hotspot) =>
                hotspot.action?.kind === "button" && hotspot.action.buttonIndex === buttonIndex
              )
              .sort(( left,  right) => right.line - left.line || right.start - left.start);
            relevantHotspots.forEach(( hotspot) => {
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
              label.text = lines.map(( line) => line.join("")).join("\n");
            }
          });
        }, "none");
      }

      
      function syncVisibilityButtonState(buttonIndex: number, action: VisibilityButtonAction) {
        let active = false;
        if (action.kind === "show-hide-visibility") {
          active = visibilityTargetsMatch(action, true);
        } else {
          return;
        }
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = active;
            if (action.kind === "show-hide-visibility") {
              buttons[buttonIndex].text = toggledVisibilityText(
                buttons[buttonIndex].baseText || buttons[buttonIndex].text,
                active,
              );
            }
          }
        });
        if (action.kind === "show-hide-visibility") {
          const button = buttonsState.val[buttonIndex];
          if (button) {
            updateLinkedButtonLabels(buttonIndex, button.text);
          }
        }
      }

      
      function focusPoint(pointIndex: number) {
        const point = env.currentScene().points[pointIndex];
        if (!point) {
          return;
        }
        
        env.updateViewState?.((view) => {
          view.centerX = point.x;
          view.centerY = point.y;
        });
      }

      
      function updateHotspotFlashes(mutator: (flashes: HotspotFlash[]) => void) {
        const next = hotspotFlashesState.val.slice();
        mutator(next);
        hotspotFlashesState.val = next;
      }

      
      function hotspotFlashKey(action: LabelHotspotActionJson) {
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

      
      function flashHotspotAction(action: LabelHotspotActionJson) {
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

      
      function stopButtonAnimation(buttonIndex: number) {
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

      
      function setParameterValue(parameterName: string, value: number) {
        if (typeof parameterName !== "string" || !Number.isFinite(value)) {
          return;
        }
        let updated = false;
        env.updateDynamics((draft: ViewerSceneData) => {
          const parameter = draft.parameters.find((candidate) => candidate.name === parameterName);
          if (!parameter) {
            return;
          }
          parameter.value = value;
          updated = true;
        });
        if (!updated) {
          return;
        }
        modules.dynamics?.syncDynamicScene?.(env, [parameterName]);
        modules.dynamics?.buildParameterControls?.(env);
      }

      
      function toggleAnimatedParameter(buttonIndex: number, parameterName: string, targetValue: number) {
        if (buttonsState.val[buttonIndex]?.active) {
          stopButtonAnimation(buttonIndex);
          return;
        }
        const parameter = env.currentDynamics().parameters.find((candidate) => candidate.name === parameterName);
        if (!parameter || !Number.isFinite(targetValue)) {
          return;
        }
        const state = {
          stop: false,
          startValue: parameter.value,
          targetValue,
          elapsedMs: 0,
          durationMs: Math.max(900, Math.min(9000, Math.abs(targetValue - parameter.value) * 18)),
          rafId: 0,
        };
        buttonAnimations.set(buttonIndex, state);
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = true;
          }
        });
        
        let lastTime = null;
        
        const step = (timestamp) => {
          if (state.stop) {
            return;
          }
          if (lastTime === null) {
            lastTime = timestamp;
          }
          const dt = Math.min(64, timestamp - lastTime);
          lastTime = timestamp;
          state.elapsedMs += dt;
          const t = Math.min(1, state.elapsedMs / state.durationMs);
          const eased = t * (2 - t);
          setParameterValue(parameterName, state.startValue + (state.targetValue - state.startValue) * eased);
          if (t >= 1) {
            stopButtonAnimation(buttonIndex);
            return;
          }
          state.rafId = window.requestAnimationFrame(step);
        };
        state.rafId = window.requestAnimationFrame(step);
      }

      async function ensureAudioContext() {
        const AudioContextCtor = window.AudioContext
          || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
        if (!AudioContextCtor) {
          return null;
        }
        if (!sharedAudioContext) {
          sharedAudioContext = new AudioContextCtor();
        }
        if (sharedAudioContext.state === "suspended") {
          await sharedAudioContext.resume();
        }
        return sharedAudioContext;
      }

      
      function playbackFrequencyHz(functionDef: RuntimeFunctionJson, parameters: Map<string, number>) {
        const named = parameters.get(functionDef.name);
        if (isFiniteNumber(named) && named >= 20 && named <= 2000) {
          return named;
        }
        return 440;
      }

      
      function stopButtonPlayback(buttonIndex: number) {
        const handle = buttonAudio.get(buttonIndex);
        if (!handle) {
          return;
        }
        buttonAudio.delete(buttonIndex);
        try {
          handle.source.onended = null;
          handle.source.stop();
        } catch {}
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = false;
          }
        });
      }

      
      function buildFunctionAudioSamples(functionDef: RuntimeFunctionJson) {
        const evaluateExpr = modules.dynamics?.evaluateExpr;
        const parameterMapForScene = modules.dynamics?.parameterMapForScene;
        if (typeof evaluateExpr !== "function" || typeof parameterMapForScene !== "function") {
          return null;
        }
        const parameters = parameterMapForScene(env, env.currentScene());
        const xMin = env.sourceScene?.piMode
          ? 0
          : (Number.isFinite(functionDef.domain?.xMin) ? functionDef.domain.xMin : 0);
        const xMax = env.sourceScene?.piMode
          ? Math.PI * 2
          : (Number.isFinite(functionDef.domain?.xMax) ? functionDef.domain.xMax : xMin + 1);
        const span = Math.max(1e-6, xMax - xMin);
        const sampleCount = 4096;
        const samples = new Float32Array(sampleCount);
        let sum = 0;
        let maxAbs = 0;
        for (let index = 0; index < sampleCount; index += 1) {
          const t = sampleCount <= 1 ? 0 : index / sampleCount;
          const x = xMin + span * t;
          const y = evaluateExpr(functionDef.expr, x, parameters);
          const sample = isFiniteNumber(y) ? y : 0;
          samples[index] = sample;
          sum += sample;
        }
        const mean = sum / sampleCount;
        for (let index = 0; index < sampleCount; index += 1) {
          const centered = (samples[index] ?? 0) - mean;
          samples[index] = centered;
          maxAbs = Math.max(maxAbs, Math.abs(centered));
        }
        if (!(maxAbs > 1e-6)) {
          for (let index = 0; index < sampleCount; index += 1) {
            const phase = (index / sampleCount) * Math.PI * 2;
            samples[index] = Math.sin(phase);
          }
          maxAbs = 1;
        }
        const scale = 0.2 / maxAbs;
        for (let index = 0; index < sampleCount; index += 1) {
          samples[index] = (samples[index] ?? 0) * scale;
        }
        return {
          samples,
          frequencyHz: playbackFrequencyHz(functionDef, parameters),
        };
      }

      
      async function toggleFunctionPlayback(buttonIndex: number, functionKey: number) {
        if (buttonsState.val[buttonIndex]?.active) {
          stopButtonPlayback(buttonIndex);
          return;
        }
        const functionDef = (env.currentDynamics().functions || []).find((candidate) =>
          candidate.key === functionKey && candidate.derivative !== true
        );
        if (!functionDef) {
          return;
        }
        const context = await ensureAudioContext();
        if (!context) {
          return;
        }
        const audio = buildFunctionAudioSamples(functionDef);
        if (!audio) {
          return;
        }
        const buffer = context.createBuffer(1, audio.samples.length, context.sampleRate);
        buffer.getChannelData(0).set(audio.samples);
        const source = context.createBufferSource();
        source.buffer = buffer;
        source.loop = true;
        source.loopStart = 0;
        source.loopEnd = buffer.duration;
        const naturalFrequency = context.sampleRate / audio.samples.length;
        source.playbackRate.value = Math.max(0.01, audio.frequencyHz / naturalFrequency);
        source.connect(context.destination);
        buttonAudio.set(buttonIndex, { source });
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = true;
          }
        });
        source.onended = () => {
          if (buttonAudio.get(buttonIndex)?.source !== source) {
            return;
          }
          buttonAudio.delete(buttonIndex);
          updateButtons((buttons) => {
            if (buttons[buttonIndex]) {
              buttons[buttonIndex].active = false;
            }
          });
        };
        source.start();
      }

      
      function toggleAnimatedPoint(
        buttonIndex: number,
        pointIndex: number,
        mode: "animate" | "scroll",
        animation: PointAnimationJson | null = null,
      ) {
        if (buttonsState.val[buttonIndex]?.active) {
          stopButtonAnimation(buttonIndex);
          return;
        }
        const scene = env.currentScene();
        const point = scene.points[pointIndex];
        if (!point) {
          return;
        }
        if (mode === "animate" && !animation) {
          return;
        }
        let initialDirection = 1;
        if (point.constraint?.kind === "segment") {
          initialDirection = point.constraint.t < 0.5 ? 1 : -1;
        }
        if (animation?.direction === 1) {
          initialDirection *= -1;
        }
        const state = {
          stop: false,
          direction: initialDirection,
          rafId: 0,
        };
        buttonAnimations.set(buttonIndex, state);
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = true;
          }
        });
        
        let lastTime = null;
        
        const step = (timestamp) => {
          if (state.stop) {
            return;
          }
          if (lastTime === null) {
            lastTime = timestamp;
          }
          const dt = Math.min(64, timestamp - lastTime);
          lastTime = timestamp;
          const sourcePointRootId = modules.dynamics?.sourcePointRootId;
          if (typeof sourcePointRootId === "function") {
            env.markDependencyRootsDirty?.(sourcePointRootId(pointIndex));
          }
          env.updateScene((draft: ViewerSceneData) => {
            const draftPoint = draft.points[pointIndex];
            if (!draftPoint) {
              return;
            }
            const parameterized = modules.dynamics.parameterValueFromPoint
              ? modules.dynamics.parameterValueFromPoint(draft, pointIndex)
              : null;
            if (parameterized !== null && draftPoint.constraint) {
              const speed = mode === "scroll" ? 0.75 : animation!.speed;
              const delta = dt * speed / 12000;
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
                  if (animation!.repeat) {
                    state.direction = -1;
                  } else {
                    state.stop = true;
                  }
                } else if (next <= 0) {
                  next = 0;
                  if (animation!.repeat) {
                    state.direction = 1;
                  } else {
                    state.stop = true;
                  }
                }
                modules.dynamics.applyNormalizedParameterToPoint(draftPoint, draft, next);
              }
            } else {
              state.stop = true;
            }
          }, "graph");
          if (state.stop) {
            stopButtonAnimation(buttonIndex);
            return;
          }
          state.rafId = window.requestAnimationFrame(step);
        };
        state.rafId = window.requestAnimationFrame(step);
      }

      
      function toggleAnimatedPoints(buttonIndex: number, targets: AnimatedPointTargetJson[]) {
        if (buttonsState.val[buttonIndex]?.active) {
          stopButtonAnimation(buttonIndex);
          return;
        }
        const scene = env.currentScene();
        const points = targets
          .map((target) => {
            const point = scene.points[target.pointIndex];
            if (!point || !target.animation) {
              return null;
            }
            let direction = 1;
            if (point.constraint?.kind === "segment") {
              direction = point.constraint.t < 0.5 ? 1 : -1;
            }
            if (target.animation.direction === 1) {
              direction *= -1;
            }
            return {
              pointIndex: target.pointIndex,
              direction,
              animation: target.animation,
              finished: false,
            };
          })
          .filter((point) => !!point);
        if (points.length === 0) {
          return;
        }
        const state = { stop: false, rafId: 0 };
        buttonAnimations.set(buttonIndex, state);
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) {
            buttons[buttonIndex].active = true;
          }
        });
        
        let lastTime = null;
        
        const step = (timestamp) => {
          if (state.stop) {
            return;
          }
          if (lastTime === null) {
            lastTime = timestamp;
          }
          const dt = Math.min(64, timestamp - lastTime);
          lastTime = timestamp;
          const sourcePointRootId = modules.dynamics?.sourcePointRootId;
          if (typeof sourcePointRootId === "function") {
            points.forEach((point) => {
              env.markDependencyRootsDirty?.(sourcePointRootId(point.pointIndex));
            });
          }
          env.updateScene((draft: ViewerSceneData) => {
            points.forEach((point) => {
              if (point.finished) return;
              const draftPoint = draft.points[point.pointIndex];
              if (!draftPoint?.constraint) {
                point.finished = true;
                return;
              }
              const parameterized = modules.dynamics.parameterValueFromPoint
                ? modules.dynamics.parameterValueFromPoint(draft, point.pointIndex)
                : null;
              if (parameterized === null) {
                point.finished = true;
                return;
              }
              const delta = dt * point.animation.speed / 12000;
              let next = parameterized + delta * point.direction;
              if (next >= 1) {
                next = 1;
                if (point.animation.repeat) point.direction = -1;
                else point.finished = true;
              } else if (next <= 0) {
                next = 0;
                if (point.animation.repeat) point.direction = 1;
                else point.finished = true;
              }
              modules.dynamics.applyNormalizedParameterToPoint(draftPoint, draft, next);
            });
            state.stop = points.every((point) => point.finished);
          }, "graph");
          if (state.stop) {
            stopButtonAnimation(buttonIndex);
            return;
          }
          state.rafId = window.requestAnimationFrame(step);
        };
        state.rafId = window.requestAnimationFrame(step);
      }

      
      function movePointsToPayloadTargets(
        buttonIndex: number,
        targets: Array<{ pointIndex: number, targetPointIndex: number | null }>,
        speed: number,
      ) {
        if (buttonsState.val[buttonIndex]?.active) {
          stopButtonAnimation(buttonIndex);
          return;
        }
        const sourcePointRootId = modules.dynamics?.sourcePointRootId;
        const markSourcesDirty = () => {
          if (typeof sourcePointRootId === "function") {
            env.markDependencyRootsDirty?.(
              targets.map((move) => sourcePointRootId(move.pointIndex)),
            );
          }
        };
        const starts = targets.map((move) => {
          const point = env.currentScene().points[move.pointIndex];
          return point ? { x: point.x, y: point.y } : null;
        });
        const applyProgress = (progress: number) => {
          markSourcesDirty();
          env.updateScene((draft: ViewerSceneData) => {
            targets.forEach((move, index) => {
              const start = starts[index];
            const targetPoint = typeof move.targetPointIndex === "number"
              ? draft.points[move.targetPointIndex]
              : null;
              if (!start || !targetPoint) return;
            modules.drag.updatePointToWorld(
              env,
              draft,
              move.pointIndex,
                {
                  x: start.x + (targetPoint.x - start.x) * progress,
                  y: start.y + (targetPoint.y - start.y) * progress,
                },
            );
          });
        }, "graph");
        };
        if (!Number.isFinite(speed) || speed <= 0) {
          applyProgress(1);
          return;
        }
        const state = { stop: false, progress: 0, rafId: 0 };
        buttonAnimations.set(buttonIndex, state);
        updateButtons((buttons) => {
          if (buttons[buttonIndex]) buttons[buttonIndex].active = true;
        });
        let lastTime: number | null = null;
        const step = (timestamp: number) => {
          if (state.stop) return;
          if (lastTime === null) lastTime = timestamp;
          const dt = Math.min(64, timestamp - lastTime);
          lastTime = timestamp;
          state.progress = Math.min(1, state.progress + dt * speed / 12000);
          applyProgress(state.progress);
          if (state.progress >= 1) {
            stopButtonAnimation(buttonIndex);
            return;
          }
          state.rafId = window.requestAnimationFrame(step);
        };
        state.rafId = window.requestAnimationFrame(step);
      }


      function runButtonAction(buttonIndex: number) {
        const button = buttonsState.val[buttonIndex];
        if (!button) {
          return;
        }
        
        const action = button.action;
        switch (action.kind) {
          case "link":
            if (action.href) {
              window.open(action.href, "_blank", "noopener,noreferrer");
            }
            break;
          case "show-hide-visibility": {
            const nextVisible = !visibilityTargetsMatch(action, true);
            setTargetsVisibility(action, nextVisible);
            syncVisibilityButtonState(buttonIndex, action);
            break;
          }
          case "move-point":
            if (typeof action.pointIndex === "number") {
              movePointsToPayloadTargets(buttonIndex, [action], action.speed);
            }
            break;
          case "move-points":
            if (Array.isArray(action.targets)) {
              movePointsToPayloadTargets(buttonIndex, action.targets, action.speed);
            }
            break;
          case "set-parameter":
            setParameterValue(action.parameterName, action.value);
            break;
          case "animate-parameter":
            toggleAnimatedParameter(buttonIndex, action.parameterName, action.targetValue);
            break;
          case "animate-point":
            if (typeof action.pointIndex === "number") {
              toggleAnimatedPoint(buttonIndex, action.pointIndex, "animate", action.animation);
            }
            break;
          case "animate-points":
            if (Array.isArray(action.targets)) {
              toggleAnimatedPoints(buttonIndex, action.targets);
            }
            break;
          case "scroll-point":
            if (typeof action.pointIndex === "number") {
              toggleAnimatedPoint(buttonIndex, action.pointIndex, "scroll");
            }
            break;
          case "focus-point":
            if (typeof action.pointIndex === "number") {
              focusPoint(action.pointIndex);
            }
            break;
          case "play-function":
            if (typeof action.functionKey === "number") {
              void toggleFunctionPlayback(buttonIndex, action.functionKey);
            }
            break;
          case "sequence": {
            const intervalMs = Math.max(0, action.intervalMs || 0);
            (action.buttonIndices || []).forEach(( childButtonIndex,  offset) => {
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

      
      function runHotspotAction(action: LabelHotspotActionJson | null) {
        if (!action) {
          return;
        }
        if (action.kind === "button" && typeof action.buttonIndex === "number") {
          runButtonAction(action.buttonIndex);
          return;
        }
        flashHotspotAction(action);
      }

      
      function beginButtonPointer(buttonIndex: number, event: PointerEvent) {
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

      
      function handleButtonPointerMove(event: PointerEvent) {
        const pointerState = buttonPointerState;
        if (!pointerState || event.pointerId !== pointerState.pointerId) {
          return;
        }
        const dx = (event.clientX - pointerState.startClientX) * pointerState.scaleX;
        const dy = (event.clientY - pointerState.startClientY) * pointerState.scaleY;
        if (!pointerState.dragged && Math.hypot(dx, dy) >= 4) {
          pointerState.dragged = true;
        }
        if (!pointerState.dragged) {
          return;
        }
        updateButtons((buttons) => {
          const button = buttons[pointerState.buttonIndex];
          if (!button) {
            return;
          }
          button.x = pointerState.originX + dx;
          button.y = pointerState.originY + dy;
        });
      }

      function clearButtonPointer() {
        window.removeEventListener("pointermove", handleButtonPointerMove);
        window.removeEventListener("pointerup", handleButtonPointerUp);
        window.removeEventListener("pointercancel", handleButtonPointerUp);
        buttonPointerState = null;
      }

      
      function getOverlayNode(key: string, factory: () => HTMLElement) {
        const existing = overlayNodeCache.get(key);
        if (existing) {
          return existing;
        }
        const created = factory();
        overlayNodeCache.set(key, created);
        return created;
      }

      
      function appendOverlayNodeAt(node: HTMLElement, index: number) {
        if (!buttonOverlays) {
          return;
        }
        const current = buttonOverlays.children[index] || null;
        if (current !== node) {
          buttonOverlays.insertBefore(node, current);
        }
      }

      
      function pruneOverlayNodes(activeKeys: Set<string>) {
        for (const [key, node] of overlayNodeCache.entries()) {
          if (activeKeys.has(key)) {
            continue;
          }
          node.remove();
          overlayNodeCache.delete(key);
        }
      }

      
      function getButtonNode(buttonIndex: number) {
        return getOverlayNode(`button:${buttonIndex}`, () => {
          const anchor = document.createElement("button") as OverlayButtonElement;
          anchor.className = "scene-link-button";
          anchor.type = "button";
          anchor.addEventListener("pointerdown", (event) => {
            const currentButtonIndex = anchor.__gspButtonIndex;
            if (typeof currentButtonIndex !== "number") {
              return;
            }
            beginButtonPointer(currentButtonIndex, event);
          });
          return anchor;
        }) as OverlayButtonElement;
      }

      
      function getHotspotNode(labelIndex: number, hotspotIndex: number) {
        return getOverlayNode(`hotspot:${labelIndex}:${hotspotIndex}`, () => {
          const hotspot = document.createElement("button") as OverlayButtonElement;
          hotspot.className = "scene-hotspot";
          hotspot.type = "button";
          hotspot.addEventListener("click", (event) => {
            event.preventDefault();
            runHotspotAction(hotspot.__gspHotspotAction ?? null);
          });
          return hotspot;
        }) as OverlayButtonElement;
      }

      
      function getRichLabelNode(labelIndex: number) {
        return  (getOverlayNode(`rich-label:${labelIndex}`, () => {
          const richLabel = document.createElement("div");
          richLabel.className = "scene-rich-label";
          return richLabel;
        }));
      }

      
      function handleButtonPointerUp(event: PointerEvent) {
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
        
        const activeKeys = new Set<string>();
        const stackedOffsets = new Map();
        let overlayIndex = 0;
        buttonsState.val.forEach(( buttonDef,  buttonIndex) => {
          if (buttonDef.visible === false) {
            return;
          }
          const nodeKey = `button:${buttonIndex}`;
          activeKeys.add(nodeKey);
          const anchor = getButtonNode(buttonIndex);
          anchor.__gspButtonIndex = buttonIndex;
          anchor.setAttribute("aria-pressed", buttonDef.active ? "true" : "false");
          anchor.classList.toggle("is-active", !!buttonDef.active);
          anchor.textContent = buttonDef.text;
          const key = `${Math.round(buttonDef.x)}:${Math.round(buttonDef.y)}`;
          const stackedOffset = stackedOffsets.get(key) || 0;
          stackedOffsets.set(key, stackedOffset + 1);
          anchor.style.left = `${(buttonDef.x / sourceScene.width) * 100}%`;
          anchor.style.top = `${((buttonDef.y + stackedOffset * 34) / sourceScene.height) * 100}%`;
          anchor.style.width = buttonDef.width
            ? `${(buttonDef.width / sourceScene.width) * 100}%`
            : "";
          anchor.style.height = buttonDef.height
            ? `${(buttonDef.height / sourceScene.height) * 100}%`
            : "";
          env.registerDebugElement?.(anchor, { category: "buttons", index: buttonIndex });
          appendOverlayNodeAt(anchor, overlayIndex);
          overlayIndex += 1;
        });

        env.currentScene().labels.forEach(( label,  labelIndex: number) => {
          if (label.visible === false) {
            return;
          }
          if (label.richMarkup && !label.hotspots?.length) {
            const anchor = label.screenSpace
              ? label.anchor as Point
              : env.resolvePoint(label.anchor);
            if (!anchor) {
              return;
            }
            const screen = label.screenSpace ? anchor : env.toScreen(anchor);
            const renderedRichLabel = renderRichLabel(label);
            if (!screen || !renderedRichLabel) {
              return;
            }
            const nodeKey = `rich-label:${labelIndex}`;
            activeKeys.add(nodeKey);
            const richLabel = getRichLabelNode(labelIndex);
            richLabel.className = renderedRichLabel.className;
            richLabel.replaceChildren(...Array.from(renderedRichLabel.childNodes));
            richLabel.style.color = env.rgba(label.color);
            richLabel.style.fontSize = label.fontSize ? `${label.fontSize}px` : "";
            richLabel.style.fontFamily = label.fontFamily
              ? `"${label.fontFamily}", "Noto Sans", "Segoe UI", sans-serif`
              : "";
            richLabel.style.left = `${(((screen.x + (label.centeredOnAnchor ? 0 : 2)) / sourceScene.width) * 100)}%`;
            richLabel.style.top = `${(((screen.y + (label.centeredOnAnchor ? -10 : -14)) / sourceScene.height) * 100)}%`;
            richLabel.style.transform = label.centeredOnAnchor ? "translate(-50%, -50%)" : "";
            env.registerDebugElement?.(richLabel, { category: "labels", index: labelIndex });
            appendOverlayNodeAt(richLabel, overlayIndex);
            overlayIndex += 1;
            return;
          }
          if (!label.hotspots?.length) {
            return;
          }
          modules.render.labelHotspotRects(env, label).forEach((rect) => {
            const nodeKey = `hotspot:${labelIndex}:${rect.hotspotIndex ?? -1}`;
            activeKeys.add(nodeKey);
            const hotspot = getHotspotNode(labelIndex, rect.hotspotIndex ?? -1);
            hotspot.__gspHotspotAction = rect.action ?? null;
            hotspot.setAttribute("aria-label", rect.text);
            hotspot.style.left = `${(rect.left / sourceScene.width) * 100}%`;
            hotspot.style.top = `${(rect.top / sourceScene.height) * 100}%`;
            hotspot.style.width = `${(rect.width / sourceScene.width) * 100}%`;
            hotspot.style.height = `${(rect.height / sourceScene.height) * 100}%`;
            env.registerDebugElement?.(hotspot, {
              category: "labelHotspots",
              index: labelIndex,
              hotspotIndex: rect.hotspotIndex ?? null,
            });
            appendOverlayNodeAt(hotspot, overlayIndex);
            overlayIndex += 1;
          });
        });
        pruneOverlayNodes(activeKeys);
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
