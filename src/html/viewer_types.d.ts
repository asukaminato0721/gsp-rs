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
  | RuntimeButtonJson
  | RuntimeLabelJson
  | ImageJson
  | RuntimeScenePointJson
  | RuntimeLineJson
  | RuntimeCircleJson
  | RuntimePolygonJson;

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

type RuntimePointBindingJson = (PointBindingJson | {
  kind: "rotate";
  sourceIndex: number;
  centerIndex: number;
  angleDegrees: number;
}) & {
  angleExpr?: FunctionExprJson;
  angleDegreesScale?: number;
  absoluteValue?: boolean;
  axis?: CoordinateAxisJson;
  axisEndIndex?: number;
  clampToUnit?: boolean;
  centerIndex?: number;
  distanceRawScale?: number;
  distanceExpr?: FunctionExprJson;
  expr?: FunctionExprJson;
  name?: string;
  originIndex?: number;
  parameterEndIndex?: number | null;
  parameterName?: string;
  parameterStartIndex?: number | null;
  ratioDenominatorIndex?: number;
  ratioNumeratorIndex?: number;
  ratioOriginIndex?: number;
  signed?: boolean;
  sourceIndex?: number;
  startIndex?: number;
  midIndex?: number | null;
  endIndex?: number;
  transform?: PointTransformJson;
  xExpr?: FunctionExprJson;
  xName?: string;
  xScale?: number;
  yExpr?: FunctionExprJson;
  yName?: string;
  yScale?: number;
};

type RuntimeLabelBindingJson = LabelBindingJson & {
  depth?: number;
  depthParameterName?: string | null;
  expr?: FunctionExprJson;
  exprLabel?: string;
  name?: string;
  parameterName?: string;
  pointIndex?: number;
  pointName?: string;
  resultName?: string | null;
  anchorDx?: number;
  anchorDy?: number;
  anchorYDy?: number | null;
  anchorYPointIndex?: number | null;
  axis?: CoordinateAxisJson;
  circleName?: string;
  clampToUnit?: boolean;
  decimals?: number;
  denominatorIndex?: number;
  endIndex?: number;
  leftIndex?: number;
  numeratorIndex?: number;
  objectName?: string;
  originIndex?: number | null;
  pointIndices?: number[];
  polygonName?: string;
  refs?: RichTextExpressionRefJson[];
  rightIndex?: number;
  segmentName?: string;
  startIndex?: number;
  templateRichMarkup?: string | null;
  templateText?: string;
  valueScale?: number;
  valueSuffix?: string;
  vertexIndex?: number;
  xUnitIndex?: number | null;
  yUnitIndex?: number | null;
};

type RuntimeLineBindingJson = LineBindingJson & {
  boundaryKind?: ArcBoundaryKind;
  complement?: boolean;
  centerIndex?: number | null;
  depth?: number;
  depthParameterName?: string | null;
  driverIndex?: number;
  endIndex?: number;
  hostKey?: number;
  lineEndIndex?: number | null;
  lineIndex?: number | null;
  lineStartIndex?: number | null;
  markerClass?: number;
  midIndex?: number | null;
  parameterName?: string;
  pointIndex?: number;
  ray?: boolean;
  reflectionAxisLineIndex?: number | null;
  reflectionDirectrixLineIndex?: number | null;
  reflectionFocusIndex?: number | null;
  reflectionSourceIndex?: number | null;
  reversed?: boolean;
  sampleCount?: number;
  sourceIndex?: number;
  startIndex?: number;
  stepIndex?: number;
  throughIndex?: number;
  traceEndpointIndex?: number;
  traceLineIndex?: number;
  transform?: TransformJson;
  useMidpoints?: boolean;
  vertexIndex?: number;
  xExpr?: FunctionExprJson;
  xMax?: number;
  xMin?: number;
  yExpr?: FunctionExprJson;
};

