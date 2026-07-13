declare const van: typeof import("./vendor/van-1.6.0").default;

type PointJson = import("./generated/PointJson").PointJson;
type Point = PointJson;
type RuntimeLineKind = "segment" | "line" | "ray";
type RuntimeBounds = { minX: number; maxX: number; minY: number; maxY: number };
type RuntimeProjection = { t: number; projected: Point; distanceSquared: number };
type RuntimeArcGeometry = {
  start: Point;
  mid: Point;
  end: Point;
  center: Point;
  radius: number;
  startAngle: number;
  midAngle: number;
  endAngle: number;
  ccwSpan: number;
  ccwMid: number;
};
type BoundsJson = import("./generated/BoundsJson").BoundsJson;
type SceneData = import("./generated/SceneData").SceneData;
type ScenePointJson = import("./generated/ScenePointJson").ScenePointJson;
type PointConstraintJson = import("./generated/PointConstraintJson").PointConstraintJson;
type PointBindingJson = import("./generated/PointBindingJson").PointBindingJson;
type PointTransformJson = import("./generated/PointTransformJson").PointTransformJson;
type TransformJson = import("./generated/TransformJson").TransformJson;
type LineJson = import("./generated/LineJson").LineJson;
type LineBindingJson = import("./generated/LineBindingJson").LineBindingJson;
type PolygonJson = import("./generated/PolygonJson").PolygonJson;
type CircleJson = import("./generated/CircleJson").CircleJson;
type ArcJson = import("./generated/ArcJson").ArcJson;
type LabelJson = import("./generated/LabelJson").LabelJson;
type LabelBindingJson = import("./generated/LabelBindingJson").LabelBindingJson;
type LabelHotspotJson = import("./generated/LabelHotspotJson").LabelHotspotJson;
type LabelHotspotActionJson = import("./generated/LabelHotspotActionJson").LabelHotspotActionJson;
type ButtonJson = import("./generated/ButtonJson").ButtonJson;
type ButtonActionJson = import("./generated/ButtonActionJson").ButtonActionJson;
type PointAnimationJson = import("./generated/PointAnimationJson").PointAnimationJson;
type AnimatedPointTargetJson = import("./generated/AnimatedPointTargetJson").AnimatedPointTargetJson;
type ImageJson = import("./generated/ImageJson").ImageJson;
type IterationTableJson = import("./generated/IterationTableJson").IterationTableJson;
type ParameterJson = import("./generated/ParameterJson").ParameterJson;
type FunctionJson = import("./generated/FunctionJson").FunctionJson;
type FunctionExprJson = import("./generated/FunctionExprJson").FunctionExprJson;
type FunctionAstJson = import("./generated/FunctionAstJson").FunctionAstJson;
type PointIterationJson = import("./generated/PointIterationJson").PointIterationJson;
type LineIterationJson = import("./generated/LineIterationJson").LineIterationJson;
type PolygonIterationJson = import("./generated/PolygonIterationJson").PolygonIterationJson;
type LabelIterationJson = import("./generated/LabelIterationJson").LabelIterationJson;
type CircleIterationJson = import("./generated/CircleIterationJson").CircleIterationJson;
type LineConstraintJson = import("./generated/LineConstraintJson").LineConstraintJson;
type CircularConstraintJson = import("./generated/CircularConstraintJson").CircularConstraintJson;
type ArcBoundaryKind = import("./generated/ArcBoundaryKindJson").ArcBoundaryKindJson;
type CoordinateAxisJson = import("./generated/CoordinateAxisJson").CoordinateAxisJson;
type ShapeBindingJson = import("./generated/ShapeBindingJson").ShapeBindingJson;
type ColorBindingJson = import("./generated/ColorBindingJson").ColorBindingJson;
type RichTextExpressionRefJson = import("./generated/RichTextExpressionRefJson").RichTextExpressionRefJson;
type IterationPointHandleJson = import("./generated/IterationPointHandleJson").IterationPointHandleJson;
type DomainJson = import("./generated/DomainJson").DomainJson;
type DebugSourceJson = import("./generated/DebugSourceJson").DebugSourceJson;

type RuntimeJsonPrimitive = string | number | boolean | null;
type RuntimeJsonValue =
  | RuntimeJsonPrimitive
  | RuntimeJsonValue[]
  | { [key: string]: RuntimeJsonValue };
type HostLineBinding = {
  lineStartIndex?: number | null;
  lineEndIndex?: number | null;
  lineIndex?: number | null;
};
type VisibilityTarget =
  { visible: boolean };

type RuntimePointRef =
  | Point
  | {
      pointIndex: number;
      dx?: number;
      dy?: number;
      x?: number;
      y?: number;
    }
  | {
      lineIndex: number;
      pointIndex?: number;
      segmentIndex?: number;
      t?: number;
      dx?: number;
      dy?: number;
      x?: number;
      y?: number;
    };

type PointHandle = RuntimePointRef;

type UnionKeys<T> = T extends unknown ? keyof T : never;
type StrictUnion<T, All = T> = T extends unknown
  ? T & Partial<Record<Exclude<UnionKeys<All>, keyof T>, never>>
  : never;

type RuntimePointBindingJson = StrictUnion<PointBindingJson | {
  kind: "rotate";
  sourceIndex: number;
  centerIndex: number;
  angleDegrees: number;
}>;

type RuntimeLabelBindingJson = StrictUnion<LabelBindingJson>;

type RuntimeLineBindingJson = StrictUnion<
  | Exclude<LineBindingJson, { kind: "point-trace" }>
  | (Extract<LineBindingJson, { kind: "point-trace" }> & { useMidpoints?: boolean })
