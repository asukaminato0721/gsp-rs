// @ts-check

(function() {
  const modules = /** @type {Partial<ViewerModules> & { geometry: ViewerGeometryModule; scene: ViewerSceneModule }} */ (
    window.GspViewerModules || (window.GspViewerModules = {})
  );
  const geometry = modules.geometry;
  const {
    lerpPoint,
    rotateAround,
    scaleAround,
    reflectAcrossLine,
    clipParametricLineToBounds,
    clipLineToBounds,
    clipRayToBounds,
    angleBisectorDirection,
    measuredRotationRadians,
    scaleByThreePointRatio,
  } = geometry;
  /** @typedef {{ minX: number; maxX: number; minY: number; maxY: number; spanX?: number; spanY?: number }} ViewBounds */
  /**
   * @param {unknown} value
   * @returns {value is number}
   */
  function isFiniteNumber(value) {
    return typeof value === "number" && Number.isFinite(value);
  }
  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { pointIndex: number }>}
   */
  function hasPointIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "pointIndex" in handle && typeof handle.pointIndex === "number";
  }

  /**
   * @param {PointHandle} handle
   * @returns {handle is Extract<PointHandle, { lineIndex: number }>}
   */
  function hasLineIndexHandle(handle) {
    return !!handle && typeof handle === "object" && "lineIndex" in handle && typeof handle.lineIndex === "number";
  }

  /**
   * @param {Extract<PointTransformJson, { kind: "rotate" }>} transform
   * @param {Map<string, number>} parameters
   * @param {(index: number) => Point | null | undefined} resolvePoint
   * @returns {number | null}
   */
  function resolveRotateTransformAngleDegrees(transform, parameters, resolvePoint) {
    if (
      typeof transform.angleParameterPointIndex === "number"
      && typeof transform.angleParameterStartIndex === "number"
      && typeof transform.angleParameterEndIndex === "number"
    ) {
      const point = resolvePoint(transform.angleParameterPointIndex);
      const start = resolvePoint(transform.angleParameterStartIndex);
      const end = resolvePoint(transform.angleParameterEndIndex);
      if (!point || !start || !end) return null;
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const lenSq = dx * dx + dy * dy;
      if (lenSq <= 1e-9) return null;
      const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSq));
      return t * (transform.angleParameterScale ?? 1);
    }
    if (
      typeof transform.angleStartIndex === "number"
      && typeof transform.angleVertexIndex === "number"
      && typeof transform.angleEndIndex === "number"
    ) {
      const start = resolvePoint(transform.angleStartIndex);
      const vertex = resolvePoint(transform.angleVertexIndex);
      const end = resolvePoint(transform.angleEndIndex);
      if (!start || !vertex || !end) return null;
      const radians = measuredRotationRadians(start, vertex, end);
      return radians === null ? null : radians * 180 / Math.PI;
    }
    if (transform.angleExpr) {
      return evaluateExpr(transform.angleExpr, 0, parameters);
    }
    if (transform.parameterName) {
      return parameters.get(transform.parameterName) ?? null;
    }
    return transform.angleDegrees;
  }

  /**
   * @param {{ centerIndex: number; factor: number; parameterName?: string | null; factorExpr?: FunctionExprJson | null; factorParameterPointIndex?: number | null; factorParameterStartIndex?: number | null; factorParameterEndIndex?: number | null }} transform
   * @param {Map<string, number>} parameters
   * @param {((index: number) => Point | null | undefined) | null} [resolvePointAt]
   * @returns {number | null}
   */
  function resolveScaleTransformFactor(transform, parameters, resolvePointAt = null) {
    if (
      typeof transform.factorParameterPointIndex === "number"
      && typeof transform.factorParameterStartIndex === "number"
      && typeof transform.factorParameterEndIndex === "number"
      && typeof resolvePointAt === "function"
    ) {
      const point = resolvePointAt(transform.factorParameterPointIndex);
      const start = resolvePointAt(transform.factorParameterStartIndex);
      const end = resolvePointAt(transform.factorParameterEndIndex);
      const value = segmentProjectionParameterFromPoints(point, start, end);
      if (Number.isFinite(value)) return value;
    }
    if (transform.factorExpr) {
      return evaluateExpr(transform.factorExpr, 0, parameters);
    }
    if (transform.parameterName) {
      return parameters.get(transform.parameterName) ?? null;
    }
    return transform.factor;
  }

  /**
   * @param {RuntimeLabelJson} label
   */
  function usesVerboseParameterLabel(label) {
    return typeof label.text === "string" && label.text.includes("在");
  }

  /**
   * @param {Point} vertex
   * @param {Point} first
   * @param {Point} second
   * @param {number} shortestLen
   */
  function resolveRightAngleMarkerPoints(vertex, first, second, shortestLen) {
    const side = Math.min(Math.max(shortestLen * 0.125, 10), 28, shortestLen * 0.5);
    if (side <= 1e-9) return null;
    return [
      { x: vertex.x + first.x * side, y: vertex.y + first.y * side },
      { x: vertex.x + (first.x + second.x) * side, y: vertex.y + (first.y + second.y) * side },
      { x: vertex.x + second.x * side, y: vertex.y + second.y * side },
    ];
  }

  /**
   * @param {Point} vertex
   * @param {Point} first
   * @param {number} shortestLen
   * @param {number} cross
   * @param {number} dot
   * @param {number} markerClass
   */
  function resolveArcAngleMarkerPoints(vertex, first, shortestLen, cross, dot, markerClass) {
    const classScale = 1 + 0.18 * Math.max(0, (markerClass || 1) - 1);
    const radius = Math.min(Math.max(shortestLen * 0.12, 10), 28) * classScale;
    const clampedRadius = Math.min(radius, shortestLen * 0.42);
    if (clampedRadius <= 1e-9) return null;
    const delta = Math.atan2(cross, dot);
    if (Math.abs(delta) <= 1e-6) return null;
    const startAngle = Math.atan2(first.y, first.x);
    const samples = 9;
    return Array.from({ length: samples }, (_, index) => {
      const t = index / (samples - 1);
      const angle = startAngle + delta * t;
      return {
        x: vertex.x + clampedRadius * Math.cos(angle),
        y: vertex.y + clampedRadius * Math.sin(angle),
      };
    });
  }

  /**
   * @param {Point} start
   * @param {Point} vertex
   * @param {Point} end
   * @param {number} markerClass
   */
  function resolveAngleMarkerPoints(start, vertex, end, markerClass) {
    const firstDx = start.x - vertex.x;
    const firstDy = start.y - vertex.y;
    const secondDx = end.x - vertex.x;
    const secondDy = end.y - vertex.y;
    const firstLen = Math.hypot(firstDx, firstDy);
    const secondLen = Math.hypot(secondDx, secondDy);
    const shortestLen = Math.min(firstLen, secondLen);
    if (firstLen <= 1e-9 || secondLen <= 1e-9 || shortestLen <= 1e-9) return null;
    const first = { x: firstDx / firstLen, y: firstDy / firstLen };
    const second = { x: secondDx / secondLen, y: secondDy / secondLen };
    const dot = Math.max(-1, Math.min(1, first.x * second.x + first.y * second.y));
    const cross = first.x * second.y - first.y * second.x;
    if (Math.abs(dot) <= 0.12) {
      return resolveRightAngleMarkerPoints(vertex, first, second, shortestLen);
    }
    return resolveArcAngleMarkerPoints(vertex, first, shortestLen, cross, dot, markerClass);
  }

  /**
   * @param {string} op
   * @param {number} x
   * @returns {number | null}
   */
  function evaluateUnary(op, x, degrees = false) {
    const value = degrees ? x * Math.PI / 180 : x;
    switch (op) {
      case "sin": return Math.sin(value);
      case "cos": return Math.cos(value);
      case "tan": return Math.tan(value);
      case "abs": return Math.abs(x);
      case "sqrt": return x >= 0 ? Math.sqrt(x) : null;
      case "ln": return x > 0 ? Math.log(x) : null;
      case "log10": return x > 0 ? Math.log10(x) : null;
      case "sign": return x > 0 ? 1 : (x < 0 ? -1 : 0);
      case "round": return Math.round(x);
      case "trunc": return Math.trunc(x);
      default: return null;
    }
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {number} x
   * @param {Map<string, number>} parameters
   * @returns {number | null}
   */
  function evaluateExpr(expr, x, parameters) {
    if (expr.kind === "constant") return expr.value;
    if (expr.kind === "identity") return x;
    if (expr.kind !== "parsed") return null;
    return evaluateExprAst(expr.expr, x, parameters);
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {number} x
   * @param {Map<string, number>} parameters
   * @returns {number | null}
   */
  function evaluateExprAst(expr, x, parameters) {
    if (!expr || typeof expr !== "object") return null;
    switch (expr.kind) {
      case "variable":
        return x;
      case "constant":
        return expr.value;
      case "parameter":
        return parameters.get(expr.name) ?? expr.value;
      case "pi-angle":
        return 180;
      case "unary": {
        const inner = evaluateExprAst(expr.expr, x, parameters);
        return inner === null ? null : evaluateUnary(expr.op, inner, exprContainsPiAngle(expr.expr));
      }
      case "binary": {
        const lhs = evaluateExprAst(expr.lhs, x, parameters);
        const rhs = evaluateExprAst(expr.rhs, x, parameters);
        if (lhs === null || rhs === null) return null;
        const value = expr.op === "add"
          ? lhs + rhs
          : expr.op === "sub"
            ? lhs - rhs
          : expr.op === "mul"
            ? lhs * rhs
          : expr.op === "div"
            ? (Math.abs(rhs) >= 1e-9 ? lhs / rhs : null)
          : Math.pow(lhs, rhs);
        return value === null || !Number.isFinite(value) ? null : value;
      }
      default:
        return null;
    }
  }

  /**
   * @param {FunctionAstJson | null | undefined} expr
   * @returns {boolean}
   */
  function exprContainsPiAngle(expr) {
    if (!expr || typeof expr !== "object") return false;
    if (expr.kind === "pi-angle") return true;
    if (expr.kind === "unary") return exprContainsPiAngle(expr.expr);
    if (expr.kind === "binary") {
      return exprContainsPiAngle(expr.lhs) || exprContainsPiAngle(expr.rhs);
    }
    return false;
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {(value: number) => string} formatAxisNumber
   * @param {string} [variableLabel]
   */
  function formatExpr(expr, formatAxisNumber, variableLabel = "x") {
    if (expr.kind === "constant") return formatAxisNumber(expr.value);
    if (expr.kind === "identity") return variableLabel;
    if (expr.kind === "parsed") {
      return formatExprAst(expr.expr, formatAxisNumber, variableLabel, 0);
    }
    return "?";
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson | null | undefined} expr
   * @param {(value: number) => string} formatAxisNumber
   * @param {string} [variableLabel]
   * @param {number} [parentPrec]
   * @returns {string}
   */
  function formatExprAst(expr, formatAxisNumber, variableLabel = "x", parentPrec = 0) {
    if (!expr || typeof expr !== "object") return "?";
    switch (expr.kind) {
      case "variable":
        return variableLabel;
      case "constant":
        return formatAxisNumber(expr.value);
      case "parameter":
        return expr.name;
      case "pi-angle":
        return "180";
      case "unary": {
        /** @type {string} */
        const inner = formatExprAst(expr.expr, formatAxisNumber, variableLabel, 0);
        if (expr.op === "abs") return `|${inner}|`;
        if (expr.op === "sqrt") {
          return expr.expr?.kind === "binary" ? `√(${inner})` : `√${inner}`;
        }
        return `${expr.op}(${inner})`;
      }
      case "binary": {
        const { prec, rightAssoc } = binaryPrecedence(expr.op);
        /** @type {string} */
        const left = formatExprAst(expr.lhs, formatAxisNumber, variableLabel, prec);
        /** @type {string} */
        const right = formatExprAst(
          expr.rhs,
          formatAxisNumber,
          variableLabel,
          prec + (rightAssoc ? 0 : 1),
        );
        /** @type {string} */
        const text = `${left}${binaryOpText(expr.op)}${right}`;
        return prec < parentPrec ? `(${text})` : text;
      }
      default:
        return "?";
    }
  }

  /** @param {string} op */
  function binaryPrecedence(op) {
    switch (op) {
      case "add":
      case "sub":
        return { prec: 1, rightAssoc: false };
      case "mul":
      case "div":
        return { prec: 2, rightAssoc: false };
      case "pow":
        return { prec: 3, rightAssoc: true };
      default:
        return { prec: 0, rightAssoc: false };
    }
  }

  /** @param {string} op */
  function binaryOpText(op) {
    switch (op) {
      case "add": return " + ";
      case "sub": return " - ";
      case "mul": return "*";
      case "div": return " / ";
      case "pow": return "^";
      default: return " ? ";
    }
  }

  /**
   * @param {ViewerSceneData | null | undefined} scene
   * @param {Map<string, number>} seedParameters
   */
  function deriveExpressionLabelParameters(scene, seedParameters) {
    const parameters = new Map(seedParameters);
    if (!scene?.labels?.length) {
      return parameters;
    }
    const maxPasses = Math.max(16, scene.labels.length + 1);
    for (let pass = 0; pass < maxPasses; pass += 1) {
      let changed = false;
      scene.labels.forEach((/** @type {RuntimeLabelJson} */ label) => {
        const binding = label.binding;
        if (!binding) {
          return;
        }
        if (
          (binding.kind === "segment-parameter"
            || binding.kind === "segment-projection-parameter"
            || binding.kind === "polyline-parameter"
            || binding.kind === "polygon-boundary-parameter"
            || binding.kind === "circle-parameter")
          && typeof binding.pointName === "string"
        ) {
          const value = labelParameterValueFromBinding(scene, binding);
          const nextValue = isDiscreteIterationParameterName(scene, binding.pointName)
            ? discreteIterationDepth(value)
            : value;
          if (typeof nextValue === "number" && Number.isFinite(nextValue) && parameters.get(binding.pointName) !== nextValue) {
            parameters.set(binding.pointName, nextValue);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-distance-value"
          && typeof binding.name === "string"
        ) {
          const value = pointDistanceValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-angle-value"
          && typeof binding.name === "string"
        ) {
          const value = pointAngleValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "polygon-area-value"
          && typeof binding.name === "string"
        ) {
          const value = polygonAreaValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-distance-ratio-value"
          && typeof binding.name === "string"
        ) {
          const value = pointDistanceRatioValue(scene, binding);
          if (typeof value === "number" && Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          binding.kind === "point-axis-value"
          && typeof binding.name === "string"
        ) {
          const point = scene.points[binding.pointIndex];
          if (!point) {
            return;
          }
          const value = binding.axis === "vertical" ? point.y : point.x;
          if (Number.isFinite(value) && parameters.get(binding.name) !== value) {
            parameters.set(binding.name, value);
            changed = true;
          }
          return;
        }
        if (
          (binding.kind === "expression-value" || binding.kind === "point-bound-expression-value")
          && (typeof binding.resultName === "string" || typeof binding.exprLabel === "string")
        ) {
          const value = evaluateExpr(binding.expr, 0, parameters);
          const resultNames = new Set();
          if (typeof binding.resultName === "string") {
            resultNames.add(binding.resultName);
          }
          if (typeof binding.exprLabel === "string") {
            resultNames.add(binding.exprLabel);
          }
          resultNames.add(formatExpr(binding.expr, formatSequenceValue));
          if (typeof value === "number" && Number.isFinite(value)) {
            resultNames.forEach((/** @type {string} */ resultName) => {
              if (resultName && parameters.get(resultName) !== value) {
                parameters.set(resultName, value);
                changed = true;
              }
            });
          }
        }
      });
      if (!changed) {
        break;
      }
    }
    return parameters;
  }

  /**
   * @param {ViewerSceneData | null | undefined} scene
   * @param {Map<string, number>} seedParameters
   */
  function deriveSequenceLabelParameters(scene, seedParameters) {
    const sequenceLabels = (scene?.labels || [])
      .filter((/** @type {RuntimeLabelJson} */ label) => label.binding?.kind === "sequence-expression-value");
    if (sequenceLabels.length === 0) {
      return seedParameters;
    }
    const parameters = new Map(seedParameters);
    const maxDepth = Math.max(
      ...sequenceLabels.map((/** @type {RuntimeLabelJson} */ label) => pointIterationDepth({
        depth: label.binding.depth,
        parameterName: label.binding.depthParameterName,
      }, parameters)),
    );
    for (let step = 0; step <= maxDepth; step += 1) {
      const derived = deriveExpressionLabelParameters(scene, parameters);
      /** @type {[string, number][]} */
      const updates = [];
      sequenceLabels.forEach((/** @type {RuntimeLabelJson} */ label) => {
        const binding = label.binding;
        const depth = pointIterationDepth({
          depth: binding.depth,
          parameterName: binding.depthParameterName,
        }, derived);
        if (step > depth) {
          return;
        }
        const value = evaluateExpr(binding.expr, 0, derived);
        if (typeof value === "number" && Number.isFinite(value)) {
          updates.push([binding.parameterName, value]);
        }
      });
      if (updates.length === 0) {
        break;
      }
      updates.forEach(([name, value]) => parameters.set(name, value));
    }
    return deriveExpressionLabelParameters(scene, parameters);
  }

  /**
   * @param {ViewerSceneData | null | undefined} scene
   * @param {Map<string, number>} seedParameters
   */
  function deriveLabelParameters(scene, seedParameters) {
    return deriveSequenceLabelParameters(
      scene,
      deriveExpressionLabelParameters(scene, seedParameters),
    );
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function parameterMapForScene(env, scene) {
    return deriveLabelParameters(
      scene,
      new Map(env.currentDynamics().parameters.map((parameter) => [parameter.name, parameter.value])),
    );
  }

  /** @typedef {{ id: string; kind: string; dependsOn: string[]; recipe: string | null }} DependencyNode */
  /** @type {{ nodes: DependencyNode[]; nodeMap: Map<string, DependencyNode>; topoOrder: string[]; reverseEdges: Map<string, string[]> } | null} */
  let dependencyGraphCache = null;

  /** @param {string} name */
  function parameterRootId(name) {
    return `param:${name}`;
  }

  /** @param {number} index */
  function sourcePointRootId(index) {
    return `source-point:${index}`;
  }

  /** @param {number} index */
  function sourceLineRootId(index) {
    return `source-line:${index}`;
  }

  /** @param {number} index */
  function sourceCircleRootId(index) {
    return `source-circle:${index}`;
  }

  /** @param {number} index */
  function sourcePolygonRootId(index) {
    return `source-polygon:${index}`;
  }

  /** @type {Record<string, (env: ViewerEnv, scene: ViewerSceneData) => void>} */
  const GRAPH_RECIPES = {
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

  /**
   * @param {Set<string>} deps
   * @param {string | null | undefined} name
   * @param {Set<string>} knownParameters
   */
  function addKnownParameterDep(deps, name, knownParameters) {
    if (typeof name === "string" && knownParameters.has(name)) {
      deps.add(parameterRootId(name));
    }
  }

  /**
   * @param {Set<string>} deps
   * @param {string | null | undefined} name
   * @param {Set<string>} knownParameters
   * @param {Map<string, Set<string>>} derivedParameterDeps
   */
  function addParameterDep(deps, name, knownParameters, derivedParameterDeps) {
    addKnownParameterDep(deps, name, knownParameters);
    if (typeof name !== "string") return;
    const derivedDeps = derivedParameterDeps.get(name);
    if (!derivedDeps) return;
    derivedDeps.forEach((dep) => deps.add(dep));
  }

  /**
   * @param {Set<string>} deps
   * @param {FunctionExprJson | FunctionAstJson | null | undefined} expr
   * @param {Set<string>} knownParameters
   * @param {Map<string, Set<string>>} [derivedParameterDeps]
   */
  function addExprParameterDeps(deps, expr, knownParameters, derivedParameterDeps = new Map()) {
    const names = new Set();
    collectExprParameterNames(expr, names);
    names.forEach((name) => addParameterDep(deps, name, knownParameters, derivedParameterDeps));
  }

  /**
   * @param {Set<string>} deps
   * @param {unknown} value
   * @param {Set<string>} knownParameters
   * @param {Map<string, Set<string>>} [derivedParameterDeps]
   * @param {SceneData | ViewerSceneData | null} [sourceScene]
   */
  function collectSceneDependencyIds(deps, value, knownParameters, derivedParameterDeps = new Map(), sourceScene = null) {
    if (!value || typeof value !== "object") {
      return;
    }
    if (Array.isArray(value)) {
      value.forEach((entry) => collectSceneDependencyIds(deps, entry, knownParameters, derivedParameterDeps, sourceScene));
      return;
    }
    const addPointRefDep = (/** @type {number} */ index) => {
      deps.add(sourcePointRootId(index));
      const point = sourceScene?.points?.[index];
      if (point?.binding || point?.constraint) {
        deps.add(`point:${index}`);
      }
    };
    const addLineRefDep = (/** @type {number} */ index) => {
      deps.add(sourceLineRootId(index));
      const line = sourceScene?.lines?.[index];
      if (line?.binding) {
        deps.add(`line:${index}`);
      }
    };
    const addCircleRefDep = (/** @type {number} */ index) => {
      deps.add(sourceCircleRootId(index));
      const circle = sourceScene?.circles?.[index];
      if (circle?.binding || circle?.fillColorBinding) {
        deps.add(`circle:${index}`);
      }
    };
    const addPolygonRefDep = (/** @type {number} */ index) => {
      deps.add(sourcePolygonRootId(index));
      const polygon = sourceScene?.polygons?.[index];
      if (polygon?.binding) {
        deps.add(`polygon:${index}`);
      }
    };
    Object.entries(/** @type {Record<string, unknown>} */ (value)).forEach(([key, child]) => {
      if (key === "expr" && child && typeof child === "object") {
        addExprParameterDeps(
          deps,
          /** @type {FunctionExprJson | FunctionAstJson} */ (child),
          knownParameters,
          derivedParameterDeps,
        );
        collectSceneDependencyIds(deps, child, knownParameters, derivedParameterDeps, sourceScene);
        return;
      }
      if (typeof child === "number") {
        if (
          key === "pointIndex"
          || key === "targetPointIndex"
          || key === "sourceIndex"
          || key === "centerIndex"
          || key === "originIndex"
          || key === "xUnitIndex"
          || key === "yUnitIndex"
          || key === "denominatorIndex"
          || key === "numeratorIndex"
          || key === "ratioOriginIndex"
          || key === "ratioDenominatorIndex"
          || key === "ratioNumeratorIndex"
          || key === "radiusIndex"
          || key === "startIndex"
          || key === "endIndex"
          || key === "leftIndex"
          || key === "rightIndex"
          || key === "midIndex"
          || key === "throughIndex"
          || key === "vertexIndex"
          || key === "lineStartIndex"
          || key === "lineEndIndex"
          || key === "sourceCenterIndex"
          || key === "sourceNextCenterIndex"
          || key === "reflectionSourceIndex"
          || key === "vectorStartIndex"
          || key === "vectorEndIndex"
          || key === "startControlIndex"
          || key === "endControlIndex"
          || key === "anchorYPointIndex"
          || key === "driverIndex"
          || key === "seedIndex"
          || key === "pointSeedIndex"
          || key === "angleParameterPointIndex"
          || key === "angleParameterStartIndex"
          || key === "angleParameterEndIndex"
          || key === "factorParameterPointIndex"
          || key === "factorParameterStartIndex"
          || key === "factorParameterEndIndex"
          || key === "reflectionFocusIndex"
        ) {
          addPointRefDep(child);
          return;
        }
        if (
          key === "lineIndex"
          || key === "traceLineIndex"
          || key === "reflectionAxisLineIndex"
          || key === "reflectionDirectrixLineIndex"
        ) {
          addLineRefDep(child);
          return;
        }
        if (key === "circleIndex" || key === "sourceCircleIndex") {
          addCircleRefDep(child);
          return;
        }
        if (key === "polygonIndex") {
          addPolygonRefDep(child);
        }
        return;
      }
      if (Array.isArray(child)) {
        if (key === "vertexIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addPointRefDep(entry);
            }
          });
        } else if (
          key === "pointIndices"
          || key === "constrainedPointIndices"
        ) {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addPointRefDep(entry);
            }
          });
        } else if (key === "lineIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addLineRefDep(entry);
            }
          });
        } else if (key === "circleIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addCircleRefDep(entry);
            }
          });
        } else if (key === "polygonIndices") {
          child.forEach((entry) => {
            if (typeof entry === "number") {
              addPolygonRefDep(entry);
            }
          });
        }
        child.forEach((entry) => collectSceneDependencyIds(deps, entry, knownParameters, derivedParameterDeps, sourceScene));
        return;
      }
      if (typeof child === "string") {
        if (
          key === "parameterName"
          || key === "depthParameterName"
          || key === "traceParameterName"
          || key === "pointName"
          || key === "name"
          || key === "resultName"
        ) {
          addParameterDep(deps, child, knownParameters, derivedParameterDeps);
        }
        return;
      }
      collectSceneDependencyIds(deps, child, knownParameters, derivedParameterDeps, sourceScene);
    });
  }

  /**
   * @param {RuntimeLabelJson} label
   * @returns {string | null}
   */
  function labelDerivedParameterName(label) {
    const binding = label.binding;
    if (!binding) return null;
    if (
      (binding.kind === "point-distance-ratio-value"
        || binding.kind === "point-distance-value"
        || binding.kind === "point-angle-value"
        || binding.kind === "polygon-area-value"
        || binding.kind === "parameter-value"
        || binding.kind === "point-axis-value")
      && typeof binding.name === "string"
    ) {
      return binding.name;
    }
    if (
      (binding.kind === "segment-parameter"
        || binding.kind === "segment-projection-parameter"
        || binding.kind === "polyline-parameter"
        || binding.kind === "polygon-boundary-parameter"
        || binding.kind === "circle-parameter")
      && typeof binding.pointName === "string"
    ) {
      return binding.pointName;
    }
    if (
      (binding.kind === "expression-value" || binding.kind === "point-bound-expression-value")
      && typeof binding.resultName === "string"
    ) {
      return binding.resultName;
    }
    return null;
  }

  /**
   * @param {{ labels?: RuntimeLabelJson[] }} scene
   * @param {Set<string>} knownParameters
   * @returns {Map<string, Set<string>>}
   */
  function collectLabelDerivedParameterDeps(scene, knownParameters) {
    /** @type {{ name: string, directDeps: Set<string>, exprNames: Set<string> }[]} */
    const defs = [];
    (scene.labels || []).forEach((/** @type {RuntimeLabelJson} */ label) => {
      const name = labelDerivedParameterName(label);
      if (!name || !label.binding) return;
      /** @type {Set<string>} */
      const directDeps = new Set();
      collectSceneDependencyIds(directDeps, label.binding, knownParameters);
      /** @type {Set<string>} */
      const exprNames = new Set();
      if ("expr" in label.binding) {
        collectExprParameterNames(label.binding.expr, exprNames);
      }
      defs.push({ name, directDeps, exprNames });
    });

    /** @type {Map<string, Set<string>>} */
    const depsByName = new Map();
    defs.forEach((def) => depsByName.set(def.name, new Set(def.directDeps)));
    for (let pass = 0; pass < 4; pass += 1) {
      let changed = false;
      defs.forEach((def) => {
        /** @type {Set<string>} */
        const deps = new Set(def.directDeps);
        def.exprNames.forEach((name) => {
          addParameterDep(deps, name, knownParameters, depsByName);
        });
        const current = depsByName.get(def.name) || new Set();
        deps.forEach((dep) => {
          if (!current.has(dep)) {
            current.add(dep);
            changed = true;
          }
        });
        depsByName.set(def.name, current);
      });
      if (!changed) break;
    }
    return depsByName;
  }

  /**
   * @param {ViewerEnv} env
   * @returns {{ nodes: DependencyNode[]; nodeMap: Map<string, DependencyNode>; topoOrder: string[]; reverseEdges: Map<string, string[]> }}
   */
  function ensureDependencyGraph(env) {
    if (dependencyGraphCache) {
      return dependencyGraphCache;
    }
    /** @type {DependencyNode[]} */
    const nodes = [];
    /** @type {Map<string, DependencyNode>} */
    const nodeMap = new Map();
    const knownParameters = new Set((env.currentDynamics().parameters || []).map((parameter) => parameter.name));
    const derivedParameterDeps = collectLabelDerivedParameterDeps(env.sourceScene, knownParameters);
    const collectDeps = (/** @type {Set<string>} */ deps, /** @type {unknown} */ value) => {
      collectSceneDependencyIds(deps, value, knownParameters, derivedParameterDeps, env.sourceScene);
    };

    /** @param {DependencyNode} node */
    const addNode = (node) => {
      const normalized = {
        ...node,
        dependsOn: [...new Set((node.dependsOn || []).filter((dep) => dep !== node.id))],
      };
      nodes.push(normalized);
      nodeMap.set(normalized.id, normalized);
    };

    (env.currentDynamics().parameters || []).forEach((parameter) => {
      addNode({
        id: parameterRootId(parameter.name),
        kind: "parameter-root",
        dependsOn: [],
        recipe: null,
      });
      addNode({
        id: `parameter-sync:${parameter.name}`,
        kind: "parameter-sync",
        dependsOn: [parameterRootId(parameter.name)],
        recipe: "sync-base-dynamics",
      });
    });
    (env.sourceScene.points || []).forEach((_, index) => {
      addNode({ id: sourcePointRootId(index), kind: "source-point", dependsOn: [], recipe: null });
    });
    (env.sourceScene.lines || []).forEach((_, index) => {
      addNode({ id: sourceLineRootId(index), kind: "source-line", dependsOn: [], recipe: null });
    });
    (env.sourceScene.circles || []).forEach((_, index) => {
      addNode({ id: sourceCircleRootId(index), kind: "source-circle", dependsOn: [], recipe: null });
    });
    (env.sourceScene.polygons || []).forEach((_, index) => {
      addNode({ id: sourcePolygonRootId(index), kind: "source-polygon", dependsOn: [], recipe: null });
    });

    (env.sourceScene.points || []).forEach((point, index) => {
      if (!point.binding && !point.constraint) return;
      const deps = new Set();
      collectDeps(deps, point.binding);
      collectDeps(deps, point.constraint);
      addNode({
        id: `point:${index}`,
        kind: "point",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.sourceScene.lines || []).forEach((line, index) => {
      if (!line.binding) return;
      const deps = new Set();
      collectDeps(deps, line.binding);
      if (line.binding.kind === "point-trace") {
        [line.binding.pointIndex, line.binding.driverIndex].forEach((/** @type {number} */ pointIndex) => {
          const point = env.sourceScene.points?.[pointIndex];
          collectDeps(deps, point?.binding);
          collectDeps(deps, point?.constraint);
        });
      }
      addNode({
        id: `line:${index}`,
        kind: "line",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.sourceScene.circles || []).forEach((circle, index) => {
      if (!circle.binding && !circle.fillColorBinding) return;
      const deps = new Set();
      collectDeps(deps, circle.binding);
      collectDeps(deps, circle.fillColorBinding);
      addNode({
        id: `circle:${index}`,
        kind: "circle",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.sourceScene.polygons || []).forEach((polygon, index) => {
      if (!polygon.binding) return;
      const deps = new Set();
      collectDeps(deps, polygon.binding);
      addNode({
        id: `polygon:${index}`,
        kind: "polygon",
        dependsOn: [...deps],
        recipe: "refresh-derived-points",
      });
    });

    (env.currentDynamics().functions || []).forEach((functionDef, index) => {
      const deps = new Set();
      addExprParameterDeps(deps, functionDef.expr, knownParameters, derivedParameterDeps);
      collectDeps(deps, functionDef.constrainedPointIndices);
      addNode({
        id: `function:${index}`,
        kind: "function",
        dependsOn: [...deps],
        recipe: "sync-base-dynamics",
      });
    });

    (env.sourceScene.labels || []).forEach((label, index) => {
      if (!label.binding) return;
      const deps = new Set();
      collectDeps(deps, label.binding);
      if ("expr" in label.binding) {
        addExprParameterDeps(deps, label.binding.expr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `label:${index}`,
        kind: "label",
        dependsOn: [...deps],
        recipe: "refresh-dynamic-labels",
      });
    });

    (env.sourceScene.pointIterations || []).forEach((family, index) => {
      const deps = new Set();
      collectDeps(deps, family);
      if (family.kind === "rotate") {
        addExprParameterDeps(deps, family.angleExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `point-iteration:${index}`,
        kind: "point-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.circleIterations || []).forEach((family, index) => {
      const deps = new Set();
      collectDeps(deps, family);
      addNode({
        id: `circle-iteration:${index}`,
        kind: "circle-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.lineIterations || []).forEach((family, index) => {
      const deps = new Set();
      collectDeps(deps, family);
      if (family.kind === "rotate") {
        addExprParameterDeps(deps, family.angleExpr, knownParameters, derivedParameterDeps);
      }
      if ("depthExpr" in family) {
        addExprParameterDeps(deps, family.depthExpr, knownParameters, derivedParameterDeps);
      }
      if (family.kind === "parameterized-point-trace") {
        addExprParameterDeps(deps, family.stepExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `line-iteration:${index}`,
        kind: "line-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.polygonIterations || []).forEach((family, index) => {
      const deps = new Set();
      collectDeps(deps, family);
      if (family.kind === "coordinate-grid") {
        addExprParameterDeps(deps, family.stepExpr, knownParameters, derivedParameterDeps);
        addExprParameterDeps(deps, family.xExpr, knownParameters, derivedParameterDeps);
        addExprParameterDeps(deps, family.yExpr, knownParameters, derivedParameterDeps);
        addExprParameterDeps(deps, family.depthExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `polygon-iteration:${index}`,
        kind: "polygon-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.labelIterations || []).forEach((family, index) => {
      const deps = new Set();
      collectDeps(deps, family);
      addExprParameterDeps(deps, family.expr, knownParameters, derivedParameterDeps);
      if ("depthExpr" in family) {
        addExprParameterDeps(deps, family.depthExpr, knownParameters, derivedParameterDeps);
      }
      addNode({
        id: `label-iteration:${index}`,
        kind: "label-iteration",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });
    (env.sourceScene.iterationTables || []).forEach((table, index) => {
      const deps = new Set();
      collectDeps(deps, table);
      addExprParameterDeps(deps, table.expr, knownParameters, derivedParameterDeps);
      addNode({
        id: `iteration-table:${index}`,
        kind: "iteration-table",
        dependsOn: [...deps],
        recipe: "rebuild-iteration-geometry",
      });
    });

    /** @type {Map<string, number>} */
    const indegree = new Map();
    /** @type {Map<string, string[]>} */
    const reverseEdges = new Map();
    nodes.forEach((node) => {
      indegree.set(node.id, 0);
    });
    nodes.forEach((node) => {
      node.dependsOn.forEach((dep) => {
        if (!nodeMap.has(dep)) {
          return;
        }
        indegree.set(node.id, (indegree.get(node.id) || 0) + 1);
        const dependents = reverseEdges.get(dep) || [];
        dependents.push(node.id);
        reverseEdges.set(dep, dependents);
      });
    });
    const queue = nodes
      .filter((node) => (indegree.get(node.id) || 0) === 0)
      .map((node) => node.id);
    /** @type {string[]} */
    const topoOrder = [];
    while (queue.length > 0) {
      const id = /** @type {string} */ (queue.shift());
      topoOrder.push(id);
      (reverseEdges.get(id) || []).forEach((dependentId) => {
        const nextDegree = (indegree.get(dependentId) || 0) - 1;
        indegree.set(dependentId, nextDegree);
        if (nextDegree === 0) {
          queue.push(dependentId);
        }
      });
    }
    nodes.forEach((node) => {
      if (!topoOrder.includes(node.id)) {
        topoOrder.push(node.id);
      }
    });

    dependencyGraphCache = { nodes, nodeMap, topoOrder, reverseEdges };
    return dependencyGraphCache;
  }

  /**
   * @param {ViewerEnv} env
   */
  function describeDependencyGraph(env) {
    const graph = ensureDependencyGraph(env);
    return graph.topoOrder
      .map((id) => graph.nodeMap.get(id))
      .filter((/** @type {DependencyNode | undefined} */ node) => !!node)
      .map((node) => ({
        id: node.id,
        kind: node.kind,
        dependsOn: [...node.dependsOn],
        recipe: node.recipe,
      }));
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson | null | undefined} expr
   * @param {Set<string>} names
   */
  function collectExprParameterNames(expr, names) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parsed") {
      collectExprAstParameterNames(expr.expr, names);
    }
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {Set<string>} names
   */
  function collectExprAstParameterNames(expr, names) {
    if (!expr || typeof expr !== "object") return;
    if (expr.kind === "parameter" && typeof expr.name === "string") {
      names.add(expr.name);
      return;
    }
    if (expr.kind === "unary") {
      collectExprAstParameterNames(expr.expr, names);
      return;
    }
    if (expr.kind === "binary") {
      collectExprAstParameterNames(expr.lhs, names);
      collectExprAstParameterNames(expr.rhs, names);
    }
  }

  /**
   * @param {FunctionJson} functionDef
   * @param {Map<string, number>} parameters
   */
  function sampleDynamicFunction(functionDef, parameters) {
    const segments = [];
    let points = [];
    const last = Math.max(1, functionDef.domain.sampleCount - 1);
    for (let index = 0; index < functionDef.domain.sampleCount; index += 1) {
      const t = index / last;
      const x = functionDef.domain.xMin + (functionDef.domain.xMax - functionDef.domain.xMin) * t;
      const y = evaluateExpr(functionDef.expr, x, parameters);
      if (y === null) {
        if (points.length >= 2) {
          segments.push(points);
        }
        points = [];
        continue;
      }
      if (functionDef.domain.plotMode === "polar") {
        points.push({
          x: y * Math.cos(x),
          y: y * Math.sin(x),
        });
      } else {
        points.push({ x, y });
      }
    }
    if (points.length >= 2) {
      segments.push(points);
    }
    return segments;
  }

  /**
   * @param {{ xExpr: FunctionExprJson, yExpr: FunctionExprJson, xMin: number, xMax: number, sampleCount: number }} binding
   * @param {Map<string, number>} parameters
   * @returns {Point[]}
   */
  function sampleParametricCurve(binding, parameters) {
    const points = [];
    const last = Math.max(1, binding.sampleCount - 1);
    for (let index = 0; index < binding.sampleCount; index += 1) {
      const t = index / last;
      const value = binding.xMin + (binding.xMax - binding.xMin) * t;
      const x = evaluateExpr(binding.xExpr, value, parameters);
      const y = evaluateExpr(binding.yExpr, value, parameters);
      if (x === null || y === null) continue;
      points.push({ x, y });
    }
    return points;
  }

  /** @param {number} value */
  function wrapUnitInterval(value) {
    return ((value % 1) + 1) % 1;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function circleParameterFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (constraint?.kind !== "circle" && constraint?.kind !== "circular-constraint") {
      return null;
    }
    const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
    const tau = Math.PI * 2;
    return ((pointAngle % tau) + tau) % tau / tau;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {{ originIndex: number, denominatorIndex: number, numeratorIndex: number, clampToUnit?: boolean }} binding
   */
  function pointDistanceRatioValue(scene, binding) {
    const origin = scene.points[binding.originIndex];
    const denominator = scene.points[binding.denominatorIndex];
    const numerator = scene.points[binding.numeratorIndex];
    if (!origin || !denominator || !numerator) return null;
    const denominatorLength = Math.hypot(denominator.x - origin.x, denominator.y - origin.y);
    if (denominatorLength <= 1e-9) return null;
    const ratio = Math.hypot(numerator.x - origin.x, numerator.y - origin.y) / denominatorLength;
    return binding.clampToUnit === true ? Math.min(ratio, 1) : ratio;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {{ leftIndex: number, rightIndex: number, valueScale?: number | null }} binding
   */
  function pointDistanceValue(scene, binding) {
    const left = scene.points[binding.leftIndex];
    const right = scene.points[binding.rightIndex];
    if (!left || !right) return null;
    return Math.hypot(right.x - left.x, right.y - left.y) * (binding.valueScale ?? 1);
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {{ startIndex: number, vertexIndex: number, endIndex: number }} binding
   */
  function pointAngleValue(scene, binding) {
    const start = scene.points[binding.startIndex];
    const vertex = scene.points[binding.vertexIndex];
    const end = scene.points[binding.endIndex];
    if (!start || !vertex || !end) return null;
    const first = { x: start.x - vertex.x, y: start.y - vertex.y };
    const second = { x: end.x - vertex.x, y: end.y - vertex.y };
    const firstLen = Math.hypot(first.x, first.y);
    const secondLen = Math.hypot(second.x, second.y);
    if (firstLen <= 1e-9 || secondLen <= 1e-9) return null;
    const cross = (first.x / firstLen) * (second.y / secondLen)
      - (first.y / firstLen) * (second.x / secondLen);
    const dot = (first.x / firstLen) * (second.x / secondLen)
      + (first.y / firstLen) * (second.y / secondLen);
    return Math.abs(Math.atan2(cross, dot)) * 180 / Math.PI;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {{ pointIndices: number[], valueScale?: number | null }} binding
   */
  function polygonAreaValue(scene, binding) {
    const points = binding.pointIndices.map((index) => scene.points[index]);
    if (points.length < 3 || points.some((point) => !point)) return null;
    let twiceArea = 0;
    for (let index = 0; index < points.length; index += 1) {
      const left = points[index];
      const right = points[(index + 1) % points.length];
      twiceArea += left.x * right.y - right.x * left.y;
    }
    return Math.abs(twiceArea) * 0.5 * (binding.valueScale ?? 1);
  }

  /**
   * @param {Point | null | undefined} point
   * @param {Point | null | undefined} origin
   * @param {Point | null | undefined} xUnit
   * @param {Point | null | undefined} yUnit
   */
  function pointCoordinatesInBasis(point, origin, xUnit, yUnit) {
    if (!point || !origin || !xUnit || !yUnit) return null;
    const xAxisX = xUnit.x - origin.x;
    const xAxisY = xUnit.y - origin.y;
    const yAxisX = yUnit.x - origin.x;
    const yAxisY = yUnit.y - origin.y;
    const pointX = point.x - origin.x;
    const pointY = point.y - origin.y;
    const det = xAxisX * yAxisY - xAxisY * yAxisX;
    if (!Number.isFinite(det) || Math.abs(det) <= 1e-9) return null;
    return {
      x: (pointX * yAxisY - pointY * yAxisX) / det,
      y: (xAxisX * pointY - xAxisY * pointX) / det,
    };
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {{ pointIndex: number, startIndex: number, endIndex: number }} binding
   */
  function segmentProjectionParameterFromBinding(scene, binding) {
    const point = scene.points[binding.pointIndex];
    const start = scene.points[binding.startIndex];
    const end = scene.points[binding.endIndex];
    return segmentProjectionParameterFromPoints(point, start, end);
  }

  /**
   * @param {Point | null | undefined} point
   * @param {Point | null | undefined} start
   * @param {Point | null | undefined} end
   */
  function segmentProjectionParameterFromPoints(point, start, end) {
    if (!point || !start || !end) return null;
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) return null;
    const t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / lenSq;
    return Math.max(0, Math.min(1, t));
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function polygonBoundaryParameterFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (
      !constraint
      || (constraint.kind !== "polygon-boundary" && constraint.kind !== "translated-polygon-boundary")
      || constraint.vertexIndices.length < 2
    ) {
      return null;
    }

    const count = constraint.vertexIndices.length;
    let perimeter = 0;
    let traveled = 0;
    for (let index = 0; index < count; index += 1) {
      const start = scene.points[constraint.vertexIndices[index]];
      const end = scene.points[constraint.vertexIndices[(index + 1) % count]];
      if (!start || !end) {
        return null;
      }
      const length = Math.hypot(end.x - start.x, end.y - start.y);
      perimeter += length;
      if (index < constraint.edgeIndex) {
        traveled += length;
      } else if (index === constraint.edgeIndex) {
        traveled += length * Math.max(0, Math.min(1, constraint.t));
      }
    }

    return perimeter > 1e-9 ? traveled / perimeter : null;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function polylineParameterFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (constraint?.kind !== "polyline" || !Array.isArray(constraint.points) || constraint.points.length < 2) {
      return null;
    }
    const segmentIndex = Number.isFinite(constraint.segmentIndex) ? constraint.segmentIndex : 0;
    const t = Number.isFinite(constraint.t) ? Math.max(0, Math.min(1, constraint.t)) : 0;
    return (segmentIndex + t) / (constraint.points.length - 1);
  }

  /**
   * @param {Point[]} points
   * @param {number} normalized
   * @returns {Point | null}
   */
  function pointOnPolylineByIndex(points, normalized) {
    if (!Array.isArray(points) || points.length < 2 || !Number.isFinite(normalized)) {
      return null;
    }
    const wrapped = ((normalized % 1) + 1) % 1;
    const scaled = wrapped * (points.length - 1);
    const segmentIndex = Math.max(0, Math.min(points.length - 2, Math.floor(scaled)));
    const t = scaled - segmentIndex;
    const start = points[segmentIndex];
    const end = points[segmentIndex + 1];
    if (!start || !end) return null;
    return {
      x: start.x + (end.x - start.x) * t,
      y: start.y + (end.y - start.y) * t,
    };
  }

  /**
   * @param {Point[]} vertices
   * @param {number} parameter
   * @returns {Point | null}
   */
  function pointOnPolygonBoundary(vertices, parameter) {
    if (!vertices || vertices.length < 2) {
      return null;
    }
    const wrapped = ((parameter % 1) + 1) % 1;
    const lengths = [];
    let perimeter = 0;
    for (let index = 0; index < vertices.length; index += 1) {
      const start = vertices[index];
      const end = vertices[(index + 1) % vertices.length];
      const length = Math.hypot(end.x - start.x, end.y - start.y);
      lengths.push(length);
      perimeter += length;
    }
    if (perimeter <= 1e-9) {
      return null;
    }
    const target = wrapped * perimeter;
    let traveled = 0;
    for (let edgeIndex = 0; edgeIndex < lengths.length; edgeIndex += 1) {
      const length = lengths[edgeIndex];
      if (traveled + length >= target || edgeIndex === lengths.length - 1) {
        const start = vertices[edgeIndex];
        const end = vertices[(edgeIndex + 1) % vertices.length];
        const localT = length <= 1e-9 ? 0 : Math.max(0, Math.min(1, (target - traveled) / length));
        return {
          x: start.x + (end.x - start.x) * localT,
          y: start.y + (end.y - start.y) * localT,
        };
      }
      traveled += length;
    }
    return null;
  }

  /** @type {Record<string, PointConstraintParameterReader>} */
  const POINT_CONSTRAINT_PARAMETER_READERS = {
    segment: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    line: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    "line-constraint": (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    ray: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    "ray-constraint": (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    polyline: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    "polygon-boundary": polygonBoundaryParameterFromPoint,
    "translated-polygon-boundary": polygonBoundaryParameterFromPoint,
    circle: circleParameterFromPoint,
    "circle-arc": (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
    arc: (scene, pointIndex) => scene.points[pointIndex]?.constraint?.t ?? null,
  };

  /** @type {Record<string, PointConstraintParameterApplier>} */
  const POINT_CONSTRAINT_PARAMETER_APPLIERS = {
    segment(point, _scene, value) {
      point.constraint.t = wrapUnitInterval(value);
    },
    line(point, _scene, value) {
      point.constraint.t = value;
    },
    "line-constraint"(point, _scene, value) {
      point.constraint.t = value;
    },
    ray(point, _scene, value) {
      point.constraint.t = Math.max(0, value);
    },
    "ray-constraint"(point, _scene, value) {
      point.constraint.t = Math.max(0, value);
    },
    polyline(point, _scene, value) {
      point.constraint.t = wrapUnitInterval(value);
    },
    "polygon-boundary"(point, scene, value) {
      const wrapped = wrapUnitInterval(value);
      const count = point.constraint.vertexIndices.length;
      if (count < 2) return;
      const lengths = [];
      let perimeter = 0;
      for (let i = 0; i < count; i += 1) {
        const start = scene.points[point.constraint.vertexIndices[i]];
        const end = scene.points[point.constraint.vertexIndices[(i + 1) % count]];
        if (!start || !end) return;
        const length = Math.hypot(end.x - start.x, end.y - start.y);
        lengths.push(length);
        perimeter += length;
      }
      if (perimeter <= 1e-9) return;
      const target = wrapped * perimeter;
      let traveled = 0;
      for (let edgeIndex = 0; edgeIndex < lengths.length; edgeIndex += 1) {
        const length = lengths[edgeIndex];
        if (traveled + length >= target || edgeIndex === lengths.length - 1) {
          point.constraint.edgeIndex = edgeIndex;
          point.constraint.t = length <= 1e-9 ? 0 : Math.max(0, Math.min(1, (target - traveled) / length));
          return;
        }
        traveled += length;
      }
    },
    "translated-polygon-boundary"(point, scene, value) {
      POINT_CONSTRAINT_PARAMETER_APPLIERS["polygon-boundary"](point, scene, value);
    },
    circle(point, _scene, value) {
      const wrapped = wrapUnitInterval(value);
      const angle = Math.PI * 2 * wrapped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    },
    "circular-constraint"(point, _scene, value) {
      const wrapped = wrapUnitInterval(value);
      const angle = Math.PI * 2 * wrapped;
      point.constraint.unitX = Math.cos(angle);
      point.constraint.unitY = -Math.sin(angle);
    },
    "circle-arc"(point, _scene, value) {
      point.constraint.t = wrapUnitInterval(value);
    },
    arc(point, _scene, value) {
      point.constraint.t = wrapUnitInterval(value);
    },
  };

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   */
  function parameterValueFromPoint(scene, pointIndex) {
    const point = scene.points[pointIndex];
    const constraint = point?.constraint;
    if (!constraint) return null;
    const readParameter = POINT_CONSTRAINT_PARAMETER_READERS[constraint.kind];
    return readParameter ? readParameter(scene, pointIndex) : null;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {LabelBindingJson} binding
   */
  function labelParameterValueFromBinding(scene, binding) {
    if (binding.kind === "segment-projection-parameter") {
      return segmentProjectionParameterFromBinding(scene, binding);
    }
    if (binding.kind === "polyline-parameter") {
      return polylineParameterFromPoint(scene, binding.pointIndex);
    }
    if (binding.kind === "polygon-boundary-parameter") {
      return polygonBoundaryParameterFromPoint(scene, binding.pointIndex);
    }
    return "pointIndex" in binding && typeof binding.pointIndex === "number"
      ? parameterValueFromPoint(scene, binding.pointIndex)
      : null;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {number} pointIndex
   * @returns {string | null}
   */
  function parameterNameFromPoint(scene, pointIndex) {
    for (const label of scene.labels || []) {
      const binding = label?.binding;
      if (!binding || binding.pointIndex !== pointIndex || typeof binding.pointName !== "string") {
        continue;
      }
      if (
        binding.kind === "segment-parameter"
        || binding.kind === "polyline-parameter"
        || binding.kind === "polygon-boundary-parameter"
        || binding.kind === "circle-parameter"
      ) {
        return binding.pointName;
      }
    }
    return null;
  }

  /** @param {number | null | undefined} value */
  function clampNormalizedValue(value) {
    return typeof value === "number" && Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : null;
  }

  /**
   * @param {number} hue
   * @param {number} saturation
   * @param {number} brightness
   * @param {number} alpha
   * @returns {[number, number, number, number]}
   */
  function hsbToRgba(hue, saturation, brightness, alpha) {
    const wrappedHue = wrapUnitInterval(hue);
    const s = Math.max(0, Math.min(1, saturation));
    const v = Math.max(0, Math.min(1, brightness));
    if (s <= 1e-9) {
      const channel = Math.round(v * 255);
      return [channel, channel, channel, alpha];
    }
    const scaled = wrappedHue * 6;
    const sector = Math.floor(scaled) % 6;
    const fraction = scaled - Math.floor(scaled);
    const p = v * (1 - s);
    const q = v * (1 - s * fraction);
    const t = v * (1 - s * (1 - fraction));
    const [r, g, b] = (() => {
      switch (sector) {
        case 0: return [v, t, p];
        case 1: return [q, v, p];
        case 2: return [p, v, t];
        case 3: return [p, q, v];
        case 4: return [t, p, v];
        default: return [v, p, q];
      }
    })();
    return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255), alpha];
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {RuntimeCircleJson} circle
   */
  function refreshCircleFillColorBinding(scene, circle) {
    const binding = circle.fillColorBinding;
    if (!binding) return;
    if (binding.kind === "rgb") {
      const red = clampNormalizedValue(parameterValueFromPoint(scene, binding.redPointIndex));
      const green = clampNormalizedValue(parameterValueFromPoint(scene, binding.greenPointIndex));
      const blue = clampNormalizedValue(parameterValueFromPoint(scene, binding.bluePointIndex));
      if (red === null || green === null || blue === null) return;
      circle.fillColor = [
        Math.round(red * 255),
        Math.round(green * 255),
        Math.round(blue * 255),
        binding.alpha,
      ];
      return;
    }
    if (binding.kind === "hsb") {
      const hue = clampNormalizedValue(parameterValueFromPoint(scene, binding.huePointIndex));
      const saturation = clampNormalizedValue(parameterValueFromPoint(scene, binding.saturationPointIndex));
      const brightness = clampNormalizedValue(parameterValueFromPoint(scene, binding.brightnessPointIndex));
      if (hue === null || saturation === null || brightness === null) return;
      circle.fillColor = hsbToRgba(hue, saturation, brightness, binding.alpha);
    }
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {ViewerSceneData} scene
   * @param {number | null | undefined} value
   */
  function applyNormalizedParameterToPoint(point, scene, value) {
    if (typeof value !== "number") return;
    if (!point.constraint) return;
    const applyParameter = POINT_CONSTRAINT_PARAMETER_APPLIERS[point.constraint.kind];
    if (applyParameter) {
      applyParameter(point, scene, value);
    }
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {ViewerSceneData} scene
   * @param {number | null | undefined} value
   * @param {number} xMin
   * @param {number} xMax
   */
  function applyTraceValueToPoint(point, scene, value, xMin, xMax) {
    if (typeof value !== "number") return;
    if (!point?.constraint) return;
    if (point.constraint.kind === "circle" || point.constraint.kind === "circular-constraint") {
      point.constraint.unitX = Math.cos(value);
      point.constraint.unitY = -Math.sin(value);
      return;
    }
    if (
      point.constraint.kind === "line"
      || point.constraint.kind === "line-constraint"
      || point.constraint.kind === "ray"
      || point.constraint.kind === "ray-constraint"
    ) {
      applyNormalizedParameterToPoint(point, scene, value);
      return;
    }
    const normalized = Math.abs(xMax - xMin) <= 1e-9
      ? 0
      : Math.max(0, Math.min(1, (value - xMin) / (xMax - xMin)));
    applyNormalizedParameterToPoint(point, scene, normalized);
  }

  /**
   * @param {{ depth: number; parameterName?: string | null; depthParameterName?: string | null; depthExpr?: FunctionExprJson | null }} family
   * @param {Map<string, number>} parameters
   */
  function pointIterationDepth(family, parameters) {
    const rawValue = family.depthParameterName
      ? parameters.get(family.depthParameterName)
      : family.depthExpr
        ? Math.max(
          Number.isFinite(family.depth) ? family.depth : 0,
          evaluateExpr(family.depthExpr, 0, parameters) ?? 0,
        )
      : family.parameterName
        ? parameters.get(family.parameterName)
        : family.depth;
    const fallback = Number.isFinite(family.depth) ? family.depth : 0;
    const depth = typeof rawValue === "number" && Number.isFinite(rawValue) ? rawValue : fallback;
    return discreteIterationDepth(depth);
  }

  /**
   * Iteration counts are discrete payload values. Avoid rounding up early while a
   * live control is between integer steps.
   *
   * @param {number | null | undefined} value
   */
  function discreteIterationDepth(value) {
    if (typeof value !== "number" || !Number.isFinite(value)) {
      return 0;
    }
    return Math.max(0, Math.floor(value + 1e-9));
  }

  /**
   * @param {ViewerSceneData | SceneData | null | undefined} scene
   * @returns {Set<string>}
   */
  function collectDiscreteIterationParameterNames(scene) {
    const names = new Set();
    const add = (/** @type {string | null | undefined} */ name) => {
      if (typeof name === "string" && name.length > 0) {
        names.add(name);
      }
    };
    (scene?.pointIterations || []).forEach((family) => {
      if ("parameterName" in family) {
        add(family.parameterName);
      }
    });
    (scene?.circleIterations || []).forEach((family) => add(family.depthParameterName));
    (scene?.lineIterations || []).forEach((family) => {
      if ("parameterName" in family) {
        add(family.parameterName);
      }
      if ("depthParameterName" in family) {
        add(family.depthParameterName);
      }
    });
    (scene?.lines || []).forEach((line) => {
      if (line.binding?.kind === "colorized-spectrum") {
        add(line.binding.depthParameterName);
      }
    });
    (scene?.polygonIterations || []).forEach((family) => add(family.parameterName));
    (scene?.labelIterations || []).forEach((family) => add(family.depthParameterName));
    (scene?.iterationTables || []).forEach((table) => add(table.depthParameterName));
    return names;
  }

  /**
   * @param {ViewerSceneData | SceneData | null | undefined} scene
   * @param {string} name
   * @returns {boolean}
   */
  function isDiscreteIterationParameterName(scene, name) {
    return collectDiscreteIterationParameterNames(scene).has(name);
  }

  /**
   * @param {Point[]} sourceTriangle
   * @param {Point[]} targetTriangle
   */
  function affineMapFromTriangles(sourceTriangle, targetTriangle) {
    const sourceOrigin = sourceTriangle[0];
    const su = {
      x: sourceTriangle[1].x - sourceOrigin.x,
      y: sourceTriangle[1].y - sourceOrigin.y,
    };
    const sv = {
      x: sourceTriangle[2].x - sourceOrigin.x,
      y: sourceTriangle[2].y - sourceOrigin.y,
    };
    const det = su.x * sv.y - su.y * sv.x;
    if (Math.abs(det) <= 1e-9) {
      return null;
    }
    const targetOrigin = targetTriangle[0];
    const tu = {
      x: targetTriangle[1].x - targetOrigin.x,
      y: targetTriangle[1].y - targetOrigin.y,
    };
    const tv = {
      x: targetTriangle[2].x - targetOrigin.x,
      y: targetTriangle[2].y - targetOrigin.y,
    };
    return (/** @type {Point} */ point) => {
      const relative = { x: point.x - sourceOrigin.x, y: point.y - sourceOrigin.y };
      const u = (relative.x * sv.y - relative.y * sv.x) / det;
      const v = (su.x * relative.y - su.y * relative.x) / det;
      return {
        x: targetOrigin.x + tu.x * u + tv.x * v,
        y: targetOrigin.y + tu.y * u + tv.y * v,
      };
    };
  }

  /**
   * @param {Point} sourceStart
   * @param {Point} sourceEnd
   * @param {Point} point
   */
  function segmentPointCoefficients(sourceStart, sourceEnd, point) {
    const dx = sourceEnd.x - sourceStart.x;
    const dy = sourceEnd.y - sourceStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) {
      return null;
    }
    const relativeX = point.x - sourceStart.x;
    const relativeY = point.y - sourceStart.y;
    return {
      alpha: (relativeX * dx + relativeY * dy) / lenSq,
      beta: (relativeX * -dy + relativeY * dx) / lenSq,
    };
  }

  /**
   * @param {Point} segmentStart
   * @param {Point} segmentEnd
   * @param {{ alpha: number; beta: number }} coeffs
   */
  function applySegmentCoefficients(segmentStart, segmentEnd, coeffs) {
    const dx = segmentEnd.x - segmentStart.x;
    const dy = segmentEnd.y - segmentStart.y;
    return {
      x: segmentStart.x + coeffs.alpha * dx - coeffs.beta * dy,
      y: segmentStart.y + coeffs.alpha * dy + coeffs.beta * dx,
    };
  }

  /** @param {number} value */
  function formatSequenceValue(value) {
    if (!Number.isFinite(value)) {
      return "-";
    }
    return Math.abs(value - Math.round(value)) < 0.005
      ? String(Math.round(value))
      : value.toFixed(2);
  }

  /**
   * @param {[number, number, number, number]} color
   * @param {number} amount
   */
  function darken(color, amount) {
    return [
      Math.max(0, color[0] - amount),
      Math.max(0, color[1] - amount),
      Math.max(0, color[2] - amount),
      color[3],
    ];
  }

  /**
   * @param {FunctionExprJson | FunctionAstJson} expr
   * @param {string} parameterName
   * @param {number} currentValue
   * @param {Map<string, number>} parameters
   */
  function evaluateRecursiveExpression(expr, parameterName, currentValue, parameters) {
    const nextParameters = new Map(parameters);
    nextParameters.set(parameterName, currentValue);
    return evaluateExpr(expr, 0, nextParameters);
  }

  /**
   * @param {string} exprLabel
   * @param {string} valueText
   */
  function buildExpressionRichMarkup(exprLabel, valueText) {
    if (typeof exprLabel !== "string") {
      return null;
    }
    const richTextNode = (/** @type {string} */ text) => text
      ? `<Tx${text.split("&").join("＆").split("<").join("＜").split(">").join("＞").split("*").join("\u00b7")}>`
      : "";
    const matchingCloseParen = (/** @type {string} */ text, /** @type {number} */ openIndex) => {
      let depth = 0;
      for (let index = openIndex; index < text.length; index += 1) {
        if (text[index] === "(") {
          depth += 1;
        } else if (text[index] === ")") {
          depth -= 1;
          if (depth === 0) return index;
          if (depth < 0) return -1;
        }
      }
      return -1;
    };
    const stripWrappingParens = (/** @type {string} */ text) => {
      const trimmed = text.trim();
      if (!trimmed.startsWith("(") || !trimmed.endsWith(")")) return trimmed;
      return matchingCloseParen(trimmed, 0) === trimmed.length - 1
        ? trimmed.slice(1, -1)
        : trimmed;
    };
    const renderExpressionPart = (/** @type {string} */ text) => {
      let output = "";
      let rest = text;
      while (true) {
        const index = rest.indexOf("√(");
        if (index < 0) {
          output += richTextNode(rest);
          return output;
        }
        output += richTextNode(rest.slice(0, index));
        const openIndex = index + 1;
        const closeIndex = matchingCloseParen(rest, openIndex);
        if (closeIndex < 0) {
          output += richTextNode(rest.slice(index));
          return output;
        }
        output += `<R${renderExpressionPart(stripWrappingParens(rest.slice(openIndex + 1, closeIndex)))}>`;
        rest = rest.slice(closeIndex + 1);
      }
    };
    const additiveFraction = exprLabel.match(/^(.*)\s\+\s(.*)\s\/\s(.*)$/);
    if (additiveFraction) {
      const [, prefix, numerator, denominator] = additiveFraction;
      return `<H${renderExpressionPart(`${prefix} + `)}</<H${renderExpressionPart(numerator)}><H${renderExpressionPart(denominator)}>><Tx = ${valueText}>>`;
    }
    const parts = exprLabel.split(" / ");
    if (parts.length === 2) {
      return `<H</<H${renderExpressionPart(stripWrappingParens(parts[0]))}><H${renderExpressionPart(parts[1])}>><Tx = ${valueText}>>`;
    }
    return `<H${renderExpressionPart(exprLabel)}<Tx = ${valueText}>>`;
  }

  /**
   * @param {string} name
   * @param {string} valueText
   */
  function buildRatioValueRichMarkup(name, valueText) {
    if (typeof name !== "string") {
      return null;
    }
    const trimmed = name.trim();
    const exprLabel = trimmed.startsWith("(") && trimmed.endsWith(")")
      ? trimmed.slice(1, -1).trim()
      : trimmed;
    const parts = exprLabel.split("/");
    if (parts.length !== 2) {
      return null;
    }
    const numerator = parts[0].trim();
    const denominator = parts[1].trim();
    if (!numerator || !denominator) {
      return null;
    }
    return buildExpressionRichMarkup(`${numerator} / ${denominator}`, valueText);
  }

  /**
   * @param {string} text
   * @returns {string | null}
   */
  function buildPlainTextRichMarkup(text) {
    if (typeof text !== "string" || text.length === 0) {
      return null;
    }
    return `<H<Tx${text
      .split("&").join("＆")
      .split("<").join("＜")
      .split(">").join("＞")
      .split("*").join("\u00b7")}>>`;
  }

  /** @param {string} text */
  function escapeRichText(text) {
    return String(text)
      .split("&").join("＆")
      .split("<").join("＜")
      .split(">").join("＞")
      .split("*").join("\u00b7");
  }

  /**
   * @param {string | null | undefined} markup
   * @param {Map<number, string>} valuesBySlot
   */
  function replaceRichMarkupPathValues(markup, valuesBySlot) {
    if (typeof markup !== "string" || valuesBySlot.size === 0) {
      return markup || null;
    }
    let output = "";
    let index = 0;
    while (index < markup.length) {
      if (!markup.startsWith("<?1x", index)) {
        output += markup[index];
        index += 1;
        continue;
      }
      const nodeStart = index;
      let nameEnd = index + 4;
      while (nameEnd < markup.length && markup[nameEnd] !== "<" && markup[nameEnd] !== ">") {
        nameEnd += 1;
      }
      const slotText = markup.slice(index + 4, nameEnd);
      const slot = /^\d+$/.test(slotText)
        ? Number(slotText)
        : (/^B\d+$/.test(slotText) ? Number(slotText.slice(1)) : NaN);
      const replacement = valuesBySlot.get(slot);
      if (replacement === undefined || markup[nameEnd] !== "<") {
        output += markup.slice(nodeStart, nameEnd);
        index = nameEnd;
        continue;
      }
      let depth = 1;
      let end = nameEnd;
      while (end < markup.length) {
        if (markup[end] === "<") {
          depth += 1;
        } else if (markup[end] === ">") {
          depth -= 1;
          if (depth === 0) {
            end += 1;
            break;
          }
        }
        end += 1;
      }
      if (depth !== 0) {
        output += markup.slice(nodeStart);
        return output;
      }
      output += `<?1x${slotText}<H<T1x${escapeRichText(replacement)}>>>`;
      index = end;
    }
    return output;
  }

  /**
   * @param {string} templateText
   * @param {{ line: number; start: number; end: number; valueText: string }[]} replacements
   */
  function replaceTemplateTextRanges(templateText, replacements) {
    const lines = String(templateText).split("\n").map((line) => Array.from(line));
    [...replacements]
      .sort((left, right) => right.line - left.line || right.start - left.start)
      .forEach((replacement) => {
        const line = lines[replacement.line];
        if (!line) return;
        const start = Math.max(0, Math.min(line.length, replacement.start));
        const end = Math.max(start, Math.min(line.length, replacement.end));
        line.splice(start, end - start, ...Array.from(replacement.valueText));
      });
    return lines.map((line) => line.join("")).join("\n");
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIterationPoints(env, scene, parameters) {
    const families = env.sourceScene.pointIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      if (family.kind === "parameterized") {
        return sum;
      }
      return sum + (family.depth || 0);
    }, 0);
    const standaloneParameterPoints = env.sourceScene.points.filter((/** @type {RuntimeScenePointJson} */ point) =>
      point?.binding?.kind === "parameter" && !point.constraint
    );
    const baseCount = Math.max(
      0,
      env.sourceScene.points.length - exportedDepth - standaloneParameterPoints.length,
    );
    scene.points = scene.points.slice(0, baseCount);

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (depth <= 0) {
        return;
      }
      if (family.kind === "offset") {
        let previousIndex = family.seedIndex;
        for (let step = 0; step < depth; step += 1) {
          const origin = scene.points[previousIndex];
          if (!origin) {
            break;
          }
          scene.points.push({
            x: origin.x + family.dx,
            y: origin.y + family.dy,
            color: origin.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: {
              kind: "offset",
              originIndex: previousIndex,
              dx: family.dx,
              dy: family.dy,
            },
            binding: null,
            debug: null,
          });
          previousIndex = scene.points.length - 1;
        }
        return;
      }

      if (family.kind === "rotate-chain") {
        const center = scene.points[family.centerIndex];
        let previousIndex = family.seedIndex;
        if (!center) {
          return;
        }
        for (let step = 0; step < depth; step += 1) {
          const source = scene.points[previousIndex];
          if (!source) {
            break;
          }
          const rotated = rotateAround(source, center, family.angleDegrees * Math.PI / 180);
          scene.points.push({
            x: rotated.x,
            y: rotated.y,
            color: source.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: null,
            binding: {
              kind: "rotate",
              sourceIndex: previousIndex,
              centerIndex: family.centerIndex,
              angleDegrees: family.angleDegrees,
            },
            debug: null,
          });
          previousIndex = scene.points.length - 1;
        }
        return;
      }

      if (family.kind === "rotate") {
        const source = scene.points[family.sourceIndex];
        const center = scene.points[family.centerIndex];
        if (!source || !center) {
          return;
        }
        const angleDegrees = evaluateExpr(family.angleExpr, 0, parameters);
        if (typeof angleDegrees !== "number" || !Number.isFinite(angleDegrees)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const rotated = rotateAround(source, center, (angleDegrees * step) * Math.PI / 180);
          scene.points.push({
            x: rotated.x,
            y: rotated.y,
            color: source.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: null,
            binding: {
              kind: "rotate",
              sourceIndex: family.sourceIndex,
              centerIndex: family.centerIndex,
              angleDegrees: angleDegrees * step,
            },
            debug: null,
          });
        }
        return;
      }

      if (family.kind === "parameterized") {
        let currentValue = parameters.get(family.traceParameterName);
        if (!isFiniteNumber(currentValue)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const nextValue = evaluateRecursiveExpression(
            family.stepExpr,
            family.traceParameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(nextValue)) {
            break;
          }
          currentValue = nextValue;
          const traceParameters = deriveLabelParameters(
            scene,
            new Map(parameters).set(family.traceParameterName, currentValue),
          );
          const points = resolvePointsWithParameters(env, scene, traceParameters);
          const source = points[family.pointIndex];
          if (!source) {
            continue;
          }
          scene.points.push({
            x: source.x,
            y: source.y,
            color: source.color || [255, 60, 40, 255],
            visible: true,
            draggable: false,
            constraint: null,
            binding: null,
            debug: null,
          });
        }
      }
    });

    standaloneParameterPoints.forEach((/** @type {RuntimeScenePointJson} */ point) => {
      scene.points.push({
        ...point,
        constraint: point.constraint ? { ...point.constraint } : null,
        binding: point.binding ? { ...point.binding } : null,
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   * @returns {RuntimeScenePointJson[]}
   */
  function resolvePointsWithParameters(env, scene, parameters) {
    const draft = {
      ...scene,
      lines: scene.lines,
      circles: scene.circles,
      points: scene.points.map(cloneTracePoint),
    };
    const draftEnv = {
      ...env,
      currentScene: () => draft,
      resolveScenePoint: (/** @type {number} */ index) => draft.points[index],
    };

    const refreshDerivedPoints = () => {
      draft.points.forEach((/** @type {RuntimeScenePointJson} */ point) => {
        const refreshBinding = point.binding ? DERIVED_POINT_BINDING_REFRESHERS[point.binding.kind] : null;
        if (refreshBinding) {
          refreshBinding(draftEnv, draft, point, parameters);
        }
      });
    };

    const resolveConstrainedPoints = () => {
      draft.points.forEach((/** @type {RuntimeScenePointJson} */ point, /** @type {number} */ pointIndex) => {
        if (!point.constraint) {
          return;
        }
        const resolved = modules.scene.resolveConstrainedPoint(
          {
            sourceScene: env.sourceScene,
            currentScene: () => draft,
            resolveScenePoint: (/** @type {number} */ index) => draft.points[index],
          },
          point.constraint,
          (/** @type {number} */ index) => draft.points[index],
          point,
        );
        if (resolved) {
          draft.points[pointIndex].x = resolved.x;
          draft.points[pointIndex].y = resolved.y;
        }
      });
    };

    draft.points.forEach((/** @type {RuntimeScenePointJson} */ point) => {
      if (point.binding?.kind === "parameter" && point.constraint) {
        const value = parameters.get(point.binding.name);
        if (isFiniteNumber(value)) {
          applyNormalizedParameterToPoint(point, draft, value);
        }
        return;
      }
      const updatePoint = point.binding ? SYNC_DYNAMIC_POINT_BINDING_UPDATERS[point.binding.kind] : null;
      if (updatePoint) {
        updatePoint(draftEnv, draft, point, parameters);
      }
    });
    for (let pass = 0; pass < 3; pass += 1) {
      refreshDerivedPoints();
      resolveConstrainedPoints();
    }
    refreshDerivedPoints();
    return draft.points;
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIteratedLines(env, scene, parameters) {
    const families = env.sourceScene.lineIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      const depth = family.depth || 0;
      if (family.kind === "parameterized-point-trace") {
        return sum;
      }
      if (family.kind === "rotate") {
        return sum;
      }
      if (family.kind === "branching") {
        const branchCount = Array.isArray(family.targetSegments) ? family.targetSegments.length : 0;
        let total = 0;
        let width = branchCount;
        for (let step = 0; step < depth; step += 1) {
          total += width;
          width *= branchCount;
        }
        return sum + total;
      }
      if (family.kind === "affine") {
        return sum + depth;
      }
      if (family.bidirectional) {
        if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
          return sum + (2 * depth * (depth + 1));
        }
        return sum + (2 * depth);
      }
      if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
        return sum + (((depth + 1) * (depth + 2)) / 2 - 1);
      }
      return sum + depth;
    }, 0);
    const baseCount = Math.max(0, env.sourceScene.lines.length - exportedDepth);
    scene.lines = scene.lines.slice(0, baseCount);
    /** @type {Set<string>} */
    const resetControlledTickColors = new Set();
    /** @type {Set<string>} */
    const emittedControlledTickSeeds = new Set();

    families.forEach((family) => {
      const depth = pointIterationDepth(family, parameters);
      if (depth <= 0) {
        return;
      }
      const resolveHandle = (/** @type {PointHandle} */ handle) => {
        if (hasPointIndexHandle(handle)) {
          return env.resolveScenePoint(handle.pointIndex);
        }
        if (hasLineIndexHandle(handle)) {
          const line = scene.lines[handle.lineIndex];
          if (!line?.points || line.points.length < 2) return null;
          const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
          const t = typeof handle.t === "number" ? handle.t : 0.5;
          const p0 = line.points[segmentIndex];
          const p1 = line.points[segmentIndex + 1];
          return {
            x: p0.x + (p1.x - p0.x) * t,
            y: p0.y + (p1.y - p0.y) * t,
          };
        }
        return /** @type {Point} */ (handle);
      };
      if (family.kind === "parameterized-point-trace") {
        const depthParameterName = family.depthParameterName;
        const depthParameterValue = typeof depthParameterName === "string"
          ? parameters.get(depthParameterName)
          : undefined;
        const depth = Math.max(
          0,
          Math.round(
            isFiniteNumber(depthParameterValue)
              ? depthParameterValue
              : family.depth || 0,
          ),
        );
        let currentValue = parameters.get(family.traceParameterName);
        if (!isFiniteNumber(currentValue)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const nextValue = evaluateRecursiveExpression(
            family.stepExpr,
            family.traceParameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(nextValue)) {
            break;
          }
          currentValue = nextValue;
          const traceParameters = deriveLabelParameters(
            scene,
            new Map(parameters).set(family.traceParameterName, currentValue),
          );
          const line = {
            /** @type {PointHandle[]} */
            points: [],
            color: family.color,
            dashed: !!family.dashed,
            binding: {
              kind: "point-trace",
              pointIndex: family.pointIndex,
              driverIndex: family.driverIndex,
              xMin: family.xMin,
              xMax: family.xMax,
              sampleCount: family.sampleCount,
              useMidpoints: true,
            },
          };
          const sampled = samplePointTraceLine(scene, line, traceParameters);
          if (!sampled) {
            continue;
          }
          scene.lines.push({
            points: sampled,
            color: family.color,
            dashed: !!family.dashed,
            binding: null,
          });
        }
        return;
      }
      if (family.kind === "branching") {
        const start = env.resolveScenePoint(family.startIndex);
        const end = env.resolveScenePoint(family.endIndex);
        if (!start || !end) {
          return;
        }
        const targetSegments = (family.targetSegments || []).map((segment) => [
          resolveHandle(segment[0]),
          resolveHandle(segment[1]),
        ]);
        if (targetSegments.some((segment) => segment.some((point) => !point))) {
          return;
        }
        const coeffs = targetSegments
          .flatMap((segment) => {
            const [targetStart, targetEnd] = segment;
            if (!targetStart || !targetEnd) {
              return [];
            }
            const startCoeffs = segmentPointCoefficients(start, end, targetStart);
            const endCoeffs = segmentPointCoefficients(start, end, targetEnd);
            if (!startCoeffs || !endCoeffs) {
              return [];
            }
            return [{ startCoeffs, endCoeffs }];
          });
        if (coeffs.length === 0) {
          return;
        }
        /** @type {{ start: Point; end: Point }[]} */
        let frontier = [{ start: { ...start }, end: { ...end } }];
        for (let step = 0; step < depth; step += 1) {
          /** @type {{ start: Point; end: Point }[]} */
          const next = [];
          frontier.forEach((segment) => {
            coeffs.forEach((coeff) => {
              const childStart = applySegmentCoefficients(segment.start, segment.end, coeff.startCoeffs);
              const childEnd = applySegmentCoefficients(segment.start, segment.end, coeff.endCoeffs);
              scene.lines.push({
                points: [{ ...childStart }, { ...childEnd }],
                color: family.color,
                dashed: !!family.dashed,
                binding: null,
              });
              next.push({ start: childStart, end: childEnd });
            });
          });
          frontier = next;
        }
        return;
      }
      if (family.kind === "affine") {
        const start = env.resolveScenePoint(family.startIndex);
        const end = env.resolveScenePoint(family.endIndex);
        if (!start || !end) {
          return;
        }
        const sourceTriangle = family.sourceTriangleIndices.map((index) => env.resolveScenePoint(index));
        const targetTriangle = family.targetTriangle.map((handle) => resolveHandle(handle));
        if (sourceTriangle.some((point) => !point) || targetTriangle.some((point) => !point)) {
          return;
        }
        const mapPoint = affineMapFromTriangles(
          /** @type {Point[]} */ (sourceTriangle),
          /** @type {Point[]} */ (targetTriangle),
        );
        if (!mapPoint) {
          return;
        }
        let currentStart = { ...start };
        let currentEnd = { ...end };
        for (let step = 0; step < depth; step += 1) {
          currentStart = mapPoint(currentStart);
          currentEnd = mapPoint(currentEnd);
          scene.lines.push({
            points: [{ ...currentStart }, { ...currentEnd }],
            color: family.color,
            dashed: !!family.dashed,
            binding: null,
          });
        }
        return;
      }
      if (family.kind === "rotate") {
        const source = scene.lines[family.sourceIndex];
        const center = scene.points[family.centerIndex];
        if (!source || !center) {
          return;
        }
        const angleDegrees = evaluateExpr(family.angleExpr, 0, parameters);
        if (typeof angleDegrees !== "number" || !Number.isFinite(angleDegrees)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const radians = (angleDegrees * step) * Math.PI / 180;
          scene.lines.push({
            points: source.points.map((/** @type {Point} */ point) => rotateAround(point, center, radians)),
            color: family.color,
            dashed: !!family.dashed,
            binding: {
              kind: "derived",
              sourceIndex: family.sourceIndex,
              transform: {
                kind: "rotate",
                centerIndex: family.centerIndex,
                angleDegrees: angleDegrees * step,
                parameterName: null,
              },
            },
          });
        }
        return;
      }
      if (family.kind !== "translate") return;
      const start = env.resolveScenePoint(family.startIndex);
      const end = env.resolveScenePoint(family.endIndex);
      if (!start || !end) {
        return;
      }
      let primaryDx = family.dx;
      let primaryDy = family.dy;
      if (typeof family.vectorStartIndex === "number" && typeof family.vectorEndIndex === "number") {
        const vectorStart = env.resolveScenePoint(family.vectorStartIndex);
        const vectorEnd = env.resolveScenePoint(family.vectorEndIndex);
        if (vectorStart && vectorEnd) {
          primaryDx = vectorEnd.x - vectorStart.x;
          primaryDy = vectorEnd.y - vectorStart.y;
        }
      }
      /**
       * @param {Point} point
       * @param {number | null | undefined} controlIndex
       */
      const controlledEndpoint = (point, controlIndex) => {
        if (typeof controlIndex !== "number" || !Number.isFinite(controlIndex)) return point;
        const control = env.resolveScenePoint(controlIndex);
        if (!control) return point;
        return { x: point.x, y: control.y };
      };
      const liveStart = controlledEndpoint(start, family.startControlIndex);
      const liveEnd = controlledEndpoint(end, family.endControlIndex);
      if (Number.isFinite(family.startControlIndex) || Number.isFinite(family.endControlIndex)) {
        const colorKey = JSON.stringify(family.color || null);
        if (!resetControlledTickColors.has(colorKey)) {
          scene.lines = scene.lines.filter((line) => {
            if (line.binding || !Array.isArray(line.points) || line.points.length !== 2) return true;
            const lineStart = resolveHandle(line.points[0]);
            const lineEnd = resolveHandle(line.points[1]);
            if (!lineStart || !lineEnd) return true;
            const sameColor = JSON.stringify(line.color || null) === colorKey;
            const vertical = Math.abs(lineStart.x - lineEnd.x) < 1e-6;
            return !(sameColor && vertical);
          });
          resetControlledTickColors.add(colorKey);
        }
        const seedKey = `${colorKey}:${family.startIndex}:${family.endIndex}`;
        if (!emittedControlledTickSeeds.has(seedKey)) {
          scene.lines.push({
            points: [
              { x: liveStart.x, y: liveStart.y },
              { x: liveEnd.x, y: liveEnd.y },
            ],
            color: family.color,
            dashed: !!family.dashed,
            binding: null,
          });
          emittedControlledTickSeeds.add(seedKey);
        }
      }
      const secondaryDx = isFiniteNumber(family.secondaryDx) ? family.secondaryDx : null;
      const secondaryDy = isFiniteNumber(family.secondaryDy) ? family.secondaryDy : null;
      const hasSecondary = secondaryDx !== null && secondaryDy !== null;
      const deltas = [];
      if (family.bidirectional && hasSecondary) {
        for (let primary = -depth; primary <= depth; primary += 1) {
          for (let secondary = -depth; secondary <= depth; secondary += 1) {
            if (primary === 0 && secondary === 0) {
              continue;
            }
            if (Math.abs(primary) + Math.abs(secondary) > depth) {
              continue;
            }
            deltas.push({
              dx: primaryDx * primary + secondaryDx * secondary,
              dy: primaryDy * primary + secondaryDy * secondary,
            });
          }
        }
      } else if (family.bidirectional) {
        for (let step = 1; step <= depth; step += 1) {
          deltas.push(
            { dx: primaryDx * step, dy: primaryDy * step },
            { dx: -primaryDx * step, dy: -primaryDy * step },
          );
        }
      } else if (hasSecondary) {
        for (let primary = 0; primary <= depth; primary += 1) {
          for (let secondary = 0; secondary <= depth - primary; secondary += 1) {
            if (primary === 0 && secondary === 0) {
              continue;
            }
            deltas.push({
              dx: primaryDx * primary + secondaryDx * secondary,
              dy: primaryDy * primary + secondaryDy * secondary,
            });
          }
        }
      } else {
        for (let step = 1; step <= depth; step += 1) {
          deltas.push({
            dx: primaryDx * step,
            dy: primaryDy * step,
          });
        }
      }
      deltas.forEach(({ dx, dy }) => {
        scene.lines.push({
          points: [
            { x: liveStart.x + dx, y: liveStart.y + dy },
            { x: liveEnd.x + dx, y: liveEnd.y + dy },
          ],
          color: family.color,
          dashed: !!family.dashed,
          binding: null,
        });
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIteratedPolygons(env, scene, parameters) {
    const families = env.sourceScene.polygonIterations || [];
    if (families.length === 0) {
      return;
    }
    const exportedDepth = families.reduce((sum, family) => {
      if (family.kind === "coordinate-grid") {
        return sum + Math.max(0, Math.round(family.depth || 0));
      }
      const depth = family.depth || 0;
      if (family.bidirectional) {
        if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
          return sum + (1 + 2 * depth * (depth + 1));
        }
        return sum + (1 + 2 * depth);
      }
      if (Number.isFinite(family.secondaryDx) && Number.isFinite(family.secondaryDy)) {
        return sum + (((depth + 1) * (depth + 2)) / 2);
      }
      return sum + (depth + 1);
    }, 0);
    const baseCount = Math.max(0, env.sourceScene.polygons.length - exportedDepth);
    scene.polygons = scene.polygons.slice(0, baseCount);

    families.forEach((family) => {
      if (family.vertexIndices.length < 3) {
        return;
      }
      const seedVertices = family.vertexIndices
        .map((index) => env.resolveScenePoint(index));
      if (seedVertices.some((point) => !point)) {
        return;
      }
      const seedPoints = /** @type {Point[]} */ (seedVertices);
      if (family.kind === "coordinate-grid") {
        const depthValue = family.depthExpr
          ? evaluateExpr(family.depthExpr, 0, parameters)
          : family.depth;
        const depth = Math.max(0, Math.floor(isFiniteNumber(depthValue) ? depthValue : family.depth || 0));
        let currentValue = parameters.get(family.parameterName);
        if (!isFiniteNumber(currentValue)) {
          return;
        }
        for (let step = 1; step <= depth; step += 1) {
          const nextValue = evaluateRecursiveExpression(
            family.stepExpr,
            family.parameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(nextValue)) {
            break;
          }
          currentValue = nextValue;
          const exprParameters = deriveLabelParameters(
            scene,
            new Map(parameters).set(family.parameterName, currentValue),
          );
          const dx = evaluateExpr(family.xExpr, 0, exprParameters);
          const dy = evaluateExpr(family.yExpr, 0, exprParameters);
          if (!isFiniteNumber(dx) || !isFiniteNumber(dy)) {
            continue;
          }
          scene.polygons.push({
            points: seedPoints.map((point) => ({
              x: point.x + dx * family.xRawScale,
              y: point.y - dy * family.yRawScale,
            })),
            color: family.color,
            outlineColor: darken(family.color, 80),
            binding: null,
          });
        }
        return;
      }
      const depth = pointIterationDepth(family, parameters);
      if (family.kind !== "translate") {
        return;
      }
      const secondaryDx = isFiniteNumber(family.secondaryDx) ? family.secondaryDx : null;
      const secondaryDy = isFiniteNumber(family.secondaryDy) ? family.secondaryDy : null;
      const hasSecondary = secondaryDx !== null && secondaryDy !== null;
      const deltas = [];
      if (family.bidirectional && hasSecondary) {
        for (let primary = -depth; primary <= depth; primary += 1) {
          for (let secondary = -depth; secondary <= depth; secondary += 1) {
            if (Math.abs(primary) + Math.abs(secondary) > depth) {
              continue;
            }
            deltas.push({
              dx: family.dx * primary + secondaryDx * secondary,
              dy: family.dy * primary + secondaryDy * secondary,
            });
          }
        }
      } else if (family.bidirectional) {
        deltas.push({ dx: 0, dy: 0 });
        for (let step = 1; step <= depth; step += 1) {
          deltas.push(
            { dx: family.dx * step, dy: family.dy * step },
            { dx: -family.dx * step, dy: -family.dy * step },
          );
        }
      } else if (hasSecondary) {
        for (let primary = 0; primary <= depth; primary += 1) {
          for (let secondary = 0; secondary <= depth - primary; secondary += 1) {
            deltas.push({
              dx: family.dx * primary + secondaryDx * secondary,
              dy: family.dy * primary + secondaryDy * secondary,
            });
          }
        }
      } else {
        for (let step = 0; step <= depth; step += 1) {
          deltas.push({
            dx: family.dx * step,
            dy: family.dy * step,
          });
        }
      }
      deltas.forEach(({ dx, dy }) => {
        scene.polygons.push({
          points: seedPoints.map((point) => ({ x: point.x + dx, y: point.y + dy })),
          color: family.color,
          outlineColor: darken(family.color, 80),
          binding: null,
        });
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIteratedLabels(env, scene, parameters) {
    const families = env.sourceScene.labelIterations || [];
    if (families.length === 0) {
      return;
    }
    const baseCount = env.sourceScene.labels.length;
    scene.labels = scene.labels.slice(0, baseCount);

    families.forEach((family) => {
      if (family.kind !== "point-expression") {
        if (family.kind !== "translate-expression") {
          return;
        }
        const seedLabel = scene.labels[family.seedLabelIndex];
        const vectorStart = scene.points[family.vectorStartIndex];
        const vectorEnd = scene.points[family.vectorEndIndex];
        if (!seedLabel || !vectorStart || !vectorEnd) {
          return;
        }
        if (isFiniteNumber(family.firstOutputLabelIndex) && isFiniteNumber(family.outputLabelCount)) {
          for (let index = 0; index < family.outputLabelCount; index += 1) {
            const label = scene.labels[family.firstOutputLabelIndex + index];
            if (label) {
              label.visible = false;
            }
          }
        }
        const depth = pointIterationDepth({
          depth: family.depth,
          depthExpr: family.depthExpr,
          depthParameterName: family.depthParameterName,
        }, parameters);
        const seedAnchor = seedLabel.anchor;
        let currentValue = parameters.get(family.parameterName);
        if (!seedAnchor || !isFiniteNumber(currentValue)) {
          return;
        }
        const seedAnchorPoint = env.resolvePoint(seedAnchor);
        if (!seedAnchorPoint) {
          return;
        }
        const dx = vectorEnd.x - vectorStart.x;
        const dy = vectorEnd.y - vectorStart.y;
        const seedValue = evaluateRecursiveExpression(
          family.expr,
          family.parameterName,
          currentValue,
          parameters,
        );
        if (!isFiniteNumber(seedValue)) {
          return;
        }
        currentValue = seedValue;
        for (let step = 1; step <= depth; step += 1) {
          const value = evaluateRecursiveExpression(
            family.expr,
            family.parameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(value)) {
            break;
          }
          currentValue = value;
          const text = formatSequenceValue(value);
          scene.labels.push({
            ...seedLabel,
            text,
            richMarkup: buildPlainTextRichMarkup(text),
            binding: null,
            anchor: { x: seedAnchorPoint.x + dx * step, y: seedAnchorPoint.y + dy * step },
          });
        }
        return;
      }
      const seedLabel = scene.labels[family.seedLabelIndex];
      const seedAnchor = seedLabel?.anchor;
      const seedPointIndex = typeof seedAnchor?.pointIndex === "number"
        ? seedAnchor.pointIndex
        : (seedLabel?.binding?.kind === "point-expression-value"
          && typeof seedLabel.binding.pointIndex === "number"
            ? seedLabel.binding.pointIndex
            : null);
      if (!seedLabel || seedPointIndex === null) {
        return;
      }
      const depth = pointIterationDepth({
        depth: family.depth,
        parameterName: family.depthParameterName,
      }, parameters);
      let currentValue = parameters.get(family.parameterName);
      if (!isFiniteNumber(currentValue)) {
        return;
      }
      for (let step = 0; step <= depth; step += 1) {
        const value = evaluateRecursiveExpression(
          family.expr,
          family.parameterName,
          currentValue,
          parameters,
        );
        if (!isFiniteNumber(value)) {
          break;
        }
        const pointIndex = family.pointSeedIndex + step;
        if (!scene.points[pointIndex]) {
          break;
        }
        if (step === 0) {
          seedLabel.text = formatSequenceValue(value);
          seedLabel.richMarkup = buildPlainTextRichMarkup(seedLabel.text);
          seedLabel.anchor = { ...seedAnchor, pointIndex: seedPointIndex };
        } else {
          scene.labels.push({
            ...seedLabel,
            text: formatSequenceValue(value),
            richMarkup: buildPlainTextRichMarkup(formatSequenceValue(value)),
            binding: null,
            anchor: { ...seedAnchor, pointIndex },
          });
        }
        currentValue = value;
      }
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function rebuildIterationTables(env, scene, parameters) {
    const sourceTables = env.sourceScene.iterationTables || [];
    const currentTables = scene.iterationTables || [];
    scene.iterationTables = sourceTables.map((table, index) => {
      const current = currentTables[index];
      const depth = table.depthParameterName
        ? discreteIterationDepth(parameters.get(table.depthParameterName) ?? table.depth)
        : discreteIterationDepth(table.depth);
      let currentValue = parameters.get(table.parameterName);
      /** @type {RuntimeIterationRow[]} */
      const rows = [];
      if (isFiniteNumber(currentValue)) {
        for (let index = 0; index <= depth; index += 1) {
          const value = evaluateRecursiveExpression(
            table.expr,
            table.parameterName,
            currentValue,
            parameters,
          );
          if (!isFiniteNumber(value)) {
            break;
          }
          rows.push({ index, value });
          currentValue = value;
        }
      }
      return {
        ...table,
        x: Number.isFinite(current?.x) ? current.x : table.x,
        y: Number.isFinite(current?.y) ? current.y : table.y,
        rows,
      };
    });
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {Point | null} source
   * @param {Map<string, number>} parameters
   */
  function updateCoordinateSourcePoint(point, source, parameters) {
    if (!source) return;
    const parameterValue = parameters.get(point.binding.name);
    if (!isFiniteNumber(parameterValue)) return;
    const exprParameters = new Map(parameters);
    exprParameters.set(point.binding.name, parameterValue);
    const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
    if (offset === null) return;
    if (point.binding.axis === "horizontal") {
      point.x = source.x + offset;
      point.y = source.y;
      return;
    }
    point.x = source.x;
    point.y = source.y + offset;
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {Point | null} source
   * @param {Map<string, number>} parameters
   */
  function updateCoordinateSource2dPoint(point, source, parameters) {
    if (!source) return;
    const xParameterValue = parameters.get(point.binding.xName);
    const yParameterValue = parameters.get(point.binding.yName);
    if (!isFiniteNumber(xParameterValue) || !isFiniteNumber(yParameterValue)) return;
    const exprParameters = new Map(parameters);
    exprParameters.set(point.binding.xName, xParameterValue);
    exprParameters.set(point.binding.yName, yParameterValue);
    const dx = evaluateExpr(point.binding.xExpr, 0, exprParameters);
    const dy = evaluateExpr(point.binding.yExpr, 0, exprParameters);
    if (dx !== null && dy !== null) {
      point.x = source.x + dx;
      point.y = source.y + dy;
    }
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {ViewerSceneData} scene
   * @param {number} value
   */
  function updateConstraintParameterizedPoint(point, scene, value) {
    if (!Number.isFinite(value)) return;
    applyNormalizedParameterToPoint(point, scene, value);
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {Map<string, number>} parameters
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   * @param {ViewerSceneData} parameterSourceScene
   */
  function updateCustomTransformPoint(point, parameters, resolvePointAt, parameterSourceScene) {
    const value = parameterValueFromPoint(parameterSourceScene, point.binding.sourceIndex);
    if (!isFiniteNumber(value)) return;
    const exprParameters = new Map(parameters);
    const names = new Set();
    collectExprParameterNames(point.binding.distanceExpr, names);
    collectExprParameterNames(point.binding.angleExpr, names);
    names.forEach((name) => exprParameters.set(name, value));
    const distanceValue = evaluateExpr(point.binding.distanceExpr, value, exprParameters);
    const angleValue = evaluateExpr(point.binding.angleExpr, value, exprParameters);
    const origin = resolvePointAt(point.binding.originIndex);
    const axisEnd = resolvePointAt(point.binding.axisEndIndex);
    if (distanceValue === null || angleValue === null || !origin || !axisEnd) return;
    const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
    const radians = (baseAngle + angleValue * point.binding.angleDegreesScale) * Math.PI / 180;
    const distance = distanceValue * point.binding.distanceRawScale;
    point.x = origin.x + distance * Math.cos(radians);
    point.y = origin.y - distance * Math.sin(radians);
  }

  /**
   * @param {RuntimeScenePointJson} point
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   */
  function updateScaleByRatioPoint(point, resolvePointAt) {
    const source = resolvePointAt(point.binding.sourceIndex);
    const center = resolvePointAt(point.binding.centerIndex);
    const ratioOrigin = resolvePointAt(point.binding.ratioOriginIndex);
    const ratioDenominator = resolvePointAt(point.binding.ratioDenominatorIndex);
    const ratioNumerator = resolvePointAt(point.binding.ratioNumeratorIndex);
    if (!source || !center || !ratioOrigin || !ratioDenominator || !ratioNumerator) return;
    const scaled = scaleByThreePointRatio(
      source,
      center,
      ratioOrigin,
      ratioDenominator,
      ratioNumerator,
      point.binding.signed !== false,
      point.binding.clampToUnit === true,
    );
    if (!scaled) return;
    point.x = scaled.x;
    point.y = scaled.y;
  }

  /**
   * @param {Point} start
   * @param {Point} mid
   * @param {Point} end
   * @returns {Point | null}
   */
  function circumcenter(start, mid, end) {
    const determinant = 2 * (
      start.x * (mid.y - end.y)
      + mid.x * (end.y - start.y)
      + end.x * (start.y - mid.y)
    );
    if (Math.abs(determinant) <= 1e-9) return null;
    const startSq = start.x * start.x + start.y * start.y;
    const midSq = mid.x * mid.x + mid.y * mid.y;
    const endSq = end.x * end.x + end.y * end.y;
    return {
      x: (
        startSq * (mid.y - end.y)
        + midSq * (end.y - start.y)
        + endSq * (start.y - mid.y)
      ) / determinant,
      y: (
        startSq * (end.x - mid.x)
        + midSq * (start.x - end.x)
        + endSq * (mid.x - start.x)
      ) / determinant,
    };
  }

  /**
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   * @param {ViewBounds} bounds
   * @param {LineConstraintJson} constraint
   * @returns {Point[] | null}
   */
  function resolveLineConstraintPoints(resolvePointAt, bounds, constraint) {
    if (!constraint) return null;
    if (constraint.kind === "segment") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? [start, end] : null;
    }
    if (constraint.kind === "line") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? clipParametricLineToBounds(start, end, bounds, false) : null;
    }
    if (constraint.kind === "ray") {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? clipParametricLineToBounds(start, end, bounds, true) : null;
    }
    if (constraint.kind === "perpendicular-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x - dy / len, y: through.y + dx / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "parallel-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return clipParametricLineToBounds(
        through,
        { x: through.x + dx / len, y: through.y + dy / len },
        bounds,
        false,
      );
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolvePointAt(constraint.startIndex);
      const vertex = resolvePointAt(constraint.vertexIndex);
      const end = resolvePointAt(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      if (!direction) return null;
      return clipParametricLineToBounds(
        vertex,
        { x: vertex.x + direction.x, y: vertex.y + direction.y },
        bounds,
        true,
      );
    }
    if (constraint.kind === "translated") {
      /** @type {Point[] | null} */
      const source = resolveLineConstraintPoints(resolvePointAt, bounds, constraint.line);
      const vectorStart = resolvePointAt(constraint.vectorStartIndex);
      const vectorEnd = resolvePointAt(constraint.vectorEndIndex);
      if (!source || !vectorStart || !vectorEnd) return null;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      return source.map((/** @type {Point} */ point) => ({ x: point.x + dx, y: point.y + dy }));
    }
    return null;
  }

  /**
   * @param {(pointIndex: number) => Point | null} resolvePointAt
   * @param {LineConstraintJson} constraint
   * @returns {Point[] | null}
   */
  function resolveLineConstraintParameterPoints(resolvePointAt, constraint) {
    if (!constraint) return null;
    if (
      constraint.kind === "segment"
      || constraint.kind === "line"
      || constraint.kind === "ray"
    ) {
      const start = resolvePointAt(constraint.startIndex);
      const end = resolvePointAt(constraint.endIndex);
      return start && end ? [start, end] : null;
    }
    if (constraint.kind === "perpendicular-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return [
        through,
        { x: through.x - dy, y: through.y + dx },
      ];
    }
    if (constraint.kind === "parallel-line") {
      const through = resolvePointAt(constraint.throughIndex);
      const lineStart = resolvePointAt(constraint.lineStartIndex);
      const lineEnd = resolvePointAt(constraint.lineEndIndex);
      if (!through || !lineStart || !lineEnd) return null;
      const dx = lineEnd.x - lineStart.x;
      const dy = lineEnd.y - lineStart.y;
      const len = Math.hypot(dx, dy);
      if (len <= 1e-9) return null;
      return [
        through,
        { x: through.x + dx, y: through.y + dy },
      ];
    }
    if (constraint.kind === "angle-bisector-ray") {
      const start = resolvePointAt(constraint.startIndex);
      const vertex = resolvePointAt(constraint.vertexIndex);
      const end = resolvePointAt(constraint.endIndex);
      if (!start || !vertex || !end) return null;
      const direction = angleBisectorDirection(start, vertex, end);
      return direction
        ? [vertex, { x: vertex.x + direction.x, y: vertex.y + direction.y }]
        : null;
    }
    if (constraint.kind === "translated") {
      const source = resolveLineConstraintParameterPoints(resolvePointAt, constraint.line);
      const vectorStart = resolvePointAt(constraint.vectorStartIndex);
      const vectorEnd = resolvePointAt(constraint.vectorEndIndex);
      if (!source || !vectorStart || !vectorEnd) return null;
      const dx = vectorEnd.x - vectorStart.x;
      const dy = vectorEnd.y - vectorStart.y;
      return source.map((/** @type {Point} */ point) => ({ x: point.x + dx, y: point.y + dy }));
    }
    return null;
  }

  /** @type {Record<string, PointBindingRefresher>} */
  const DERIVED_POINT_BINDING_REFRESHERS = {
    "derived-parameter"(_env, scene, point) {
      const value = parameterValueFromPoint(scene, point.binding.sourceIndex);
      if (value !== null) {
        applyNormalizedParameterToPoint(point, scene, value);
      }
    },
    "constraint-parameter-expr"(_env, scene, point, parameters) {
      const value = evaluateExpr(point.binding.expr, 0, parameters);
      if (isFiniteNumber(value)) {
        updateConstraintParameterizedPoint(point, scene, value);
      }
    },
    "constraint-parameter-from-point-expr"(_env, scene, point, parameters) {
      const sourceValue = parameterValueFromPoint(scene, point.binding.sourceIndex);
      if (!isFiniteNumber(sourceValue)) return;
      const exprParameters = new Map(parameters);
      if (point.binding.parameterName) {
        exprParameters.set(point.binding.parameterName, sourceValue);
      }
      const value = evaluateExpr(point.binding.expr, 0, exprParameters);
      if (value !== null) {
        updateConstraintParameterizedPoint(
          point,
          scene,
          point.binding.absoluteValue === true ? value : sourceValue + value,
        );
      }
    },
    "coordinate-source"(env, _scene, point, parameters) {
      updateCoordinateSourcePoint(point, env.resolveScenePoint(point.binding.sourceIndex), parameters);
    },
    "coordinate-source-2d"(env, _scene, point, parameters) {
      updateCoordinateSource2dPoint(point, env.resolveScenePoint(point.binding.sourceIndex), parameters);
    },
    derived(env, scene, point, parameters) {
      const source = resolveScenePointInScene(env, scene, point.binding.sourceIndex);
      if (!source) return;
      const transform = point.binding.transform;
      if (transform.kind === "translate") {
        const vectorStart = resolveScenePointInScene(env, scene, transform.vectorStartIndex);
        const vectorEnd = resolveScenePointInScene(env, scene, transform.vectorEndIndex);
        if (!vectorStart || !vectorEnd) return;
        point.x = source.x + (vectorEnd.x - vectorStart.x);
        point.y = source.y + (vectorEnd.y - vectorStart.y);
        return;
      }
      if (transform.kind === "reflect") {
        const lineStart = resolveScenePointInScene(env, scene, transform.lineStartIndex);
        const lineEnd = resolveScenePointInScene(env, scene, transform.lineEndIndex);
        if (!lineStart || !lineEnd) return;
        const reflected = reflectAcrossLine(source, lineStart, lineEnd);
        if (!reflected) return;
        point.x = reflected.x;
        point.y = reflected.y;
        return;
      }
      if (transform.kind === "reflect-constraint") {
        const line = resolveLineConstraintPoints(
          (/** @type {number} */ index) => resolveScenePointInScene(env, scene, index),
          env.getViewBounds ? env.getViewBounds() : env.sourceScene.bounds,
          transform.line,
        );
        if (!line) return;
        const reflected = reflectAcrossLine(source, line[0], line[1]);
        if (!reflected) return;
        point.x = reflected.x;
        point.y = reflected.y;
        return;
      }
      if (transform.kind === "rotate") {
        const center = resolveScenePointInScene(env, scene, transform.centerIndex);
        if (!center) return;
        const angleDegrees = resolveRotateTransformAngleDegrees(
          transform,
          parameters,
          (index) => resolveScenePointInScene(env, scene, index),
        );
        if (!isFiniteNumber(angleDegrees)) return;
        const rotated = rotateAround(source, center, angleDegrees * Math.PI / 180);
        point.x = rotated.x;
        point.y = rotated.y;
        return;
      }
      if (transform.kind === "scale") {
        const center = resolveScenePointInScene(env, scene, transform.centerIndex);
        if (!center) return;
        const factor = resolveScaleTransformFactor(
          transform,
          parameters,
          (index) => resolveScenePointInScene(env, scene, index),
        );
        if (!isFiniteNumber(factor)) return;
        const scaled = scaleAround(source, center, factor);
        point.x = scaled.x;
        point.y = scaled.y;
      }
    },
    "scale-by-ratio"(env, _scene, point) {
      updateScaleByRatioPoint(point, (/** @type {number} */ index) => env.resolveScenePoint(index));
    },
    circumcenter(env, scene, point) {
      const start = resolveScenePointInScene(env, scene, point.binding.startIndex);
      const mid = resolveScenePointInScene(env, scene, point.binding.midIndex);
      const end = resolveScenePointInScene(env, scene, point.binding.endIndex);
      if (!start || !mid || !end) return;
      const center = circumcenter(start, mid, end);
      if (!center) return;
      point.x = center.x;
      point.y = center.y;
    },
    "custom-transform"(_env, scene, point, parameters) {
      updateCustomTransformPoint(point, parameters, (/** @type {number} */ index) => scene.points[index], scene);
    },
  };

  /** @type {Record<string, DynamicLabelRefresher>} */
  const DYNAMIC_LABEL_REFRESHERS = {
    "point-anchor"(_env, scene, label) {
      const point = scene.points[label.binding.pointIndex];
      if (!point) return;
      const anchor = {
        x: point.x + (label.binding.anchorDx || 0),
        y: point.y + (label.binding.anchorDy || 0),
      };
      if (Number.isFinite(label.binding.anchorYPointIndex)) {
        const yPoint = scene.points[label.binding.anchorYPointIndex];
        if (yPoint) {
          anchor.y = yPoint.y + (label.binding.anchorYDy || 0);
        }
      }
      label.anchor = anchor;
    },
    "parameter-value"(env, _scene, label, parameters) {
      const value = parameters.get(label.binding.name);
      if (value !== null && value !== undefined) {
        label.text = `${label.binding.name} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "point-expression-value"(_env, scene, label, parameters) {
      const currentValue = parameters.get(label.binding.parameterName);
      if (!isFiniteNumber(currentValue)) return;
      DYNAMIC_LABEL_REFRESHERS["point-anchor"](_env, scene, label, parameters);
      const value = evaluateRecursiveExpression(
        label.binding.expr,
        label.binding.parameterName,
        currentValue,
        parameters,
      );
      if (value !== null) {
        label.text = formatSequenceValue(value);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "sequence-expression-value"(_env, _scene, label, parameters) {
      const stateValue = parameters.get(label.binding.parameterName);
      if (isFiniteNumber(stateValue)) {
        label.text = formatSequenceValue(stateValue);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
        return;
      }
      let currentValue = parameters.get(label.binding.parameterName);
      if (!isFiniteNumber(currentValue)) return;
      const depth = pointIterationDepth({
        depth: label.binding.depth,
        parameterName: label.binding.depthParameterName,
      }, parameters);
      /** @type {number | null} */
      let value = null;
      for (let step = 0; step <= depth; step += 1) {
        const nextValue = evaluateRecursiveExpression(
          label.binding.expr,
          label.binding.parameterName,
          currentValue,
          parameters,
        );
        if (!isFiniteNumber(nextValue)) return;
        value = nextValue;
        currentValue = value;
      }
      if (value !== null) {
        label.text = formatSequenceValue(value);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "rich-text-expression-values"(_env, _scene, label, parameters) {
      /** @type {Map<number, string>} */
      const valuesBySlot = new Map();
      /** @type {{ line: number, start: number, end: number, valueText: string }[]} */
      const replacements = [];
      (label.binding.refs || []).forEach((/** @type {any} */ ref) => {
        const value = evaluateExpr(ref.expr, 0, parameters);
        const valueText = value !== null ? formatSequenceValue(value) : "未定义";
        valuesBySlot.set(ref.slot, valueText);
        replacements.push({
          line: ref.line,
          start: ref.start,
          end: ref.end,
          valueText,
        });
      });
      label.text = replaceTemplateTextRanges(label.binding.templateText || label.text || "", replacements);
      label.richMarkup = replaceRichMarkupPathValues(label.binding.templateRichMarkup, valuesBySlot)
        || buildPlainTextRichMarkup(label.text);
    },
    "point-coordinate-value"(env, scene, label) {
      const point = scene.points[label.binding.pointIndex];
      if (!point) return;
      const coordinates = pointCoordinatesInBasis(
        point,
        scene.points[label.binding.originIndex],
        scene.points[label.binding.xUnitIndex],
        scene.points[label.binding.yUnitIndex],
      ) ?? point;
      label.text = `${label.binding.pointName}: (${env.formatNumber(coordinates.x)}, ${env.formatNumber(coordinates.y)})`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "point-distance-value"(env, scene, label) {
      const value = pointDistanceValue(scene, label.binding);
      if (value === null) return;
      label.text = `${label.binding.name} = ${env.formatNumber(value)}${label.binding.valueSuffix || ""}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "point-angle-value"(_env, scene, label) {
      const value = pointAngleValue(scene, label.binding);
      if (value === null) return;
      label.text = `${label.binding.name} = ${value.toFixed(2)}${label.binding.valueSuffix || ""}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "polygon-area-value"(env, scene, label) {
      const value = polygonAreaValue(scene, label.binding);
      if (value === null) return;
      label.text = `${label.binding.name} = ${env.formatNumber(value)}${label.binding.valueSuffix || ""}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "point-distance-ratio-value"(env, scene, label) {
      const value = pointDistanceRatioValue(scene, label.binding);
      if (value === null) return;
      const valueText = env.formatNumber(value);
      label.text = `${label.binding.name} = ${valueText}`;
      label.richMarkup = buildRatioValueRichMarkup(label.binding.name, valueText)
        || buildPlainTextRichMarkup(label.text);
    },
    "point-axis-value"(env, scene, label) {
      const point = scene.points[label.binding.pointIndex];
      if (!point) return;
      const coordinates = pointCoordinatesInBasis(
        point,
        scene.points[label.binding.originIndex],
        scene.points[label.binding.xUnitIndex],
        scene.points[label.binding.yUnitIndex],
      );
      const value = label.binding.axis === "vertical"
        ? (coordinates?.y ?? point.y)
        : (coordinates?.x ?? point.x);
      label.text = `${label.binding.name} = ${env.formatAxisNumber(value)}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "expression-value"(env, _scene, label, parameters) {
      const value = evaluateExpr(label.binding.expr, 0, parameters);
      const valueText = value !== null
        ? (label.binding.exprLabel.includes("°") ? `${value.toFixed(2)}°` : env.formatNumber(value))
        : "未定义";
      label.richMarkup = buildExpressionRichMarkup(
        label.binding.exprLabel,
        valueText,
      );
      if (value !== null) {
        label.text = `${label.binding.exprLabel} = ${valueText}`;
      } else {
        label.text = `${label.binding.exprLabel} = 未定义`;
      }
    },
    "point-bound-expression-value"(env, _scene, label, parameters) {
      const value = evaluateExpr(label.binding.expr, 0, parameters);
      const valueText = value !== null
        ? (label.binding.exprLabel.includes("°") ? `${value.toFixed(2)}°` : env.formatNumber(value))
        : "未定义";
      label.richMarkup = buildExpressionRichMarkup(
        label.binding.exprLabel,
        valueText,
      );
      if (value !== null) {
        label.text = `${label.binding.exprLabel} = ${valueText}`;
      } else {
        label.text = `${label.binding.exprLabel} = 未定义`;
      }
    },
    "polygon-boundary-parameter"(env, scene, label) {
      const value = polygonBoundaryParameterFromPoint(scene, label.binding.pointIndex);
      if (value !== null) {
        label.text = label.binding.polygonName
          ? `${label.binding.pointName}在${label.binding.polygonName}上的值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "segment-parameter"(env, scene, label) {
      const value = parameterValueFromPoint(scene, label.binding.pointIndex);
      if (value !== null) {
        label.text = usesVerboseParameterLabel(label)
          ? `${label.binding.pointName}在${label.binding.segmentName}上的t值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "segment-projection-parameter"(env, scene, label) {
      const value = segmentProjectionParameterFromBinding(scene, label.binding);
      if (value !== null) {
        label.text = usesVerboseParameterLabel(label)
          ? `${label.binding.pointName}在${label.binding.segmentName}上的值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "polyline-parameter"(env, scene, label) {
      const value = polylineParameterFromPoint(scene, label.binding.pointIndex);
      if (value !== null) {
        label.text = usesVerboseParameterLabel(label)
          ? `${label.binding.pointName}在${label.binding.objectName}上的值 = ${env.formatNumber(value)}`
          : `${label.binding.pointName} = ${env.formatNumber(value)}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "circle-parameter"(env, scene, label) {
      const point = scene.points[label.binding.pointIndex];
      const constraint = point?.constraint;
      if (constraint?.kind !== "circle") return;
      const pointAngle = Math.atan2(-constraint.unitY, constraint.unitX);
      const tau = Math.PI * 2;
      const value = ((pointAngle % tau) + tau) % tau / tau;
      label.text = usesVerboseParameterLabel(label)
        ? `${label.binding.pointName}在⊙${label.binding.circleName}上的值 = ${env.formatNumber(value)}`
        : `${label.binding.pointName} = ${env.formatNumber(value)}`;
      label.richMarkup = buildPlainTextRichMarkup(label.text);
    },
    "angle-marker-value"(_env, scene, label) {
      const start = scene.points[label.binding.startIndex];
      const vertex = scene.points[label.binding.vertexIndex];
      const end = scene.points[label.binding.endIndex];
      if (!start || !vertex || !end) return;
      const first = { x: start.x - vertex.x, y: start.y - vertex.y };
      const second = { x: end.x - vertex.x, y: end.y - vertex.y };
      const firstLen = Math.hypot(first.x, first.y);
      const secondLen = Math.hypot(second.x, second.y);
      if (firstLen <= 1e-9 || secondLen <= 1e-9) return;
      const cross = (first.x / firstLen) * (second.y / secondLen)
        - (first.y / firstLen) * (second.x / secondLen);
      const dot = (first.x / firstLen) * (second.x / secondLen)
        + (first.y / firstLen) * (second.y / secondLen);
      const value = Math.abs(Math.atan2(cross, dot)) * 180 / Math.PI;
      if (Number.isFinite(value)) {
        label.text = value.toFixed(label.binding.decimals);
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
    "custom-transform-value"(env, scene, label, parameters) {
      const value = parameterValueFromPoint(scene, label.binding.pointIndex);
      if (!isFiniteNumber(value)) return;
      const exprParameters = new Map(parameters);
      const names = new Set();
      collectExprParameterNames(label.binding.expr, names);
      names.forEach((name) => exprParameters.set(name, value));
      const evaluated = evaluateExpr(label.binding.expr, value, exprParameters);
      if (evaluated !== null) {
        label.text = `${label.binding.exprLabel} = ${env.formatNumber(evaluated * label.binding.valueScale)}${label.binding.valueSuffix}`;
        label.richMarkup = buildPlainTextRichMarkup(label.text);
      }
    },
  };

  /** @type {Record<string, PointBindingRefresher>} */
  const SYNC_DYNAMIC_POINT_BINDING_UPDATERS = {
    coordinate(_env, _draft, point, parameters) {
      const value = parameters.get(point.binding.name);
      if (!isFiniteNumber(value)) return;
      point.x = value;
      const y = evaluateExpr(point.binding.expr, 0, parameters);
      if (y !== null) {
        point.y = y;
      }
    },
    "coordinate-source"(_env, draft, point, parameters) {
      updateCoordinateSourcePoint(point, draft.points[point.binding.sourceIndex], parameters);
    },
    "coordinate-source-2d"(_env, draft, point, parameters) {
      updateCoordinateSource2dPoint(point, draft.points[point.binding.sourceIndex], parameters);
    },
    "custom-transform"(_env, draft, point, parameters) {
      updateCustomTransformPoint(point, parameters, (index) => draft.points[index], draft);
    },
    "scale-by-ratio"(_env, draft, point) {
      updateScaleByRatioPoint(point, (index) => draft.points[index]);
    },
    circumcenter(_env, draft, point) {
      const start = draft.points[point.binding.startIndex];
      const mid = draft.points[point.binding.midIndex];
      const end = draft.points[point.binding.endIndex];
      if (!start || !mid || !end) return;
      const center = circumcenter(start, mid, end);
      if (!center) return;
      point.x = center.x;
      point.y = center.y;
    },
    "constraint-parameter-expr"(_env, draft, point, parameters) {
      const value = evaluateExpr(point.binding.expr, 0, parameters);
      if (isFiniteNumber(value)) {
        updateConstraintParameterizedPoint(point, draft, value);
      }
    },
    "constraint-parameter-from-point-expr"(_env, draft, point, parameters) {
      const sourceValue = parameterValueFromPoint(draft, point.binding.sourceIndex);
      if (!isFiniteNumber(sourceValue)) return;
      const exprParameters = new Map(parameters);
      if (point.binding.parameterName) {
        exprParameters.set(point.binding.parameterName, sourceValue);
      }
      const value = evaluateExpr(point.binding.expr, 0, exprParameters);
      if (value !== null) {
        updateConstraintParameterizedPoint(
          point,
          draft,
          point.binding.absoluteValue === true ? value : sourceValue + value,
        );
      }
    },
  };

  /**
   * @param {ViewerSceneData} scene
   * @param {LineBindingJson} binding
   * @returns {Point[] | null}
   */
  function resolveHostLinePoints(scene, binding) {
    const hostBinding = /** @type {{ lineStartIndex?: number | null; lineEndIndex?: number | null; lineIndex?: number | null }} */ (binding);
    if (
      typeof hostBinding?.lineStartIndex === "number"
      && typeof hostBinding?.lineEndIndex === "number"
    ) {
      const start = scene.points[hostBinding.lineStartIndex];
      const end = scene.points[hostBinding.lineEndIndex];
      return start && end ? [start, end] : null;
    }
    if (typeof hostBinding?.lineIndex === "number") {
      const hostLine = scene.lines[hostBinding.lineIndex];
      return hostLine?.points?.length >= 2 ? hostLine.points : null;
    }
    return null;
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {RuntimeLineJson} line
   * @param {Map<string, number>} parameters
   * @returns {Point[] | null}
   */
  function sampleCustomTransformTraceLine(scene, line, parameters) {
    const point = scene.points[line.binding.pointIndex];
    const binding = point?.binding;
    if (binding?.kind !== "custom-transform") return null;
    const origin = scene.points[binding.originIndex];
    const axisEnd = scene.points[binding.axisEndIndex];
    const traceMax = parameterValueFromPoint(scene, binding.sourceIndex);
    if (!origin || !axisEnd || !isFiniteNumber(traceMax)) return null;
    const sampled = [];
    const last = Math.max(1, line.binding.sampleCount - 1);
    const maxValue = Math.max(line.binding.xMin, Math.min(line.binding.xMax, traceMax));
    for (let index = 0; index < line.binding.sampleCount; index += 1) {
      const value = line.binding.xMin + (maxValue - line.binding.xMin) * (index / last);
      const exprParameters = new Map(parameters);
      const names = new Set();
      collectExprParameterNames(binding.distanceExpr, names);
      collectExprParameterNames(binding.angleExpr, names);
      names.forEach((name) => exprParameters.set(name, value));
      const distanceValue = evaluateExpr(binding.distanceExpr, value, exprParameters);
      const angleValue = evaluateExpr(binding.angleExpr, value, exprParameters);
      if (distanceValue === null || angleValue === null) continue;
      const baseAngle = Math.atan2(-(axisEnd.y - origin.y), axisEnd.x - origin.x) * 180 / Math.PI;
      const radians = (baseAngle + angleValue * binding.angleDegreesScale) * Math.PI / 180;
      const distance = distanceValue * binding.distanceRawScale;
      sampled.push({
        x: origin.x + distance * Math.cos(radians),
        y: origin.y - distance * Math.sin(radians),
      });
    }
    return sampled.length >= 2 ? sampled : null;
  }

  /** @param {Point} point */
  function cloneTracePoint(point) {
    if (typeof structuredClone === "function") {
      return structuredClone(point);
    }
    return JSON.parse(JSON.stringify(point));
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {RuntimeLineJson} line
   * @param {Map<string, number>} parameters
   * @returns {Point[] | null}
   */
  function samplePointTraceLine(scene, line, parameters) {
    const driver = scene.points[line.binding.driverIndex];
    if (!driver?.constraint) return null;
    const tracedPoint = scene.points[line.binding.pointIndex];
    const sourceBinding = tracedPoint?.binding;
    const sourcePoint = sourceBinding?.kind === "coordinate-source-2d"
      ? scene.points[sourceBinding.sourceIndex]
      : null;
    if (sourceBinding?.kind === "coordinate-source-2d" && sourcePoint) {
      const baseParameters = deriveLabelParameters(scene, new Map(parameters));
      const driverParameterName = parameterNameFromPoint(scene, line.binding.driverIndex);
      const xNames = new Set();
      const yNames = new Set();
      collectExprParameterNames(sourceBinding.xExpr, xNames);
      collectExprParameterNames(sourceBinding.yExpr, yNames);
      const sampled = [];
      const sampleCount = Math.max(2, line.binding.sampleCount || 2);
      const last = Math.max(1, sampleCount - 1);
      for (let index = 0; index < sampleCount; index += 1) {
        const value = line.binding.useMidpoints
          ? line.binding.xMin
            + (line.binding.xMax - line.binding.xMin) * ((index + 0.5) / sampleCount)
          : line.binding.xMin + (line.binding.xMax - line.binding.xMin) * (index / last);
        const exprParameters = new Map(baseParameters);
        if (driverParameterName) {
          exprParameters.set(driverParameterName, value);
        }
        xNames.forEach((name) => {
          if (!exprParameters.has(name)) {
            exprParameters.set(name, value);
          }
        });
        yNames.forEach((name) => {
          if (!exprParameters.has(name)) {
            exprParameters.set(name, value);
          }
        });
        const dx = evaluateExpr(sourceBinding.xExpr, 0, exprParameters);
        const dy = evaluateExpr(sourceBinding.yExpr, 0, exprParameters);
        if (dx === null || dy === null) {
          continue;
        }
        sampled.push({
          x: sourcePoint.x + dx,
          y: sourcePoint.y + dy,
        });
      }
      return sampled.length >= 2 ? sampled : null;
    }
    const sampleScene = {
      ...scene,
      lines: scene.lines,
      circles: scene.circles,
      /** @type {RuntimeScenePointJson[]} */
      points: [],
    };
    /** @type {Map<string, number>} */
    let baseParameters = new Map(parameters);
    let driverValue = Number.NaN;
    /** @type {Map<number, Point | null>} */
    let resolvedCache = new Map();

    /**
     * @param {RuntimeScenePointJson[]} points
     * @param {number} index
     * @param {Set<number>} [visiting]
     * @returns {Point | null}
     */
    const resolveTracePoint = (points, index, visiting = new Set()) => {
      if (resolvedCache.has(index)) {
        return resolvedCache.get(index) ?? null;
      }
      if (visiting.has(index)) return null;
      const point = points[index];
      if (!point) return null;
      visiting.add(index);
      /**
       * @param {Map<string, number>} exprParameters
       * @param {...FunctionExprJson} exprs
       */
      const applyDriverParameterGuesses = (exprParameters, ...exprs) => {
        if (!Number.isFinite(driverValue)) return exprParameters;
        const names = new Set();
        exprs.forEach((expr) => collectExprParameterNames(expr, names));
        names.forEach((name) => {
          if (!exprParameters.has(name)) {
            exprParameters.set(name, driverValue);
          }
        });
        return exprParameters;
      };

      let resolved = null;
      if (point.binding?.kind === "derived") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const transform = point.binding.transform;
        if (transform.kind === "translate") {
          const vectorStart = resolveTracePoint(points, transform.vectorStartIndex, visiting);
          const vectorEnd = resolveTracePoint(points, transform.vectorEndIndex, visiting);
          if (source && vectorStart && vectorEnd) {
            resolved = {
              x: source.x + (vectorEnd.x - vectorStart.x),
              y: source.y + (vectorEnd.y - vectorStart.y),
            };
          }
        } else if (transform.kind === "reflect") {
          const lineStart = resolveTracePoint(points, transform.lineStartIndex, visiting);
          const lineEnd = resolveTracePoint(points, transform.lineEndIndex, visiting);
          if (source && lineStart && lineEnd) {
            resolved = reflectAcrossLine(source, lineStart, lineEnd);
          }
        } else if (transform.kind === "reflect-constraint") {
          const line = resolveLineConstraintPoints(
            (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
            scene.bounds,
            transform.line,
          );
          if (source && line) {
            resolved = reflectAcrossLine(source, line[0], line[1]);
          }
        } else if (transform.kind === "rotate") {
          const center = resolveTracePoint(points, transform.centerIndex, visiting);
          const angleDegrees = resolveRotateTransformAngleDegrees(
            transform,
            baseParameters,
            (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          );
          if (source && center && isFiniteNumber(angleDegrees)) {
            resolved = rotateAround(source, center, angleDegrees * Math.PI / 180);
          }
        } else if (transform.kind === "scale") {
          const center = resolveTracePoint(points, transform.centerIndex, visiting);
          const factor = resolveScaleTransformFactor(
            transform,
            baseParameters,
            (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          );
          if (source && center && isFiniteNumber(factor)) {
            resolved = scaleAround(source, center, factor);
          }
        }
      } else if (point.binding?.kind === "scale-by-ratio") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const center = resolveTracePoint(points, point.binding.centerIndex, visiting);
        const ratioOrigin = resolveTracePoint(points, point.binding.ratioOriginIndex, visiting);
        const ratioDenominator = resolveTracePoint(points, point.binding.ratioDenominatorIndex, visiting);
        const ratioNumerator = resolveTracePoint(points, point.binding.ratioNumeratorIndex, visiting);
        if (source && center && ratioOrigin && ratioDenominator && ratioNumerator) {
          resolved = scaleByThreePointRatio(
            source,
            center,
            ratioOrigin,
            ratioDenominator,
            ratioNumerator,
            point.binding.signed !== false,
            point.binding.clampToUnit === true,
          );
        }
      } else if (point.binding?.kind === "midpoint") {
        const start = resolveTracePoint(points, point.binding.startIndex, visiting);
        const end = resolveTracePoint(points, point.binding.endIndex, visiting);
        if (start && end) {
          resolved = lerpPoint(start, end, 0.5);
        }
      } else if (point.binding?.kind === "circumcenter") {
        const start = resolveTracePoint(points, point.binding.startIndex, visiting);
        const mid = resolveTracePoint(points, point.binding.midIndex, visiting);
        const end = resolveTracePoint(points, point.binding.endIndex, visiting);
        if (start && mid && end) {
          resolved = circumcenter(start, mid, end);
        }
      } else if (point.binding?.kind === "coordinate") {
        const exprParameters = applyDriverParameterGuesses(new Map(baseParameters), point.binding.expr);
        if (typeof point.binding.name === "string" && !exprParameters.has(point.binding.name) && Number.isFinite(driverValue)) {
          exprParameters.set(point.binding.name, driverValue);
        }
        const x = exprParameters.get(point.binding.name);
        const y = evaluateExpr(point.binding.expr, 0, exprParameters);
        if (isFiniteNumber(x) && y !== null) {
          resolved = { x, y };
        }
      } else if (point.binding?.kind === "coordinate-source") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const exprParameters = applyDriverParameterGuesses(new Map(baseParameters), point.binding.expr);
        if (typeof point.binding.name === "string" && !exprParameters.has(point.binding.name) && Number.isFinite(driverValue)) {
          exprParameters.set(point.binding.name, driverValue);
        }
        const offset = evaluateExpr(point.binding.expr, 0, exprParameters);
        if (source && offset !== null) {
          resolved = point.binding.axis === "horizontal"
            ? { x: source.x + offset, y: source.y }
            : { x: source.x, y: source.y + offset };
        }
      } else if (point.binding?.kind === "coordinate-source-2d") {
        const source = resolveTracePoint(points, point.binding.sourceIndex, visiting);
        const exprParameters = applyDriverParameterGuesses(
          new Map(baseParameters),
          point.binding.xExpr,
          point.binding.yExpr,
        );
        const dx = evaluateExpr(point.binding.xExpr, 0, exprParameters);
        const dy = evaluateExpr(point.binding.yExpr, 0, exprParameters);
        if (source && dx !== null && dy !== null) {
          resolved = { x: source.x + dx, y: source.y + dy };
        }
      } else if (point.binding?.kind === "constraint-parameter-expr") {
        const value = evaluateExpr(point.binding.expr, 0, baseParameters);
        if (value !== null) {
          const derived = cloneTracePoint(point);
          updateConstraintParameterizedPoint(derived, sampleScene, value);
          sampleScene.points[index] = derived;
          resolved = modules.scene.resolveConstrainedPoint(
            {
              sourceScene: scene,
              currentScene: () => sampleScene,
              resolveScenePoint: (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
            },
            derived.constraint,
            (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
            derived,
          );
        }
      } else if (point.binding?.kind === "constraint-parameter-from-point-expr") {
        const sourceValue = parameterValueFromPoint(sampleScene, point.binding.sourceIndex);
        if (isFiniteNumber(sourceValue)) {
          const exprParameters = new Map(baseParameters);
          if (point.binding.parameterName) {
            exprParameters.set(point.binding.parameterName, sourceValue);
          }
          const exprValue = evaluateExpr(point.binding.expr, 0, exprParameters);
          if (exprValue !== null) {
            const derived = cloneTracePoint(point);
            updateConstraintParameterizedPoint(
              derived,
              sampleScene,
              point.binding.absoluteValue === true ? exprValue : sourceValue + exprValue,
            );
            sampleScene.points[index] = derived;
            resolved = modules.scene.resolveConstrainedPoint(
              {
                sourceScene: scene,
                currentScene: () => sampleScene,
                resolveScenePoint: (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
              },
              derived.constraint,
              (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
              derived,
            );
          }
        }
      } else if (point.binding?.kind === "custom-transform") {
        const derived = { ...point };
        updateCustomTransformPoint(derived, baseParameters, (pointIndex) => resolveTracePoint(points, pointIndex, visiting), sampleScene);
        if (Number.isFinite(derived.x) && Number.isFinite(derived.y)) {
          resolved = { x: derived.x, y: derived.y };
        }
      }

      if (!resolved && point.constraint) {
        sampleScene.points = points;
        resolved = modules.scene.resolveConstrainedPoint(
          {
            sourceScene: scene,
            currentScene: () => sampleScene,
            resolveScenePoint: (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          },
          point.constraint,
          (pointIndex) => resolveTracePoint(points, pointIndex, visiting),
          point,
        );
      }

      visiting.delete(index);
      const finalPoint = resolved || (point.constraint ? null : point);
      resolvedCache.set(index, finalPoint);
      return finalPoint;
    };

    const sampled = [];
    const last = Math.max(1, line.binding.sampleCount - 1);
    for (let index = 0; index < line.binding.sampleCount; index += 1) {
      const value = line.binding.useMidpoints
        ? line.binding.xMin
          + (line.binding.xMax - line.binding.xMin) * ((index + 0.5) / Math.max(1, line.binding.sampleCount))
        : line.binding.xMin + (line.binding.xMax - line.binding.xMin) * (index / last);
      const points = scene.points.map(cloneTracePoint);
      sampleScene.points = points;
      applyTraceValueToPoint(
        points[line.binding.driverIndex],
        sampleScene,
        value,
        line.binding.xMin,
        line.binding.xMax,
      );
      const driverPoint = points[line.binding.driverIndex];
      const resolvedDriver = driverPoint?.constraint
        ? modules.scene.resolveConstrainedPoint(
          {
            sourceScene: scene,
            currentScene: () => sampleScene,
            resolveScenePoint: (pointIndex) => points[pointIndex],
          },
          driverPoint.constraint,
          (pointIndex) => points[pointIndex],
          driverPoint,
        )
        : null;
      if (resolvedDriver) {
        driverPoint.x = resolvedDriver.x;
        driverPoint.y = resolvedDriver.y;
      }
      baseParameters = deriveLabelParameters(sampleScene, new Map(parameters));
      driverValue = parameterValueFromPoint(sampleScene, line.binding.driverIndex) ?? Number.NaN;
      resolvedCache = new Map();
      const point = resolveTracePoint(points, line.binding.pointIndex);
      if (point) {
        sampled.push({ x: point.x, y: point.y });
      }
    }
    return sampled.length >= 2 ? sampled : null;
  }

  /**
   * @typedef {{ kind: "translate"; dx: number; dy: number } | { kind: "rotate"; center: Point; radians: number } | { kind: "scale"; center: Point; factor: number } | { kind: "reflect"; lineStart: Point; lineEnd: Point }} DerivedTransform
   */

  /**
   * @param {import("./generated/TransformJson").TransformJson} transform
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   * @returns {DerivedTransform | null}
   */
  function resolveDerivedTransform(transform, scene, parameters) {
    if (transform.kind === "translate") {
      const vectorStart = scene.points[transform.vectorStartIndex];
      const vectorEnd = scene.points[transform.vectorEndIndex];
      if (!vectorStart || !vectorEnd) return null;
      return {
        kind: "translate",
        dx: vectorEnd.x - vectorStart.x,
        dy: vectorEnd.y - vectorStart.y,
      };
    }
    if (transform.kind === "translate-delta") {
      return { kind: "translate", dx: transform.dx, dy: transform.dy };
    }
    if (transform.kind === "rotate") {
      const center = scene.points[transform.centerIndex];
      if (!center) return null;
      const angleDegrees = transform.parameterName
        ? parameters.get(transform.parameterName)
        : transform.angleDegrees;
      if (!isFiniteNumber(angleDegrees)) return null;
      return { kind: "rotate", center, radians: angleDegrees * Math.PI / 180 };
    }
    if (transform.kind === "scale") {
      const center = scene.points[transform.centerIndex];
      if (!center) return null;
      const factor = resolveScaleTransformFactor(
        transform,
        parameters,
        (pointIndex) => scene.points[pointIndex] || null,
      );
      if (!isFiniteNumber(factor)) return null;
      return { kind: "scale", center, factor };
    }
    if (transform.kind === "reflect") {
      const [lineStart, lineEnd] = reflectionAxisPoints(scene, transform);
      if (!lineStart || !lineEnd) return null;
      return { kind: "reflect", lineStart, lineEnd };
    }
    return null;
  }

  /**
   * @param {Point} point
   * @param {DerivedTransform} transform
   * @returns {Point | null}
   */
  function applyDerivedTransform(point, transform) {
    if (transform.kind === "translate") {
      return { x: point.x + transform.dx, y: point.y + transform.dy };
    }
    if (transform.kind === "rotate") {
      return rotateAround(point, transform.center, transform.radians);
    }
    if (transform.kind === "scale") {
      return scaleAround(point, transform.center, transform.factor);
    }
    return reflectAcrossLine(point, transform.lineStart, transform.lineEnd);
  }

  /**
   * @param {{ scene: ViewerSceneData, parameters: Map<string, number> }} env
   * @param {RuntimeLineJson} line
   */
  function refreshDerivedLine(env, line) {
    const source = env.scene.lines[line.binding.sourceIndex];
    const transform = resolveDerivedTransform(line.binding.transform, env.scene, env.parameters);
    if (!source || !transform) return;
    const nextPoints = source.points
      .map((/** @type {Point} */ point) => applyDerivedTransform(point, transform));
    if (nextPoints.some((/** @type {Point | null} */ point) => !point)) return;
    line.points = /** @type {Point[]} */ (nextPoints);
  }

  /**
   * @param {{ scene: ViewerSceneData, bounds: BoundsJson, parameters: Map<string, number> }} context
   * @param {RuntimeLineJson} line
   */
  function refreshColorizedSpectrumLine(context, line) {
    const binding = line.binding;
    const hostLine = context.scene.lines[binding.lineIndex];
    const traceLine = context.scene.lines[binding.traceLineIndex];
    const baseParameter = polylineParameterFromPoint(context.scene, binding.pointIndex);
    if (!traceLine?.points || traceLine.points.length < 2 || !isFiniteNumber(baseParameter)) {
      return;
    }
    const rawDepth = binding.depthParameterName
      ? context.parameters.get(binding.depthParameterName)
      : binding.depth;
    const depth = discreteIterationDepth(isFiniteNumber(rawDepth) ? rawDepth : binding.depth);
    line.visible = binding.stepIndex < depth;
    if (depth <= 0 || binding.stepIndex >= depth) {
      return;
    }
    line.color = hsbToRgba((binding.stepIndex || 0) / depth, 1, 1, 255);
    const sample = pointOnPolylineByIndex(
      traceLine.points,
      baseParameter + (binding.stepIndex || 0) / depth,
    );
    if (!sample) return;

    const hostPoints = hostLine?.points;
    if (!hostPoints || hostPoints.length < 2) return;
    const traceEndpointIndex = binding.traceEndpointIndex === 1 ? 1 : 0;
    const hostStart = hostPoints[traceEndpointIndex];
    let hostEnd = hostPoints[1 - traceEndpointIndex];
    let rayStart = hostStart;
    let rayEnd = hostEnd;
    if (
      isFiniteNumber(binding.reflectionSourceIndex)
      && isFiniteNumber(binding.reflectionAxisLineIndex)
    ) {
      const source = context.scene.points[binding.reflectionSourceIndex];
      const sampledAxis = sampledReflectionAxis(context.scene, binding, sample);
      const axisLine = sampledAxis ? null : context.scene.lines[binding.reflectionAxisLineIndex];
      const axisStart = sampledAxis?.[0] ?? axisLine?.points?.[0];
      const axisEnd = sampledAxis?.[1] ?? axisLine?.points?.[axisLine.points.length - 1];
      if (source && axisStart && axisEnd) {
        const reflected = reflectAcrossLine(source, axisStart, axisEnd);
        if (reflected) {
          if (sampledAxis && binding.ray) {
            rayStart = reflected;
            rayEnd = sample;
          } else {
            rayStart = sample;
            rayEnd = reflected;
          }
          hostEnd = reflected;
        }
      }
    }
    if (!hostStart || !hostEnd || !rayStart || !rayEnd) return;

    if (binding.ray) {
      const dx = rayEnd.x - rayStart.x;
      const dy = rayEnd.y - rayStart.y;
      if (Math.hypot(dx, dy) <= 1e-9) return;
      const clipped = clipRayToBounds(sample, { x: sample.x + dx, y: sample.y + dy }, context.bounds);
      if (clipped) {
        line.points = clipped;
      }
      return;
    }

    line.points = [sample, { x: hostEnd.x, y: hostEnd.y }];
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {LineBindingJson & { kind: "colorized-spectrum" }} binding
   * @param {Point} sample
   * @returns {[Point, Point] | null}
   */
  function sampledReflectionAxis(scene, binding, sample) {
    if (
      !isFiniteNumber(binding.reflectionFocusIndex)
      || !isFiniteNumber(binding.reflectionDirectrixLineIndex)
    ) {
      return null;
    }
    const focus = scene.points[binding.reflectionFocusIndex];
    const directrixLine = scene.lines[binding.reflectionDirectrixLineIndex];
    const directrixStart = directrixLine?.points?.[0];
    const directrixEnd = directrixLine?.points?.[directrixLine.points.length - 1];
    if (!focus || !directrixStart || !directrixEnd) return null;
    const projection = projectPointToLine(sample, directrixStart, directrixEnd);
    if (!projection) return null;
    const normalX = focus.x - projection.x;
    const normalY = focus.y - projection.y;
    if (Math.hypot(normalX, normalY) <= 1e-9) return null;
    return [
      sample,
      { x: sample.x - normalY, y: sample.y + normalX },
    ];
  }

  /**
   * @param {Point} point
   * @param {Point} lineStart
   * @param {Point} lineEnd
   * @returns {Point | null}
   */
  function projectPointToLine(point, lineStart, lineEnd) {
    const dx = lineEnd.x - lineStart.x;
    const dy = lineEnd.y - lineStart.y;
    const lenSq = dx * dx + dy * dy;
    if (lenSq <= 1e-9) return null;
    const t = ((point.x - lineStart.x) * dx + (point.y - lineStart.y) * dy) / lenSq;
    return {
      x: lineStart.x + t * dx,
      y: lineStart.y + t * dy,
    };
  }

  /**
   * @param {{ scene: ViewerSceneData, parameters: Map<string, number>, resolveHandle: (handle: PointHandle) => Point | null }} env
   * @param {RuntimePolygonJson} polygon
   */
  function refreshDerivedPolygon(env, polygon) {
    const source = env.scene.polygons[polygon.binding.sourceIndex];
    const transform = resolveDerivedTransform(polygon.binding.transform, env.scene, env.parameters);
    if (!source || !transform) return;
    const nextPoints = source.points
      .map((/** @type {PointHandle} */ handle) => env.resolveHandle(handle))
      .filter(Boolean)
      .map((/** @type {Point} */ point) => applyDerivedTransform(point, transform));
    if (nextPoints.some((/** @type {Point | null} */ point) => !point)) return;
    polygon.points = /** @type {Point[]} */ (nextPoints);
  }

  /**
   * @param {{ scene: ViewerSceneData, parameters: Map<string, number>, resolveHandle: (handle: PointHandle) => Point | null }} env
   * @param {RuntimeCircleJson} circle
   */
  function refreshDerivedCircle(env, circle) {
    const source = env.scene.circles[circle.binding.sourceIndex];
    const transform = resolveDerivedTransform(circle.binding.transform, env.scene, env.parameters);
    if (!source || !transform) return;
    const sourceCenter = env.resolveHandle(source.center);
    const sourceRadius = env.resolveHandle(source.radiusPoint);
    if (!sourceCenter || !sourceRadius) return;
    const nextCenter = applyDerivedTransform(sourceCenter, transform);
    const nextRadius = applyDerivedTransform(sourceRadius, transform);
    if (!nextCenter || !nextRadius) return;
    circle.center = nextCenter;
    circle.radiusPoint = nextRadius;
  }

  /** @type {Record<string, LineBindingRefresher>} */
  const LINE_BINDING_REFRESHERS = {
    segment({ scene }, line) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      if (start && end) {
        line.points = [{ x: start.x, y: start.y }, { x: end.x, y: end.y }];
      }
    },
    "angle-marker"({ scene }, line) {
      const start = scene.points[line.binding.startIndex];
      const vertex = scene.points[line.binding.vertexIndex];
      const end = scene.points[line.binding.endIndex];
      const points = start && vertex && end
        ? resolveAngleMarkerPoints(start, vertex, end, line.binding.markerClass)
        : null;
      if (points) {
        line.points = points;
      }
    },
    "angle-bisector-ray"({ scene, bounds }, line) {
      const start = scene.points[line.binding.startIndex];
      const vertex = scene.points[line.binding.vertexIndex];
      const end = scene.points[line.binding.endIndex];
      if (start && vertex && end) {
        const direction = angleBisectorDirection(start, vertex, end);
        const clipped = direction
          ? clipRayToBounds(
              vertex,
              { x: vertex.x + direction.x, y: vertex.y + direction.y },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    "perpendicular-line"({ scene, bounds }, line) {
      const through = scene.points[line.binding.throughIndex];
      const hostLine = resolveHostLinePoints(scene, line.binding);
      const lineStart = hostLine?.[0];
      const lineEnd = hostLine?.[1];
      if (through && lineStart && lineEnd) {
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        const clipped = len > 1e-9
          ? clipLineToBounds(
              through,
              { x: through.x - dy / len, y: through.y + dx / len },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    "parallel-line"({ scene, bounds }, line) {
      const through = scene.points[line.binding.throughIndex];
      const hostLine = resolveHostLinePoints(scene, line.binding);
      const lineStart = hostLine?.[0];
      const lineEnd = hostLine?.[1];
      if (through && lineStart && lineEnd) {
        const dx = lineEnd.x - lineStart.x;
        const dy = lineEnd.y - lineStart.y;
        const len = Math.hypot(dx, dy);
        const clipped = len > 1e-9
          ? clipLineToBounds(
              through,
              { x: through.x + dx / len, y: through.y + dy / len },
              bounds,
            )
          : null;
        if (clipped) line.points = clipped;
      }
    },
    line({ scene, bounds }, line) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      const clipped = start && end ? clipLineToBounds(start, end, bounds) : null;
      if (clipped) line.points = clipped;
    },
    ray({ scene, bounds }, line) {
      const start = scene.points[line.binding.startIndex];
      const end = scene.points[line.binding.endIndex];
      const clipped = start && end ? clipRayToBounds(start, end, bounds) : null;
      if (clipped) line.points = clipped;
    },
    "arc-boundary"({ env }, line) {
      const sampled = modules.scene.sampleArcBoundaryPoints(env, line.binding);
      if (sampled) {
        line.points = sampled;
      }
    },
    derived: refreshDerivedLine,
    "custom-transform-trace"({ scene, parameters }, line) {
      const sampled = sampleCustomTransformTraceLine(scene, line, parameters);
      if (sampled) {
        line.points = sampled;
      }
    },
    "coordinate-trace"({ env }, line) {
      const sampled = modules.scene.sampleCoordinateTracePoints(env, line.binding);
      if (sampled && sampled.length >= 2) {
        line.points = sampled;
      }
    },
    "point-trace"({ scene, parameters }, line) {
      const sampled = samplePointTraceLine(scene, line, parameters);
      if (sampled) {
        line.points = sampled;
      }
    },
    "colorized-spectrum": refreshColorizedSpectrumLine,
    "parametric-curve"({ parameters }, line) {
      const sampled = sampleParametricCurve(line.binding, parameters);
      if (sampled.length >= 2) {
        line.points = sampled;
      }
    },
  };

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {number} index
   * @param {Set<number>} [visiting]
   * @returns {Point | null}
   */
  function resolveScenePointInScene(env, scene, index, visiting = new Set()) {
    const point = scene.points[index];
    if (!point) return null;
    if (!point.constraint) return point;
    if (visiting.has(index)) return null;
    visiting.add(index);
    const resolved = modules.scene.resolveConstrainedPoint(
      env,
      point.constraint,
      (pointIndex) => resolveScenePointInScene(env, scene, pointIndex, visiting),
      point,
    );
    visiting.delete(index);
    return resolved;
  }

  /** @type {Record<string, CircleBindingRefresher>} */
  const CIRCLE_BINDING_REFRESHERS = {
    "point-radius-circle"({ env }, circle) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const radiusPoint = env.resolveScenePoint(circle.binding.radiusIndex);
      if (!center || !radiusPoint) return;
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: radiusPoint.x, y: radiusPoint.y };
    },
    "segment-radius-circle"({ env }, circle) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const lineStart = env.resolveScenePoint(circle.binding.lineStartIndex);
      const lineEnd = env.resolveScenePoint(circle.binding.lineEndIndex);
      if (!center || !lineStart || !lineEnd) return;
      const radius = Math.hypot(lineEnd.x - lineStart.x, lineEnd.y - lineStart.y);
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: center.x + radius, y: center.y };
    },
    "parameter-radius-circle"({ env, parameters }, circle) {
      const center = env.resolveScenePoint(circle.binding.centerIndex);
      const value = parameters.get(circle.binding.parameterName);
      if (!center || !isFiniteNumber(value)) return;
      const radius = Math.abs(value) * circle.binding.rawPerUnit;
      circle.center = { x: center.x, y: center.y };
      circle.radiusPoint = { x: center.x + radius, y: center.y };
    },
    derived: refreshDerivedCircle,
  };

  /** @type {Record<string, PolygonBindingRefresher>} */
  const POLYGON_BINDING_REFRESHERS = {
    "point-polygon"({ scene }, polygon) {
      const points = polygon.binding.vertexIndices
        .map((/** @type {number} */ index) => scene.points[index])
        .filter(Boolean);
      if (points.length === polygon.binding.vertexIndices.length) {
        polygon.points = points.map((/** @type {Point} */ point) => ({ x: point.x, y: point.y }));
      }
    },
    "arc-boundary-polygon"({ env }, polygon) {
      const sampled = modules.scene.sampleArcBoundaryPoints(env, polygon.binding);
      if (sampled) {
        polygon.points = sampled;
      }
    },
    derived: refreshDerivedPolygon,
  };

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function refreshDerivedPoints(env, scene) {
    const bounds = env.getViewBounds ? env.getViewBounds() : (scene.bounds || env.sourceScene.bounds);
    const parameters = parameterMapForScene(env, scene);
    const resolveHandle = (/** @type {PointHandle} */ handle) => {
      if (hasPointIndexHandle(handle)) {
        return env.resolveScenePoint(handle.pointIndex);
      }
      if (hasLineIndexHandle(handle)) {
        const line = scene.lines[handle.lineIndex];
        if (!line?.points || line.points.length < 2) return null;
        const segmentIndex = Math.max(0, Math.min(line.points.length - 2, handle.segmentIndex || 0));
        const t = typeof handle.t === "number" ? handle.t : 0.5;
        const p0 = line.points[segmentIndex];
        const p1 = line.points[segmentIndex + 1];
        return {
          x: p0.x + (p1.x - p0.x) * t,
          y: p0.y + (p1.y - p0.y) * t,
        };
      }
      return /** @type {Point} */ (handle);
    };

    scene.points.forEach((/** @type {RuntimeScenePointJson} */ point) => {
      const refreshBinding = point.binding ? DERIVED_POINT_BINDING_REFRESHERS[point.binding.kind] : null;
      if (refreshBinding) {
        refreshBinding(env, scene, point, parameters);
      }
    });

    refreshConstrainedPointPositions(env, scene);

    const shapeContext = { env, scene, parameters, resolveHandle };
    scene.circles.forEach((/** @type {RuntimeCircleJson} */ circle) => {
      const refreshCircle = circle.binding ? CIRCLE_BINDING_REFRESHERS[circle.binding.kind] : null;
      if (refreshCircle) {
        refreshCircle(shapeContext, circle);
      }
      refreshCircleFillColorBinding(scene, circle);
    });

    const sourceCircleIterations = env.sourceScene.circleIterations || [];
    if (sourceCircleIterations.length > 0) {
      const generatedCount = sourceCircleIterations.reduce((sum, family) => sum + family.depth, 0);
      const baseCount = Math.max(0, env.sourceScene.circles.length - generatedCount);
      scene.circles = scene.circles.slice(0, baseCount);
      sourceCircleIterations.forEach((/** @type {RuntimeCircleIterationFamily} */ family) => {
        const source = scene.circles[family.sourceCircleIndex];
        if (!source) {
          return;
        }
        const vertices = family.vertexIndices
          .map((/** @type {number} */ index) => scene.points[index])
          .filter(Boolean);
        if (vertices.length !== family.vertexIndices.length) {
          return;
        }
        const liveSeedParameter =
          polygonBoundaryParameterFromPoint(scene, family.sourceCenterIndex);
        const liveNextParameter =
          polygonBoundaryParameterFromPoint(scene, family.sourceNextCenterIndex);
        const seedParameter = isFiniteNumber(liveSeedParameter)
          ? liveSeedParameter
          : family.seedParameter;
        const stepParameter = isFiniteNumber(liveSeedParameter) && isFiniteNumber(liveNextParameter)
          ? ((liveNextParameter - liveSeedParameter) % 1 + 1) % 1
          : family.stepParameter;
        if (!isFiniteNumber(seedParameter) || !isFiniteNumber(stepParameter)) {
          return;
        }
        const depth = pointIterationDepth({
          depth: family.depth,
          parameterName: family.depthParameterName,
        }, parameters);
        const dx = source.radiusPoint.x - source.center.x;
        const dy = source.radiusPoint.y - source.center.y;
        for (let step = 1; step <= depth; step += 1) {
          const center = pointOnPolygonBoundary(
            vertices,
            seedParameter + stepParameter * step,
          );
          if (!center) {
            continue;
          }
          scene.circles.push({
            center,
            radiusPoint: {
              x: center.x + dx,
              y: center.y + dy,
            },
            color: source.color,
            fillColor: source.fillColor,
            fillVisible: source.fillVisible !== false,
            fillColorBinding: null,
            dashed: source.dashed,
            visible: family.visible !== false,
            binding: null,
          });
        }
      });
    }

    scene.polygons.forEach((/** @type {RuntimePolygonJson} */ polygon) => {
      const refreshPolygon = polygon.binding ? POLYGON_BINDING_REFRESHERS[polygon.binding.kind] : null;
      if (refreshPolygon) {
        refreshPolygon(shapeContext, polygon);
      }
    });

    /** @type {RuntimeLineJson[]} */
    const preservedLines = [];
    const lineContext = { env, scene, bounds, parameters };
    scene.lines.forEach((/** @type {RuntimeLineJson} */ line) => {
      const bindingKind = line.binding?.kind;
      if (!bindingKind) {
        preservedLines.push(line);
        return;
      }
      const refreshLine = LINE_BINDING_REFRESHERS[bindingKind];
      if (refreshLine) {
        refreshLine(lineContext, line);
      }
      preservedLines.push(line);
    });
    scene.lines = preservedLines;
    refreshTraceConstrainedPointPositions(env, scene);
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function refreshConstrainedPointPositions(env, scene) {
    scene.points.forEach((/** @type {RuntimeScenePointJson} */ point, /** @type {number} */ pointIndex) => {
      if (!point.constraint) {
        return;
      }
      const resolved = resolveScenePointInScene(env, scene, pointIndex);
      if (!resolved) {
        return;
      }
      point.x = resolved.x;
      point.y = resolved.y;
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function refreshTraceConstrainedPointPositions(env, scene) {
    scene.points.forEach((/** @type {RuntimeScenePointJson} */ point, /** @type {number} */ pointIndex) => {
      if (point.constraint?.kind !== "polyline" || typeof point.constraint.functionKey !== "number") {
        return;
      }
      const resolved = resolveScenePointInScene(env, scene, pointIndex);
      if (!resolved) {
        return;
      }
      point.x = resolved.x;
      point.y = resolved.y;
    });
  }

  /**
   * @param {ViewerSceneData} scene
   * @param {{ lineStartIndex?: number | null, lineEndIndex?: number | null, lineIndex?: number | null }} binding
   * @returns {[Point | null, Point | null]}
   */
  function reflectionAxisPoints(scene, binding) {
    const lineIndex = binding.lineIndex;
    if (typeof lineIndex === "number" && Number.isInteger(lineIndex)) {
      const axis = scene.lines[lineIndex];
      if (axis?.points?.length >= 2) {
        return [axis.points[0], axis.points[axis.points.length - 1]];
      }
    }
    const lineStartIndex = binding.lineStartIndex;
    const lineEndIndex = binding.lineEndIndex;
    const lineStart = typeof lineStartIndex === "number" && Number.isInteger(lineStartIndex)
      ? scene.points[lineStartIndex]
      : null;
    const lineEnd = typeof lineEndIndex === "number" && Number.isInteger(lineEndIndex)
      ? scene.points[lineEndIndex]
      : null;
    return [lineStart || null, lineEnd || null];
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   */
  function refreshDynamicLabels(env, scene) {
    const parameters = parameterMapForScene(env, scene);
    scene.labels.forEach((/** @type {RuntimeLabelJson} */ label) => {
      if (!label.binding) return;
      const refreshLabel = DYNAMIC_LABEL_REFRESHERS[label.binding.kind];
      if (refreshLabel) {
        refreshLabel(env, scene, label, parameters);
      }
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} draft
   * @param {Map<string, number>} parameters
   */
  function applyBaseDynamicUpdates(env, draft, parameters) {
    env.currentDynamics().parameters.forEach((/** @type {ParameterJson} */ parameter) => {
      if (typeof parameter.labelIndex === "number" && draft.labels[parameter.labelIndex]) {
        draft.labels[parameter.labelIndex].text =
          `${parameter.name} = ${env.formatNumber(parameter.value)}${parameterValueSuffix(parameter)}`;
      }
    });
    draft.points.forEach((/** @type {RuntimeScenePointJson} */ point) => {
      if (point.binding?.kind !== "parameter" || !point.constraint) {
        const updatePoint = point.binding ? SYNC_DYNAMIC_POINT_BINDING_UPDATERS[point.binding.kind] : null;
        if (updatePoint) {
          updatePoint(env, draft, point, parameters);
        }
        return;
      }
      const value = parameters.get(point.binding.name);
      if (!Number.isFinite(value)) return;
      applyNormalizedParameterToPoint(point, draft, value);
    });
    env.currentDynamics().functions.forEach((/** @type {FunctionJson} */ functionDef) => {
      if (typeof functionDef.labelIndex === "number" && draft.labels[functionDef.labelIndex]) {
        const variableLabel = functionDef.domain.plotMode === "polar" ? "θ" : "x";
        const head = functionDef.domain.plotMode === "polar"
          ? (functionDef.derivative ? `r'(${variableLabel})` : "r")
          : (functionDef.derivative
            ? `${functionDef.name}'(${variableLabel})`
            : `${functionDef.name}(${variableLabel})`);
        draft.labels[functionDef.labelIndex].text = `${head} = ${formatExpr(functionDef.expr, env.formatAxisNumber, variableLabel)}`;
      }
      const sampledSegments = sampleDynamicFunction(functionDef, parameters);
      const sampled = sampledSegments.flat();
      if (typeof functionDef.lineIndex === "number" && draft.lines[functionDef.lineIndex]) {
        draft.lines[functionDef.lineIndex].points = sampled.map((point) => ({ ...point }));
        draft.lines[functionDef.lineIndex].segments = sampledSegments
          .map((segment) => segment.map((point) => ({ ...point })));
      }
      functionDef.constrainedPointIndices.forEach((/** @type {number} */ pointIndex) => {
        const constraint = draft.points[pointIndex]?.constraint;
        if (constraint && constraint.kind === "polyline") {
          constraint.points = sampled.map((point) => ({ ...point }));
          constraint.segmentIndex = Math.min(constraint.segmentIndex, Math.max(0, sampled.length - 2));
        }
      });
    });
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {string[]} dirtyRootIds
   */
  function runDependencyGraph(env, scene, dirtyRootIds) {
    const graph = ensureDependencyGraph(env);
    const rootSet = new Set(
      (dirtyRootIds || []).filter((rootId) => typeof rootId === "string" && graph.nodeMap.has(rootId)),
    );
    if (rootSet.size === 0) {
      env.currentDynamics().parameters.forEach((parameter) => {
        rootSet.add(parameterRootId(parameter.name));
      });
    }
    if (rootSet.size === 0) {
      (env.sourceScene.points || []).forEach((_, index) => rootSet.add(sourcePointRootId(index)));
      (env.sourceScene.lines || []).forEach((_, index) => rootSet.add(sourceLineRootId(index)));
      (env.sourceScene.circles || []).forEach((_, index) => rootSet.add(sourceCircleRootId(index)));
      (env.sourceScene.polygons || []).forEach((_, index) => rootSet.add(sourcePolygonRootId(index)));
    }
    const affected = new Set(rootSet);
    const queue = Array.from(rootSet);
    while (queue.length > 0) {
      const currentId = /** @type {string} */ (queue.shift());
      (graph.reverseEdges.get(currentId) || []).forEach((dependentId) => {
        if (!affected.has(dependentId)) {
          affected.add(dependentId);
          queue.push(dependentId);
        }
      });
    }
    /** @type {DependencyNode[]} */
    const orderedNodes = graph.topoOrder
      .flatMap((id) => {
        const node = graph.nodeMap.get(id);
        return node && affected.has(node.id) ? [node] : [];
      });
    /** @type {string[]} */
    const executedRecipes = [];
    const seenRecipes = new Set();
    orderedNodes.forEach((node) => {
      if (!node.recipe || seenRecipes.has(node.recipe)) {
        return;
      }
      seenRecipes.add(node.recipe);
      executedRecipes.push(node.recipe);
      const runRecipe = GRAPH_RECIPES[node.recipe];
      if (runRecipe) {
        runRecipe(env, scene);
      }
    });
    if (seenRecipes.has("refresh-dynamic-labels") && (env.sourceScene.labelIterations || []).length > 0) {
      refreshIterationGeometry(env, scene, parameterMapForScene(env, scene));
      executedRecipes.push("rebuild-label-iteration-anchors");
    }
    return {
      dirtyRoots: Array.from(rootSet),
      affectedNodes: orderedNodes.map((node) => ({
        id: node.id,
        kind: node.kind,
        dependsOn: [...node.dependsOn],
        recipe: node.recipe,
      })),
      executedRecipes,
    };
  }

  /**
   * @param {ViewerEnv} env
   * @param {string[]} [dirtyParameterNames]
   */
  function syncDynamicScene(env, dirtyParameterNames) {
    const names = Array.isArray(dirtyParameterNames) && dirtyParameterNames.length > 0
      ? dirtyParameterNames
      : env.currentDynamics().parameters.map((parameter) => parameter.name);
    env.markDependencyRootsDirty?.(
      names.map((name) => parameterRootId(name)),
    );
    env.updateScene(() => {}, "graph");
  }

  /**
   * @param {ViewerEnv} env
   * @param {ViewerSceneData} scene
   * @param {Map<string, number>} parameters
   */
  function refreshIterationGeometry(env, scene, parameters) {
    rebuildIterationPoints(env, scene, parameters);
    rebuildIteratedLines(env, scene, parameters);
    rebuildIteratedPolygons(env, scene, parameters);
    rebuildIteratedLabels(env, scene, parameters);
    rebuildIterationTables(env, scene, parameters);
  }

  /** @param {ParameterJson} parameter */
  function parameterValueSuffix(parameter) {
    switch (parameter.unit) {
      case "degree":
        return "\u00B0";
      case "cm":
        return " cm";
      default:
        return "";
    }
  }

  /** @param {ViewerEnv} env */
  function buildParameterControls(env) {
    const parameterControls = env.parameterControls;
    if (!parameterControls) {
      return;
    }
    parameterControls.replaceChildren();
    const controls = env.currentDynamics().parameters
      .map((parameter, index) => ({ parameter, index }))
      .filter(({ parameter }) => parameter.visible !== false)
      .map(({ parameter, index }) => {
        const isDiscrete = isDiscreteIterationParameterName(env.sourceScene, parameter.name);
        /** @type {{ type: string; step: string; min?: string; value: string; oninput: (event: Event) => void }} */
        const inputAttrs = {
          type: "number",
          step: isDiscrete ? "1" : "0.1",
          value: env.formatNumber(parameter.value),
          oninput: (event) => {
            const target = /** @type {HTMLInputElement} */ (event.target);
            let value = Number.parseFloat(target.value);
            if (Number.isFinite(value)) {
              if (isDiscrete) {
                value = discreteIterationDepth(value);
              }
              env.updateDynamics((draft) => {
                draft.parameters[index].value = value;
              });
              syncDynamicScene(env, [parameter.name]);
            }
          },
        };
        if (isDiscrete) {
          inputAttrs.min = "0";
        }
        return env.labelTag(
          `${parameter.name} =`,
          env.inputTag(/** @type {Parameters<ViewerEnv["inputTag"]>[0]} */ (inputAttrs)),
          parameterValueSuffix(parameter),
        );
      });
    if (controls.length > 0) {
      env.van.add(parameterControls, ...controls);
    }
  }

  modules.dynamics = {
    buildParameterControls,
    evaluateExpr,
    formatExpr,
    parameterMapForScene,
    parameterValueFromPoint,
    applyNormalizedParameterToPoint,
    refreshDerivedPoints,
    refreshDynamicLabels,
    refreshIterationGeometry,
    resolveLineConstraintPoints,
    resolveLineConstraintParameterPoints,
    parameterRootId,
    sourcePointRootId,
    runDependencyGraph,
    describeDependencyGraph,
    syncDynamicScene,
  };
})();