type RuntimeShapeBindingJson = ShapeBindingJson & {
  boundaryKind?: ArcBoundaryKind;
  complement?: boolean;
  centerIndex?: number | null;
  endIndex?: number;
  expr?: FunctionExprJson;
  hostKey?: number;
  lineEndIndex?: number;
  lineStartIndex?: number;
  midIndex?: number | null;
  parameterName?: string;
  radiusIndex?: number;
  rawPerUnit?: number;
  reversed?: boolean;
  startIndex?: number;
  vertexIndices?: number[];
  sourceIndex?: number;
  transform?: TransformJson;
};

type RuntimePolylineConstraintJson = Omit<
  Extract<PointConstraintJson, { kind: "polyline" }>,
  "points"
> & {
  points: PointHandle[];
};
type RuntimePointConstraintJson = (
  | Exclude<PointConstraintJson, { kind: "polyline" }>
  | RuntimePolylineConstraintJson
) & {
  t?: number;
  points?: PointHandle[];
  line?: LineConstraintJson;
  circle?: CircularConstraintJson;
  left?: LineConstraintJson | CircularConstraintJson;
  right?: LineConstraintJson | CircularConstraintJson;
  vertexIndices?: number[];
  edgeIndex?: number;
  unitX?: number;
  unitY?: number;
  functionKey?: number;
  pointIndex?: number;
  sampleCount?: number;
  startIndex?: number;
  endIndex?: number;
  midIndex?: number | null;
  centerIndex?: number | null;
  boundaryKind?: ArcBoundaryKind;
  reversed?: boolean;
  complement?: boolean;
  xMin?: number;
  xMax?: number;
};
type RuntimeScenePointJson = Omit<ScenePointJson, "constraint" | "binding" | "debug"> & {
  constraint?: RuntimePointConstraintJson | null;
  binding?: RuntimePointBindingJson | null;
  debug?: DebugSourceJson | null;
};
type RuntimeLineJson = Partial<Omit<LineJson, "points" | "binding" | "debug" | "color">> & {
  points: PointHandle[];
  color?: [number, number, number, number] | number[];
  binding?: RuntimeLineBindingJson | null;
  segments?: Point[][];
  debug?: DebugSourceJson | null;
};
type RuntimePolygonJson = Partial<Omit<PolygonJson, "points" | "binding" | "debug" | "color">> & {
  points: PointHandle[];
  color?: [number, number, number, number] | number[];
  binding?: RuntimeShapeBindingJson | null;
  debug?: DebugSourceJson | null;
};
type RuntimeCircleJson = Partial<Omit<CircleJson, "center" | "radiusPoint" | "binding" | "debug" | "color" | "fillColor">> & {
  center?: PointHandle;
  radiusPoint?: PointHandle;
  color?: [number, number, number, number] | number[];
  fillColor?: [number, number, number, number] | number[] | null;
  binding?: RuntimeShapeBindingJson | null;
  debug?: DebugSourceJson | null;
};
type RuntimeArcJson = Partial<Omit<ArcJson, "points" | "center" | "debug">> & {
  points: PointHandle[];
  center?: PointHandle | null;
  debug?: DebugSourceJson | null;
};
type RuntimeLabelHotspotJson = Omit<LabelHotspotJson, "action"> & {
  action: LabelHotspotActionJson;
};
type RuntimeLabelJson = Partial<Omit<LabelJson, "anchor" | "binding" | "hotspots" | "debug">> & {
  anchor?: PointHandle;
  binding?: RuntimeLabelBindingJson | null;
  centeredOnAnchor?: boolean;
  hotspots?: RuntimeLabelHotspotJson[];
  debug?: DebugSourceJson | null;
};
type TextLabel = RuntimeLabelJson;

type RuntimeIterationRow = {
  index: number;
  value: number;
  values: number[];
};