>;

type RuntimeShapeBindingJson = StrictUnion<ShapeBindingJson>;

type RuntimePolylineConstraintJson = Omit<
  Extract<PointConstraintJson, { kind: "polyline" }>,
  "points"
> & {
  points: PointHandle[];
};
type RuntimePointConstraintJson = StrictUnion<
  | Exclude<PointConstraintJson, { kind: "polyline" }>
  | RuntimePolylineConstraintJson
>;
type RuntimeScenePointJson = Omit<ScenePointJson, "constraint" | "binding"> & {
  constraint: RuntimePointConstraintJson | null;
  binding: RuntimePointBindingJson | null;
};
type RuntimeLineJson = Omit<LineJson, "points" | "segments" | "binding"> & {
  points: PointHandle[];
  segments: Point[][] | null;
  binding: RuntimeLineBindingJson | null;
};
type RuntimePolygonJson = Omit<PolygonJson, "points" | "binding"> & {
  points: PointHandle[];
  binding: RuntimeShapeBindingJson | null;
};
type RuntimeCircleJson = Omit<CircleJson, "center" | "radiusPoint" | "binding"> & {
  center: PointHandle;
  radiusPoint: PointHandle;
  binding: RuntimeShapeBindingJson | null;
};
type RuntimeArcJson = Omit<ArcJson, "points" | "center"> & {
  points: PointHandle[];
  center: PointHandle | null;
};
type RuntimeLabelHotspotJson = Omit<LabelHotspotJson, "action"> & {
  action: LabelHotspotActionJson;
};
type RuntimeLabelJson = Omit<LabelJson, "anchor" | "binding" | "hotspots"> & {
  anchor: PointHandle;
  binding: RuntimeLabelBindingJson | null;
  centeredOnAnchor: boolean;
  hotspots: RuntimeLabelHotspotJson[];
};
type TextLabel = RuntimeLabelJson;

type RuntimeIterationRow = {
  index: number;
  value: number;
  values: number[];
};

type RuntimeIterationTableJson = IterationTableJson & {
  rows: RuntimeIterationRow[];
};
type RuntimeButtonJson = ButtonJson & {
  baseText: string;
  active: boolean;
};

type DebugTarget = {
  category: string;
  index: number;
  hotspotIndex?: number | null;
  label?: string | null;
};

type SceneLabelJson = RuntimeLabelJson;
type SceneLineJson = RuntimeLineJson;
type ScenePolygonJson = RuntimePolygonJson;
type SceneIterationTableJson = RuntimeIterationTableJson;

type RuntimeDynamicsState = {
  parameters: ParameterJson[];
  functions: FunctionJson[];
};

type RuntimeFunctionJson = FunctionJson;

type RuntimePointIterationFamily = PointIterationJson;
type RuntimeLineIterationFamily = LineIterationJson;
type RuntimePolygonIterationFamily = PolygonIterationJson;
type RuntimeLabelIterationFamily = LabelIterationJson;
type RuntimeCircleIterationFamily = CircleIterationJson;

type ViewerSceneResolverEnv = {
  sourceScene: SceneData | ViewerSceneData;
  currentScene?: () => ViewerSceneData;
  resolveScenePoint?: (index: number) => Point | null;
} | ViewerEnv;

type ViewerSceneData = Omit<
  SceneData,
  | "origin"
  | "lines"
  | "polygons"
  | "circles"
  | "arcs"
  | "labels"
  | "points"
  | "iterationTables"
  | "buttons"
> & {
  origin: RuntimePointRef | null;
  lines: RuntimeLineJson[];
  polygons: RuntimePolygonJson[];
  circles: RuntimeCircleJson[];
  arcs: RuntimeArcJson[];
  labels: RuntimeLabelJson[];
  points: RuntimeScenePointJson[];
  iterationTables: RuntimeIterationTableJson[];
  buttons: RuntimeButtonJson[];
};

type ViewState = {
  centerX: number;
  centerY: number;
  zoom: number;
};

type DragState = {
  pointerId: number;
  mode: string;
  pointIndex: number | null;
  labelIndex: number | null;
  polygonIndex: number | null;
  iterationTableIndex: number | null;
  imageIndex: number | null;
  lastX: number;
  lastY: number;
} | null;

type HotspotFlash = {
  key: string;
  action: LabelHotspotActionJson;
};

type PointConstraintParameterReader = (
  scene: ViewerSceneData,
  pointIndex: number,
) => number | null;

type PointConstraintParameterApplier = (
  point: RuntimeScenePointJson,
  scene: ViewerSceneData,
  wrapped: number,
) => void;

type PointBindingRefresher = (
  env: ViewerEnv,
  scene: ViewerSceneData,
  point: RuntimeScenePointJson,
  parameters: Map<string, number>,
) => void;

type DynamicLabelRefresher = (
  env: ViewerEnv,
  scene: ViewerSceneData,
  label: RuntimeLabelJson,
  parameters: Map<string, number>,
) => void;

type LineBindingRefreshContext = {
  env: ViewerEnv;
  scene: ViewerSceneData;
  bounds: {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
    spanX?: number;
    spanY?: number;
  };
  parameters: Map<string, number>;
};

type LineBindingRefresher = (
  ctx: LineBindingRefreshContext,
  line: RuntimeLineJson,
) => void;

type CircleBindingRefreshContext = {
  env: ViewerEnv;
  scene: ViewerSceneData;
  parameters: Map<string, number>;
  resolveHandle: (handle: RuntimePointRef) => Point | null;
};

