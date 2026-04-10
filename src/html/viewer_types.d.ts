declare const van: any;

interface Window {
  gspDebug?: {
    sourceScene: SceneData;
    viewerEnv: ViewerEnv;
    readonly runtime: any;
    json: () => string;
    graph: () => string;
    dumpJson: () => void;
    dumpGraph: () => void;
    dump: () => void;
    openPanel: () => void;
    closePanel: () => void;
    togglePanel: () => void;
  };
}

type Point = {
  x: number;
  y: number;
};

type PointHandle =
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

type ArcBoundaryKind = "sector" | "circular-segment";

type ViewState = {
  centerX: number;
  centerY: number;
  zoom: number;
};

type SceneData = import("./generated/SceneData").SceneData;

type ViewerEnv = {
  canvas: HTMLCanvasElement | null;
  ctx: CanvasRenderingContext2D | null;
  sourceScene: SceneData;
  margin: number;
  trigMode: boolean;
  savedViewportMode: boolean;
  baseSpanX: number;
  baseSpanY: number;
  pointHitRadius: number;
  hoverPointIndex: { val: number | null };
  dragState: { val: any };
  view: any;
  currentScene: () => any;
  currentDynamics: () => {
    parameters: Array<{ name: string; value: number; unit?: string | null; labelIndex?: number | null }>;
    functions: Array<{
      name: string;
      derivative: boolean;
      labelIndex: number;
      lineIndex?: number | null;
      expr: any;
      domain: {
        xMin: number;
        xMax: number;
        sampleCount: number;
        plotMode: "cartesian" | "polar";
      };
      constrainedPointIndices: number[];
    }>;
  };
  currentHotspotFlashes: () => Array<{ key: string; action: any }>;
  resolveScenePoint: (index: number) => Point;
  resolvePoint: (handle: PointHandle) => Point;
  resolveAnchorBase: (handle: PointHandle) => Point;
  resolveLinePoints: (lineOrIndex: any) => Point[] | null;
  toScreen: (point: Point) => Point & { scale: number };
  toWorld: (x: number, y: number) => Point & { scale: number };
  getViewBounds: () => {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
    spanX: number;
    spanY: number;
  };
  rgba: (color: [number, number, number, number]) => string;
  updateScene: (mutator: (draft: any) => void) => void;
  updateDynamics: (mutator: (draft: any) => void) => void;
  syncDynamicScene: () => void;
  isOriginPointIndex: (index: number) => boolean;
  formatNumber: (value: number) => string;
  formatAxisNumber: (value: number) => string;
  formatPiLabel: (stepIndex: number) => string;
  drawGrid: () => void;
  inputTag: any;
  labelTag: any;
  parameterControls: HTMLElement | null;
  van: any;
};

type ViewerSceneModule = {
  resolveConstrainedPoint: (
    env: Pick<ViewerEnv, "sourceScene"> | ViewerEnv | null,
    constraint: any,
    resolveFn: (index: number) => Point,
    reference?: any,
  ) => Point | null;
  resolveScenePoint: (env: ViewerEnv, index: number) => Point;
  resolvePoint: (env: ViewerEnv, handle: PointHandle) => Point;
  resolveAnchorBase: (env: ViewerEnv, handle: PointHandle) => Point;
  resolveLinePoints: (env: ViewerEnv, lineOrIndex: any) => Point[] | null;
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
  ) => {
    t: number;
    projected: Point;
    distanceSquared: number;
  } | null;
  pointOnCircleArc: (center: Point, start: Point, end: Point, t: number) => Point | null;
  projectToCircleArc: (
    point: Point,
    center: Point,
    start: Point,
    end: Point,
  ) => {
    t: number;
    projected: Point;
    distanceSquared: number;
  } | null;
  pointOnThreePointArc: (start: Point, mid: Point, end: Point, t: number) => Point | null;
  projectToThreePointArc: (
    point: Point,
    start: Point,
    mid: Point,
    end: Point,
  ) => {
    t: number;
    projected: Point;
    distanceSquared: number;
  } | null;
  sampleArcBoundaryPoints: (
    env: ViewerEnv,
    binding: {
      kind: "arc-boundary";
      hostKey: number;
      boundaryKind: ArcBoundaryKind;
      centerIndex?: number | null;
      startIndex: number;
      midIndex?: number | null;
      endIndex: number;
      reversed: boolean;
      complement: boolean;
    },
  ) => Point[] | null;
  sampleCoordinateTracePoints: (
    env: ViewerEnv,
    binding: any,
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
  ) => Point | null;
  circleCircleIntersection: (
    leftCenter: Point,
    leftRadiusPoint: Point,
    rightCenter: Point,
    rightRadiusPoint: Point,
    variant: number,
  ) => Point | null;
  drawGrid: (env: ViewerEnv) => void;
};

type ViewerRenderModule = {
  draw: (env: ViewerEnv) => void;
  labelHotspotRects: (
    env: ViewerEnv,
    label: SceneData["labels"][number],
  ) => Array<{
    line: number;
    start: number;
    end: number;
    text: string;
    left: number;
    top: number;
    width: number;
    height: number;
    action: any;
  }>;
  findHitPoint: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitLabel: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitIterationTable: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitPolygon: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
};

type ViewerDragModule = {
  beginDrag: (
    env: ViewerEnv,
    pointerId: number,
    position: Point,
    pointIndex: number | null,
    labelIndex: number | null,
    polygonIndex: number | null,
    iterationTableIndex: number | null,
  ) => void;
  updateDraggedPoint: (env: ViewerEnv, world: Point) => void;
  updateDraggedLabel: (env: ViewerEnv, world: Point) => void;
  updateDraggedPolygon: (env: ViewerEnv, world: Point) => void;
  updateDraggedIterationTable: (env: ViewerEnv, world: Point) => void;
  panFromPointerDelta: (env: ViewerEnv, position: Point) => void;
};

type ViewerDynamicsModule = {
  buildParameterControls: (env: ViewerEnv) => void;
  evaluateExpr: (expr: any, x: number, parameters: Map<string, number>) => number | null;
  formatExpr: (expr: any, formatAxisNumber: (value: number) => string) => string;
  parameterValueFromPoint: (scene: any, pointIndex: number) => number | null;
  applyNormalizedParameterToPoint: (
    point: any,
    scene: any,
    normalizedValue: number,
  ) => void;
  refreshDerivedPoints: (env: ViewerEnv, scene: any) => void;
  refreshIterationGeometry: (env: ViewerEnv, scene: any, parameters: Map<string, number>) => void;
  refreshDynamicLabels: (env: ViewerEnv, scene: any) => void;
  syncDynamicScene: (env: ViewerEnv) => void;
};

interface Window {
  van: any;
  GspViewerModules: {
    scene?: ViewerSceneModule;
    render?: ViewerRenderModule;
    drag?: ViewerDragModule;
    dynamics?: ViewerDynamicsModule;
  };
}