type RuntimeIterationTableJson = Partial<IterationTableJson> & {
  rows?: RuntimeIterationRow[];
};
type RuntimeButtonJson = Partial<ButtonJson> & {
  baseText?: string;
  visible?: boolean;
  active?: boolean;
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
    binding: RuntimeLineBindingJson | RuntimeShapeBindingJson,
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
  formatExpr: ((expr: FunctionExprJson, formatAxisNumber: (value: number) => string, variableLabel?: string) => string) | null;
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
  exprContainsPiAngle: (expr: FunctionExprJson | FunctionAstJson | null | undefined) => boolean;
  formatExpr: (
    expr: FunctionExprJson | FunctionAstJson,
    formatAxisNumber: (value: number) => string,
    variableLabel?: string,
  ) => string;
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
  formatExpr: ViewerDynamicsExpressionModule["formatExpr"];
  formatSequenceValue: (value: number) => string;
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
  collectExprParameterNames: (
    expr: FunctionExprJson | FunctionAstJson | null | undefined,
    names: Set<string>,
  ) => void;
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
  parameterNameFromPoint: (scene: ViewerSceneData, pointIndex: number) => string | null;
  parameterValueFromPoint: (scene: ViewerSceneData, pointIndex: number) => number | null;
  pointOnPolylineByIndex: (points: Point[], normalized: number) => Point | null;
  polylineParameterFromPoint: (scene: ViewerSceneData, pointIndex: number) => number | null;
  reflectAcrossLine: (point: Point, lineStart: Point, lineEnd: Point) => Point | null;
  reflectionAxisPoints: (
    scene: ViewerSceneData,
    binding: HostLineBinding,
  ) => [PointHandle | null, PointHandle | null];
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
    collectExprParameterNames: (
      expr: FunctionExprJson | FunctionAstJson | null | undefined,
      names: Set<string>,
    ) => void;
    describeDependencyGraph: (env: ViewerEnv) => unknown[];
    runDependencyGraph: (env: ViewerEnv, scene: ViewerSceneData, dirtyRootIds: string[]) => unknown;
  };
};

type RuntimeSceneDependencyCollector = {
  expr: (deps: Set<string>, expr: FunctionExprJson | FunctionAstJson | null | undefined) => void;
  points: (
    deps: Set<string>,
    indices: readonly (number | null | undefined)[] | null | undefined,
  ) => void;
  pointBinding: (deps: Set<string>, binding: RuntimePointBindingJson | null | undefined) => void;
  pointConstraint: (deps: Set<string>, constraint: RuntimePointConstraintJson | null | undefined) => void;
  lineBinding: (deps: Set<string>, binding: RuntimeLineBindingJson | null | undefined) => void;
  shapeBinding: (
    deps: Set<string>,
    binding: RuntimeShapeBindingJson | null | undefined,
    sourceKind: "circle" | "polygon",
  ) => void;
  colorBinding: (deps: Set<string>, binding: ColorBindingJson | null | undefined) => void;
  labelBinding: (deps: Set<string>, binding: RuntimeLabelBindingJson | null | undefined) => void;
  labelReferencedParameterNames: (
    binding: RuntimeLabelBindingJson | null | undefined,
    names: Set<string>,
  ) => void;
  pointIteration: (deps: Set<string>, family: PointIterationJson) => void;
  lineIteration: (deps: Set<string>, family: LineIterationJson) => void;
  circleIteration: (deps: Set<string>, family: CircleIterationJson) => void;
  polygonIteration: (deps: Set<string>, family: PolygonIterationJson) => void;
  labelIteration: (deps: Set<string>, family: LabelIterationJson) => void;
  iterationTable: (deps: Set<string>, table: IterationTableJson) => void;
};

type ViewerDynamicsDependenciesModule = {
  createSceneDependencyCollector: (options: {
    sourceScene: SceneData | ViewerSceneData;
    knownParameters: Set<string>;
    derivedParameterDeps?: Map<string, Set<string>>;
    collectExprParameterNames: (
      expr: FunctionExprJson | FunctionAstJson | null | undefined,
      names: Set<string>,
    ) => void;
  }) => RuntimeSceneDependencyCollector;
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
    evaluateExpr: (
      expr: FunctionExprJson | FunctionAstJson,
      x: number,
      parameters: Map<string, number>,
    ) => number | null;
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