type CircleBindingRefresher = (
  ctx: CircleBindingRefreshContext,
  circle: RuntimeCircleJson,
) => void;

type PolygonBindingRefreshContext = {
  env: ViewerEnv;
  scene: ViewerSceneData;
  parameters: Map<string, number>;
  resolveHandle: (handle: RuntimePointRef) => Point | null;
};

type PolygonBindingRefresher = (
  ctx: PolygonBindingRefreshContext,
  polygon: RuntimePolygonJson,
) => void;

type ViewerEnv = {
  canvas: SVGSVGElement;
  svg: SVGSVGElement;
  gridLayer: SVGGElement;
  sceneLayer: SVGGElement;
  sourceScene: SceneData;
  margin: number;
  trigMode: boolean;
  savedViewportMode: boolean;
  baseSpanX: number;
  baseSpanY: number;
  pointHitRadius: number;
  hoverPointIndex: { val: number | null };
  dragState: { val: DragState };
  view: ViewState;
  currentScene: () => ViewerSceneData;
  currentDynamics: () => RuntimeDynamicsState;
  currentHotspotFlashes: () => HotspotFlash[];
  resolveScenePoint: (index: number) => Point | null;
  resolvePoint: (handle: RuntimePointRef) => Point | null;
  resolveAnchorBase: (handle: RuntimePointRef) => Point | null;
  resolveLinePoints: (lineOrIndex: RuntimeLineJson | number | null | undefined) => Point[] | null;
  toScreen: (point: Point) => Point & { scale: number };
  toWorld: (x: number, y: number) => Point & { scale: number };
  getViewBounds: () => BoundsJson & { spanX: number; spanY: number };
  rgba: (color: [number, number, number, number] | number[] | null | undefined) => string;
  updateScene: (mutator: (draft: ViewerSceneData) => void, mode?: "graph" | "none") => void;
  updateDynamics: (mutator: (draft: RuntimeDynamicsState) => void) => void;
  updateViewState: (mutator: (draft: ViewState) => void) => void;
  syncDynamicScene: () => void;
  isOriginPointIndex: (index: number) => boolean;
  formatNumber: (value: number) => string;
  formatAxisNumber: (value: number) => string;
  formatPiLabel: (stepIndex: number) => string;
  drawGrid: () => void;
  createSvgElement: (
    name: string,
    attrs?: Record<string, string | number | boolean | null | undefined>,
  ) => SVGElement;
  setSvgAttributes: (
    element: Element,
    attrs: Record<string, string | number | boolean | null | undefined>,
  ) => void;
  clearSvgChildren: (element: Element) => void;
  measureText: (text: string, fontSize?: number, fontWeight?: number | string) => number;
  registerDebugElement?: (element: Element, target: DebugTarget | null | undefined) => void;
  selectDebugTarget?: (target: DebugTarget) => void;
  markDependencyRootsDirty?: (rootIds: string | string[]) => void;
  inputTag: typeof import("./vendor/van-1.6.0").default.tags.input;
  labelTag: typeof import("./vendor/van-1.6.0").default.tags.label;
  parameterControls: HTMLElement | null;
  van: typeof import("./vendor/van-1.6.0").default;
};

