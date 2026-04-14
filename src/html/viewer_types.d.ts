declare const van: typeof import("./vendor/van-1.6.0").default;

type Point = import("./generated/PointJson").PointJson;
type BoundsJson = import("./generated/BoundsJson").BoundsJson;
type SceneData = import("./generated/SceneData").SceneData;
type ScenePointJson = import("./generated/ScenePointJson").ScenePointJson;
type PointConstraintJson = import("./generated/PointConstraintJson").PointConstraintJson;
type PointBindingJson = import("./generated/PointBindingJson").PointBindingJson;
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

type RuntimePointRef =
  | Point
  | {
      pointIndex: number;
      dx?: number;
      dy?: number;
    }
  | {
      lineIndex: number;
      segmentIndex?: number;
      t?: number;
      dx?: number;
      dy?: number;
      x?: number;
      y?: number;
    };

type PointHandle = RuntimePointRef;

type RuntimePointConstraintJson = any;
type RuntimeScenePointJson = any;
type RuntimeLineJson = any;
type RuntimePolygonJson = any;
type RuntimeCircleJson = any;
type RuntimeArcJson = any;
type RuntimeLabelHotspotJson = Omit<LabelHotspotJson, "action"> & {
  action: LabelHotspotActionJson | null;
};
type RuntimeLabelJson = any;
type TextLabel = RuntimeLabelJson;

type RuntimeIterationRow = {
  index: number;
  value: number;
};

type RuntimeIterationTableJson = IterationTableJson & {
  rows: RuntimeIterationRow[];
};
type RuntimeButtonJson = ButtonJson & {
  baseText: string;
  visible: boolean;
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

type RuntimePointIterationFamily = PointIterationJson;
type RuntimeLineIterationFamily = LineIterationJson;
type RuntimePolygonIterationFamily = PolygonIterationJson;
type RuntimeLabelIterationFamily = LabelIterationJson;
type RuntimeCircleIterationFamily = CircleIterationJson;

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
  canvas: SVGSVGElement | null;
  svg: SVGSVGElement | null;
  gridLayer: SVGGElement | null;
  sceneLayer: SVGGElement | null;
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
  rgba: (color: [number, number, number, number]) => string;
  updateScene: (mutator: (draft: ViewerSceneData) => void, mode?: "graph" | "none") => void;
  updateDynamics: (mutator: (draft: RuntimeDynamicsState) => void) => void;
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
  registerPointConstraintResolver: (
    kind: string,
    resolver: (
      env: ViewerEnv | null,
      constraint: RuntimePointConstraintJson,
      resolveFn: (index: number) => Point | null,
      reference?: RuntimeScenePointJson | Point | null,
    ) => Point | null,
  ) => void;
  registerLineBindingResolver: (
    kind: string,
    resolver: (env: ViewerEnv, line: RuntimeLineJson) => Point[] | null,
  ) => void;
  resolveConstrainedPoint: (
    env: { sourceScene: SceneData | ViewerSceneData } | ViewerEnv | null,
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
  pointOnCircleArc: (center: Point, start: Point, end: Point, t: number, yUp?: boolean) => Point | null;
  projectToCircleArc: (
    point: Point,
    center: Point,
    start: Point,
    end: Point,
    yUp?: boolean,
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
    binding: Extract<LineBindingJson, { kind: "arc-boundary" }>,
  ) => Point[] | null;
  sampleCoordinateTracePoints: (
    env: ViewerEnv | null,
    binding: Extract<LineBindingJson, { kind: "coordinate-trace" }> | Extract<RuntimePointConstraintJson, { kind: "line-trace-intersection" }>,
  ) => Point[] | null;
  lineLineIntersection: (
    leftStart: Point,
    leftEnd: Point,
    leftKind: string,
    rightStart: Point,
    rightEnd: Point,
    rightKind: string,
  ) => Point | null;
  lineCircleIntersection: (
    lineStart: Point,
    lineEnd: Point,
    lineKind: string,
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
  drawGrid: (env: ViewerEnv) => void;
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
  evaluateExpr: (expr: FunctionExprJson, x: number, parameters: Map<string, number>) => number | null;
  formatExpr: (expr: FunctionExprJson, formatAxisNumber: (value: number) => string, variableLabel?: string) => string;
  parameterValueFromPoint: (scene: ViewerSceneData, pointIndex: number) => number | null;
  applyNormalizedParameterToPoint: (
    point: RuntimeScenePointJson,
    scene: ViewerSceneData,
    normalizedValue: number,
  ) => void;
  parameterMapForScene?: (env: ViewerEnv, scene: ViewerSceneData) => Map<string, number>;
  refreshDerivedPoints: (env: ViewerEnv, scene: ViewerSceneData) => void;
  refreshIterationGeometry: (env: ViewerEnv, scene: ViewerSceneData, parameters: Map<string, number>) => void;
  refreshDynamicLabels: (env: ViewerEnv, scene: ViewerSceneData) => void;
  parameterRootId?: (name: string) => string;
  sourcePointRootId?: (index: number) => string;
  runDependencyGraph?: (env: ViewerEnv, scene: ViewerSceneData, dirtyRootIds: string[]) => unknown;
  describeDependencyGraph?: (env: ViewerEnv) => unknown[];
  syncDynamicScene: (env: ViewerEnv, dirtyParameterNames?: string[]) => void;
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
  scene: ViewerSceneModule;
  render: ViewerRenderModule;
  overlay: ViewerOverlayModule;
  drag: ViewerDragModule;
  dynamics: ViewerDynamicsModule;
};

interface Window {
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