type ViewerSceneModule = {
  resolveAngleMarkerPoints: (
    start: Point,
    vertex: Point,
    end: Point,
    markerClass: number,
  ) => Point[] | null;
  registerPointConstraintResolver: <K extends RuntimePointConstraintJson["kind"]>(
    kind: K,
    resolver: (
      env: ViewerSceneResolverEnv | null,
      constraint: Extract<RuntimePointConstraintJson, { kind: K }>,
      resolveFn: (index: number) => Point | null,
      reference?: RuntimeScenePointJson | Point | null,
    ) => Point | null,
  ) => void;
  registerLineBindingResolver: <K extends LineBindingJson["kind"]>(
    kind: K,
    resolver: (env: ViewerEnv, line: RuntimeLineJson & { binding: Extract<LineBindingJson, { kind: K }> }) => Point[] | null,
  ) => void;
  resolveConstrainedPoint: (
    env: ViewerSceneResolverEnv | null,
    constraint: RuntimePointConstraintJson | null,
    resolveFn: (index: number) => Point | null,
    reference?: RuntimeScenePointJson | Point | null,
  ) => Point | null;
  resolveScenePoint: (env: ViewerEnv, index: number) => Point | null;
  resolvePoint: (env: ViewerEnv, handle: RuntimePointRef) => Point | null;
  resolveAnchorBase: (env: ViewerEnv, handle: RuntimePointRef) => Point | null;
  resolveLinePoints: (env: ViewerEnv, lineOrIndex: RuntimeLineJson | number | null | undefined) => Point[] | null;
  toScreen: (env: ViewerEnv, point: Point) => Point & { scale: number };
  toWorld: (env: ViewerEnv, x: number, y: number) => Point & { scale: number };
  getViewBounds: (env: ViewerEnv) => ViewerEnv["getViewBounds"] extends () => infer T ? T : never;
  getCanvasCoords: (env: ViewerEnv, event: MouseEvent | PointerEvent | WheelEvent) => Point;
  chooseGridStep: (span: number, targetLines: number) => number;
  lerpPoint: (start: Point, end: Point, t: number) => Point;
  projectToSegment: (
    point: Point,
    start: Point,
    end: Point,
  ) => { t: number; projected: Point; distanceSquared: number } | null;
  projectToLineLike: (
    point: Point,
    start: Point,
    end: Point,
    kind: "segment" | "line" | "ray",
  ) => { t: number; projected: Point; distanceSquared: number } | null;
  pointOnCircleArc: (center: Point, start: Point, end: Point, t: number, yUp: boolean) => Point | null;
  projectToCircleArc: (
    point: Point,
    center: Point,
    start: Point,
    end: Point,
    yUp: boolean,
  ) => { t: number; projected: Point; distanceSquared: number } | null;
  pointOnThreePointArc: (start: Point, mid: Point, end: Point, t: number) => Point | null;
  projectToThreePointArc: (
    point: Point,
    start: Point,
    mid: Point,
    end: Point,
  ) => { t: number; projected: Point; distanceSquared: number } | null;
  sampleArcBoundaryPoints: (
    env: ViewerEnv,
    binding:
      | Extract<RuntimeLineBindingJson, { kind: "arc-boundary" }>
      | Extract<RuntimeShapeBindingJson, { kind: "arc-boundary-polygon" }>,
  ) => Point[] | null;
  sampleCoordinateTracePoints: (
    env: ViewerEnv | null,
    binding: RuntimeLineBindingJson | RuntimePointConstraintJson,
  ) => Point[] | null;
  lineLineIntersection: (
    leftStart: Point,
    leftEnd: Point,
    leftKind: RuntimeLineKind,
    rightStart: Point,
    rightEnd: Point,
    rightKind: RuntimeLineKind,
  ) => Point | null;
  lineCircleIntersection: (
    lineStart: Point,
    lineEnd: Point,
    lineKind: RuntimeLineKind,
    center: Point,
    radiusPoint: Point,
    variant: number,
    reference?: Point | RuntimeScenePointJson | null,
  ) => Point | null;
  circleCircleIntersection: (
    leftCenter: Point,
    leftRadiusPoint: Point,
    rightCenter: Point,
    rightRadiusPoint: Point,
    variant: number,
    reference?: Point | RuntimeScenePointJson | null,
  ) => Point | null;
  _circleFromConstraint?: (
    env: ViewerEnv | null,
    constraint: CircularConstraintJson | null,
    resolveFn: (index: number) => Point | null,
  ) => { kind: string; center: Point; radius: number } | null;
  _pointLiesOnCircularConstraint?: (
    point: Point,
    constraint: { kind: string; center?: Point; radius?: number } | null,
  ) => boolean;
  _threePointArcGeometry?: (
    start: Point,
    mid: Point,
    end: Point,
  ) => Record<string, unknown> | null;
  _circleArcControlPoints?: (
    center: Point,
    start: Point,
    end: Point,
    yUp: boolean,
  ) => { start: Point; mid: Point; end: Point } | null;
  _pointOnThreePointArcComplement?: (
    start: Point,
    mid: Point,
    end: Point,
    t: number,
  ) => Point | null;
  drawGrid: (env: ViewerEnv) => void;
};

type ViewerGeometryModule = {
  normalizeAngleDelta: (from: number, to: number) => number;
  lerpPoint: (start: Point, end: Point, t: number) => Point;
  rotateAround: (point: Point, center: Point, radians: number) => Point;
  scaleAround: (point: Point, center: Point, factor: number) => Point;
  reflectAcrossLine: (point: Point, lineStart: Point, lineEnd: Point) => Point | null;
  clipParametricLineToBounds: (
    start: Point,
    end: Point,
    bounds: { minX: number; maxX: number; minY: number; maxY: number },
    rayOnly: boolean,
  ) => Point[] | null;
  clipLineToBounds: (
    start: Point,
    end: Point,
    bounds: { minX: number; maxX: number; minY: number; maxY: number },
  ) => Point[] | null;
  clipRayToBounds: (
    start: Point,
    end: Point,
    bounds: { minX: number; maxX: number; minY: number; maxY: number },
  ) => Point[] | null;
  angleBisectorDirection: (start: Point, vertex: Point, end: Point) => Point | null;
  measuredRotationRadians: (start: Point, vertex: Point, end: Point) => number | null;
  scaleByThreePointRatio: (
    source: Point,
    center: Point,
    ratioOrigin: Point,
    ratioDenominator: Point,
    ratioNumerator: Point,
    signed?: boolean,
    clampToUnit?: boolean,
  ) => Point | null;
};

type ViewerRenderModule = {
  labelMetrics: (env: ViewerEnv, text: string) => { lines: string[]; width: number; height: number };
  drawImages: (env: ViewerEnv) => void;
  drawPolygons: (env: ViewerEnv) => void;
  drawLines: (env: ViewerEnv) => void;
  drawCircles: (env: ViewerEnv) => void;
  drawArcs: (env: ViewerEnv) => void;
  drawPoints: (env: ViewerEnv) => void;
  drawLabels: (env: ViewerEnv) => void;
  drawIterationTables: (env: ViewerEnv) => void;
  drawHotspotFlashes: (env: ViewerEnv) => void;
  draw: (env: ViewerEnv) => void;
  pathFromPoints: (points: Point[], close?: boolean) => string;
  arcPath: (
    center: Point,
    radius: number,
    startAngle: number,
    endAngle: number,
    counterClockwise: boolean,
  ) => string;
  appendSceneElement: (
    env: ViewerEnv,
    tag: string,
    attrs: Record<string, string | number | boolean | null | undefined>,
    text?: string | null,
    debugTarget?: DebugTarget | null,
  ) => SVGElement;
  appendPointPath: (
    env: ViewerEnv,
    points: Point[],
    options: {
      stroke: string;
      strokeWidth?: number;
      fill?: string;
      dashed?: boolean;
      close?: boolean;
      lineCap?: string;
      lineJoin?: string;
      debugTarget?: DebugTarget | null;
    },
  ) => SVGElement | null;
  labelHotspotRects: (
    env: ViewerEnv,
    label: RuntimeLabelJson,
  ) => Array<{
    line: number;
    start: number;
    end: number;
    text: string;
    left: number;
    top: number;
    width: number;
    height: number;
    action: LabelHotspotActionJson | null;
    hotspotIndex?: number;
  }>;
  findHitPoint: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitImage?: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitLabel: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitIterationTable: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitPolygon: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  iterationTableBounds: (
    env: ViewerEnv,
    table: RuntimeIterationTableJson,
  ) => {
    left: number;
    top: number;
    width: number;
    height: number;
    rows: string[][];
    colWidths: number[];
    rowHeight: number;
  } | null;
  labelBounds: (
    env: ViewerEnv,
    label: RuntimeLabelJson,
  ) => {
    screen: Point;
    lines: string[];
    width: number;
    height: number;
    left: number;
    top: number;
  } | null;
};

type ViewerDragModule = {
  dragModeFor: (
    env: ViewerEnv,
    pointIndex: number | null,
    labelIndex: number | null,
    polygonIndex: number | null,
    iterationTableIndex: number | null,
    imageIndex: number | null,
  ) => string;
  beginDrag: (
    env: ViewerEnv,
    pointerId: number,
    position: Point,
    pointIndex: number | null,
    labelIndex: number | null,
    polygonIndex: number | null,
    iterationTableIndex: number | null,
    imageIndex: number | null,
  ) => void;
  updateDraggedPoint: (env: ViewerEnv, world: Point) => void;
  updateDraggedLabel: (env: ViewerEnv, world: Point) => void;
  updateDraggedImage: (env: ViewerEnv, position: Point) => void;
  updateDraggedPolygon: (env: ViewerEnv, world: Point) => void;
  updateDraggedIterationTable: (env: ViewerEnv, world: Point) => void;
  panFromPointerDelta: (env: ViewerEnv, position: Point) => void;
};

type ViewerDynamicsModule = {
  buildParameterControls: (env: ViewerEnv) => void;
  evaluateExpr: ((expr: FunctionExprJson, x: number, parameters: Map<string, number>) => number | null) | null;
  parameterValueFromPoint: ((scene: ViewerSceneData, pointIndex: number) => number | null) | null;
  applyNormalizedParameterToPoint: (
    point: RuntimeScenePointJson,
    scene: ViewerSceneData,
    normalizedValue: number,
  ) => void;
  parameterMapForScene?: (env: ViewerEnv, scene: ViewerSceneData) => Map<string, number>;
  refreshDerivedPoints: (env: ViewerEnv, scene: ViewerSceneData) => void;
  refreshIterationGeometry: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
  refreshDynamicLabels: (env: ViewerEnv, scene: ViewerSceneData) => void;
  resolveLineConstraintPoints: (
    resolvePointAt: (pointIndex: number) => Point | null,
    bounds: { minX: number; maxX: number; minY: number; maxY: number; spanX?: number; spanY?: number },
    constraint: LineConstraintJson,
  ) => Point[] | null;
  resolveLineConstraintParameterPoints: (
    resolvePointAt: (pointIndex: number) => Point | null,
    constraint: LineConstraintJson,
  ) => Point[] | null;
  parameterRootId?: (name: string) => string;
  sourcePointRootId?: (index: number) => string;
  runDependencyGraph?: (env: ViewerEnv, scene: ViewerSceneData, dirtyRootIds: string[]) => unknown;
  describeDependencyGraph?: (env: ViewerEnv) => unknown[];
  syncDynamicScene: (env: ViewerEnv, dirtyParameterNames?: string[]) => void;
};

type ViewerDynamicsExpressionModule = {
  evaluateExpr: (expr: FunctionExprJson | FunctionAstJson, x: number, parameters: Map<string, number>) => number | null;
};

type ViewerDynamicsRichTextModule = {
  buildExpressionRichMarkup: (exprLabel: string, valueText: string) => string | null;
  buildRatioValueRichMarkup: (name: string, valueText: string) => string | null;
  buildPlainTextRichMarkup: (text: string) => string | null;
  replaceRichMarkupPathValues: (markup: string | null | undefined, valuesBySlot: Map<number, string>) => string | null;
  replaceTemplateTextRanges: (
    templateText: string,
    replacements: Array<{ line: number; start: number; end: number; valueText: string }>,
  ) => string;
};

type RuntimeDynamicsParameterDependencies = {
  discreteIterationDepth: (value: number | null | undefined) => number;
  evaluateExpr: ViewerDynamicsExpressionModule["evaluateExpr"];
  isDiscreteIterationParameterName: (
    scene: ViewerSceneData | SceneData | null | undefined,
    name: string,
  ) => boolean;
  labelParameterValueFromBinding: (scene: ViewerSceneData, binding: LabelBindingJson) => number | null;
  pointAngleValue: (
    scene: ViewerSceneData,
    binding: Extract<LabelBindingJson, { kind: "point-angle-value" }>,
  ) => number;
  pointDistanceRatioValue: (
    scene: ViewerSceneData,
    binding: Extract<LabelBindingJson, { kind: "point-distance-ratio-value" }>,
  ) => number | null;
  pointDistanceValue: (
    scene: ViewerSceneData,
    binding: Extract<LabelBindingJson, { kind: "point-distance-value" }>,
  ) => number;
  pointIterationDepth: (
    family: {
      depth: number;
      parameterName?: string | null;
      depthParameterName?: string | null;
      depthExpr?: FunctionExprJson | null;
    },
    parameters: Map<string, number>,
  ) => number;
  polygonAreaValue: (
    scene: ViewerSceneData,
    binding: Extract<LabelBindingJson, { kind: "polygon-area-value" }>,
  ) => number;
};

type ViewerDynamicsParametersModule = {
  createDynamicsParameters: (dependencies: RuntimeDynamicsParameterDependencies) => {
    deriveExpressionLabelParameters: (
      scene: ViewerSceneData | null | undefined,
      parameters: Map<string, number>,
    ) => Map<string, number>;
    deriveLabelParameters: (
      scene: ViewerSceneData | null | undefined,
      parameters: Map<string, number>,
    ) => Map<string, number>;
    parameterMapForScene: (env: ViewerEnv, scene: ViewerSceneData) => Map<string, number>;
  };
};

type RuntimeDynamicsGeometryDependencies = {
  applyTraceValueToPoint: (
    point: RuntimeScenePointJson,
    scene: ViewerSceneData,
    value: number | null | undefined,
    xMin: number,
    xMax: number,
  ) => void;
  circumcenter: (start: Point, mid: Point, end: Point) => Point | null;
  clipRayToBounds: (start: Point, end: Point, bounds: RuntimeBounds) => Point[] | null;
  deriveLabelParameters: (
    scene: ViewerSceneData | null | undefined,
    parameters: Map<string, number>,
  ) => Map<string, number>;
  discreteIterationDepth: (value: number | null | undefined) => number;
  evaluateExpr: ViewerDynamicsExpressionModule["evaluateExpr"];
  hsbToRgba: (
    hue: number,
    saturation: number,
    brightness: number,
    alpha: number,
  ) => [number, number, number, number];
  isFiniteNumber: (value: unknown) => value is number;
  lerpPoint: (start: Point, end: Point, t: number) => Point;
  lineProjectionParameterFromPoints: (
    point: Point | null | undefined,
    start: Point | null | undefined,
    end: Point | null | undefined,
    lineKind?: RuntimeLineKind,
  ) => number | null;
  parameterValueFromPoint: (scene: ViewerSceneData, pointIndex: number) => number | null;
  pointOnPolylineByIndex: (points: Point[], normalized: number) => Point | null;
  polylineParameterFromPoint: (scene: ViewerSceneData, pointIndex: number) => number | null;
  reflectAcrossLine: (point: Point, lineStart: Point, lineEnd: Point) => Point | null;
  resolveLineConstraintPoints: (
    resolvePointAt: (pointIndex: number) => Point | null,
    bounds: RuntimeBounds,
    constraint: LineConstraintJson,
  ) => Point[] | null;
  resolveRotateTransformAngleDegrees: (
    transform:
      | Extract<TransformJson, { kind: "rotate" }>
      | Extract<PointTransformJson, { kind: "rotate" }>,
    parameters: Map<string, number>,
    resolvePoint: (index: number) => Point | null | undefined,
  ) => number | null | undefined;
  resolveScaleTransformFactor: (
    transform:
      | Extract<TransformJson, { kind: "scale" }>
      | Extract<PointTransformJson, { kind: "scale" }>,
    parameters: Map<string, number>,
    resolvePoint?: ((index: number) => Point | null | undefined) | null,
  ) => number | null | undefined;
  rotateAround: (point: Point, center: Point, radians: number) => Point;
  scaleAround: (point: Point, center: Point, factor: number) => Point;
  scaleByThreePointRatio: (
    source: Point,
    center: Point,
    ratioOrigin: Point,
    ratioDenominator: Point,
    ratioNumerator: Point,
    signed: boolean,
    clampToUnit: boolean,
  ) => Point | null;
  updateConstraintParameterizedPoint: (
    point: RuntimeScenePointJson,
    scene: ViewerSceneData,
    value: number,
  ) => void;
  updateCustomTransformPoint: (
    point: RuntimeScenePointJson,
    parameters: Map<string, number>,
    resolvePointAt: (pointIndex: number) => Point | null,
    parameterSourceScene: ViewerSceneData,
  ) => void;
};

type ViewerDynamicsGeometryModule = {
  createDynamicsGeometry: (dependencies: RuntimeDynamicsGeometryDependencies) => {
    resolveHostLinePoints: (scene: ViewerSceneData, binding: HostLineBinding) => PointHandle[] | null;
    sampleCustomTransformTraceLine: (
      scene: ViewerSceneData,
      line: RuntimeLineJson,
      parameters: Map<string, number>,
    ) => Point[] | null;
    cloneTracePoint: <T extends Point>(point: T) => T;
    samplePointTraceTargets: (
      scene: ViewerSceneData,
      line: RuntimeLineJson,
      parameters: Map<string, number>,
      targetPointIndices: number[],
    ) => Point[][] | null;
    samplePointTraceLine: (
      scene: ViewerSceneData,
      line: RuntimeLineJson,
      parameters: Map<string, number>,
    ) => Point[] | null;
    refreshDerivedLine: LineBindingRefresher;
    refreshColorizedSpectrumLine: LineBindingRefresher;
    refreshDerivedPolygon: (
      env: CircleBindingRefreshContext,
      polygon: RuntimePolygonJson,
    ) => void;
    refreshDerivedCircle: CircleBindingRefresher;
  };
};

type ViewerDynamicsIterationsModule = {
  createDynamicsIterations: (dependencies: Record<string, any>) => {
    rebuildIterationPoints: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
    rebuildIteratedLines: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
    rebuildIteratedPolygons: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
    rebuildIteratedLabels: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
    rebuildIterationTables: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
  };
};

type ViewerDynamicsDependencyGraphModule = {
  createDependencyGraphRuntime: (dependencies: Record<string, Function>) => {
    parameterRootId: (name: string) => string;
    sourcePointRootId: (index: number) => string;
    describeDependencyGraph: (env: ViewerEnv) => unknown[];
    runDependencyGraph: (env: ViewerEnv, scene: ViewerSceneData, dirtyRootIds: string[]) => unknown;
  };
};

type ViewerDynamicsDependenciesModule = {
  createPointDependencyOrder: (sourceScene: SceneData | ViewerSceneData) => number[];
};

type DocumentScenePage = { index: number; title: string; scene: SceneData };
type DocumentSceneData = { kind: "gsp-document"; pages: DocumentScenePage[] };
type ViewerAppDocumentModule = {
  readSceneData: (element: HTMLElement | null) => {
    raw: unknown;
    pages: DocumentScenePage[] | null;
    activePageIndex: number;
    sourceScene: SceneData;
  };
  installPageNavigation: (
    pages: DocumentScenePage[] | null,
    activePageIndex: number,
    buttons: HTMLButtonElement[],
  ) => void;
};

type ViewerAppDebugGraphModule = {
  createDebugGraphRuntime: (dependencies: {
    formatNumber: (value: number) => string;
  }) => {
    collectReferenceTokens: (value: unknown) => string[];
    summarizeDebugEntity: (entity: unknown) => string;
    buildDebugGraph: (scene: ViewerSceneData) => string;
  };
};

type ViewerOverlayRuntime = {
  currentButtons: () => RuntimeButtonJson[];
  currentHotspotFlashes: () => HotspotFlash[];
  render: () => void;
};

type ViewerOverlayModule = {
  init: (env: ViewerEnv, buttonOverlays: HTMLElement | null) => ViewerOverlayRuntime;
};

type ViewerModules = {
  geometry: ViewerGeometryModule;
  scene: ViewerSceneModule;
  render: ViewerRenderModule;
  overlay: ViewerOverlayModule;
  drag: ViewerDragModule;
  dynamicsExpression: ViewerDynamicsExpressionModule;
  dynamicsRichText: ViewerDynamicsRichTextModule;
  dynamicsParameters: ViewerDynamicsParametersModule;
  dynamicsGeometry: ViewerDynamicsGeometryModule;
  dynamicsIterations: ViewerDynamicsIterationsModule;
  dynamicsDependencies: ViewerDynamicsDependenciesModule;
  dynamicsDependencyGraph: ViewerDynamicsDependencyGraphModule;
  appDocument: ViewerAppDocumentModule;
  appDebugGraph: ViewerAppDebugGraphModule;
  dynamics: ViewerDynamicsModule;
};

interface Window {
  GspRuntimeCore: {
    createDependencyPlan: (nodes: Array<{ id: string; dependsOn: string[] }>) => {
      topoOrder: number[];
      affected: (dirtyRootIds: string[]) => number[];
    };
    normalizeAngleDelta: (from: number, to: number) => number;
    lerpPoint: (start: Point, end: Point, t: number) => Point;
    rotateAround: (point: Point, center: Point, radians: number) => Point;
    scaleAround: (point: Point, center: Point, factor: number) => Point;
    reflectAcrossLine: (point: Point, lineStart: Point, lineEnd: Point) => Point | null;
    projectToLineLike: (point: Point, start: Point, end: Point, kind: RuntimeLineKind) => RuntimeProjection | null;
    angleBisectorDirection: (start: Point, vertex: Point, end: Point) => Point | null;
    measuredRotationRadians: (start: Point, vertex: Point, end: Point) => number | null;
    scaleByThreePointRatio: (
      source: Point,
      center: Point,
      ratioOrigin: Point,
      ratioDenominator: Point,
      ratioNumerator: Point,
      signed: boolean,
      clampToUnit: boolean,
    ) => Point | null;
    clipLineToBounds: (start: Point, end: Point, bounds: RuntimeBounds) => Point[] | null;
    clipRayToBounds: (start: Point, end: Point, bounds: RuntimeBounds) => Point[] | null;
    threePointArcGeometry: (start: Point, mid: Point, end: Point) => RuntimeArcGeometry | null;
    pointOnThreePointArc: (start: Point, mid: Point, end: Point, t: number, complement: boolean) => Point | null;
    circleArcControlPoints: (center: Point, start: Point, end: Point, yUp: boolean) => [Point, Point, Point] | null;
    pointOnCircleArc: (center: Point, start: Point, end: Point, t: number, yUp: boolean) => Point | null;
    projectToThreePointArc: (point: Point, start: Point, mid: Point, end: Point) => RuntimeProjection | null;
    projectToCircleArc: (point: Point, center: Point, start: Point, end: Point, yUp: boolean) => RuntimeProjection | null;
    lineLineIntersection: (
      leftStart: Point,
      leftEnd: Point,
      leftKind: RuntimeLineKind,
      rightStart: Point,
      rightEnd: Point,
      rightKind: RuntimeLineKind,
    ) => Point | null;
    lineCircleIntersections: (
      start: Point,
      end: Point,
      lineKind: RuntimeLineKind,
      center: Point,
      radius: number,
    ) => Point[];
    circleCircleIntersections: (
      leftCenter: Point,
      leftRadius: number,
      rightCenter: Point,
      rightRadius: number,
    ) => Point[];
    pointCircleTangents: (point: Point, center: Point, radius: number) => Point[];
    resolvePointConstraints: (
      points: RuntimeScenePointJson[],
      pointOrder: number[],
      yUp: boolean,
      parameters: Map<string, number>,
    ) => Array<Point | null>;
    inversePointTransform: (
      world: Point,
      transform: PointTransformJson,
      points: RuntimeScenePointJson[],
      parameters: Map<string, number>,
    ) => Point | null;
    transformPoints: (
      points: Point[],
      transform: TransformJson,
      scene: ViewerSceneData,
      parameters: Map<string, number>,
    ) => Point[] | null;
    sampleFunction: (
      expr: FunctionExprJson | FunctionAstJson,
      parameters: Map<string, number>,
      xMin: number,
      xMax: number,
      sampleCount: number,
      plotMode: "cartesian" | "polar",
    ) => Point[][];
    sampleParametricCurve: (
      xExpr: FunctionExprJson | FunctionAstJson,
      yExpr: FunctionExprJson | FunctionAstJson,
      parameters: Map<string, number>,
      valueMin: number,
      valueMax: number,
      sampleCount: number,
    ) => Point[];
    sampleCoordinateTrace: (
      xExpr: FunctionExprJson | FunctionAstJson,
      yExpr: FunctionExprJson | FunctionAstJson | null,
      parameters: Map<string, number>,
      xParameterName: string | null,
      yParameterName: string | null,
      source: Point,
      valueMin: number,
      valueMax: number,
      sampleCount: number,
      useMidpoints: boolean,
      mode: "horizontal" | "vertical" | "two-dimensional",
    ) => Point[];
    sampleCustomTransformTrace: (
      distanceExpr: FunctionExprJson | FunctionAstJson,
      angleExpr: FunctionExprJson | FunctionAstJson,
      parameters: Map<string, number>,
      origin: Point,
      axisEnd: Point,
      valueMin: number,
      valueMax: number,
      traceMax: number,
      sampleCount: number,
      distanceScale: number,
      angleDegreesScale: number,
    ) => Point[];
    customTransformPoint: (
      distanceExpr: FunctionExprJson | FunctionAstJson,
      angleExpr: FunctionExprJson | FunctionAstJson,
      parameters: Map<string, number>,
      origin: Point,
      axisEnd: Point,
      value: number,
      distanceScale: number,
      angleDegreesScale: number,
    ) => Point | null;
    sampleCircleArc: (center: Point, start: Point, end: Point, steps: number, yUp: boolean) => Point[] | null;
    sampleThreePointArc: (start: Point, mid: Point, end: Point, steps: number, complement: boolean) => Point[] | null;
    translationIterationDeltas: (
      depth: number,
      primary: Point,
      secondary: Point | null,
      bidirectional: boolean,
      includeOrigin: boolean,
    ) => Point[];
    rotateIterationPoints: (
      points: Point[],
      center: Point,
      angleRadians: number,
      depth: number,
    ) => Point[][];
    affineIterationSegments: (
      start: Point,
      end: Point,
      sourceTriangle: [Point, Point, Point],
      targetTriangle: [Point, Point, Point],
      depth: number,
    ) => [Point, Point][] | null;
    branchingIterationSegments: (
      start: Point,
      end: Point,
      targetSegments: [Point, Point][],
      depth: number,
    ) => [Point, Point][] | null;
    linePolylineIntersection: (
      lineStart: Point,
      lineEnd: Point,
      lineKind: RuntimeLineKind,
      points: Point[],
      sampleHint: number | null,
      variant: number,
    ) => Point | null;
    choosePointCandidate: (
      candidates: Point[],
      reference: Point | null,
      variant: number,
    ) => Point | null;
    lineCircleIntersectionCandidate: (
      start: Point,
      end: Point,
      lineKind: RuntimeLineKind,
      center: Point,
      radius: number,
      variant: number,
    ) => Point | null;
    pointDistance: (left: Point, right: Point, valueScale: number) => number | null;
    pointDistanceRatio: (origin: Point, denominator: Point, numerator: Point, clampToUnit: boolean) => number | null;
    pointAngleDegrees: (start: Point, vertex: Point, end: Point) => number | null;
    polygonArea: (points: Point[], valueScale: number) => number | null;
    evaluateExpr: (
      expr: FunctionExprJson | FunctionAstJson,
      x: number,
      parameters: Map<string, number>,
    ) => number | null;
    evaluateExprWithDriver: (
      expr: FunctionExprJson | FunctionAstJson,
      x: number,
      parameters: Map<string, number>,
      driverValue: number,
    ) => number | null;
    iterateExpression: (
      expr: FunctionExprJson | FunctionAstJson,
      parameterName: string,
      initialValue: number,
      parameters: Map<string, number>,
      count: number,
    ) => number[];
  };
  gspDebug?: {
    sourceScene: SceneData;
    viewerEnv: ViewerEnv;
    readonly runtime: {
      view: ViewState;
      scene: ViewerSceneData;
      dynamics: RuntimeDynamicsState;
      buttons: RuntimeButtonJson[];
    };
    readonly dependencyRun?: unknown;
    readonly selection?: unknown;
    json: () => string;
    graph: () => string;
    scene: () => string;
    dependencyGraph: () => unknown[];
    inspectSelection: () => string;
    inspectElement: (element: Element) => unknown;
    dumpJson: () => void;
    dumpGraph?: () => void;
    dumpScene: () => void;
    dumpSelection: () => void;
    dump: () => void;
    openPanel: () => void;
    closePanel: () => void;
    togglePanel: () => void;
  };
  van: typeof import("./vendor/van-1.6.0").default;
  GspViewerModules: Partial<ViewerModules>;
}
