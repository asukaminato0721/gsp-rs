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

type SceneData = {
  width: number;
  height: number;
  graphMode?: boolean;
  yUp?: boolean;
  piMode?: boolean;
  savedViewport?: boolean;
  bounds: {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
  };
  origin?: PointHandle | null;
  images?: Array<{
    topLeft: Point;
    bottomRight: Point;
    src: string;
    screenSpace?: boolean;
  }>;
  points: any[];
  lines: any[];
  polygons: any[];
  circles: any[];
  arcs: Array<{
    color: [number, number, number, number];
    visible?: boolean;
    points: PointHandle[];
    center?: PointHandle | null;
    counterclockwise?: boolean;
  }>;
  labels: Array<{
    anchor: Point;
    text: string;
    richMarkup?: string | null;
    color: [number, number, number, number];
    binding?: any;
    screenSpace?: boolean;
    hotspots?: Array<{
      line: number;
      start: number;
      end: number;
      text: string;
      action: {
        kind: "button" | "point" | "segment" | "angle-marker" | "circle" | "polygon";
        buttonIndex?: number;
        pointIndex?: number;
        startPointIndex?: number;
        vertexPointIndex?: number;
        endPointIndex?: number;
        circleIndex?: number;
        polygonIndex?: number;
      };
    }>;
  }>;
  pointIterations?: Array<
    | {
        kind: "offset";
        seedIndex: number;
        dx: number;
        dy: number;
        depth: number;
        parameterName?: string | null;
      }
    | {
        kind: "rotate-chain";
        seedIndex: number;
        centerIndex: number;
        angleDegrees: number;
        depth: number;
      }
    | {
        kind: "rotate";
        sourceIndex: number;
        centerIndex: number;
        angleExpr: any;
        depth: number;
        parameterName?: string | null;
      }
  >;
  lineIterations?: Array<{
    kind: "translate";
    startIndex: number;
    endIndex: number;
    dx: number;
    dy: number;
    secondaryDx?: number | null;
    secondaryDy?: number | null;
    depth: number;
    parameterName?: string | null;
    color: [number, number, number, number];
    dashed: boolean;
  } | {
    kind: "affine";
    startIndex: number;
    endIndex: number;
    sourceTriangleIndices: [number, number, number];
    targetTriangle: [PointHandle, PointHandle, PointHandle];
    depth: number;
    color: [number, number, number, number];
    dashed: boolean;
  }>;
  // line binding kinds are structural in JS; no explicit TS alias here.
  polygonIterations?: Array<{
    kind: "translate";
    vertexIndices: number[];
    dx: number;
    dy: number;
    secondaryDx?: number | null;
    secondaryDy?: number | null;
    depth: number;
    parameterName?: string | null;
    color: [number, number, number, number];
  }>;
  labelIterations?: Array<{
    kind: "point-expression";
    seedLabelIndex: number;
    pointSeedIndex: number;
    parameterName: string;
    expr: any;
    depth: number;
    depthParameterName?: string | null;
  }>;
  buttons?: Array<{
    text: string;
    x: number;
    y: number;
    width?: number | null;
    height?: number | null;
    action: {
      kind: string;
      href?: string;
      visible?: boolean;
      pointIndices?: number[];
      lineIndices?: number[];
      circleIndices?: number[];
      polygonIndices?: number[];
      pointIndex?: number;
      targetPointIndex?: number | null;
      buttonIndices?: number[];
      intervalMs?: number;
    };
  }>;
  parameters?: any[];
  functions?: any[];
};

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
  resolveConstrainedPoint: (env: Pick<ViewerEnv, "sourceScene"> | ViewerEnv | null, constraint: any, resolveFn: (index: number) => Point) => Point | null;
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
  ) => void;
  updateDraggedPoint: (env: ViewerEnv, world: Point) => void;
  updateDraggedLabel: (env: ViewerEnv, world: Point) => void;
  updateDraggedPolygon: (env: ViewerEnv, world: Point) => void;
  panFromPointerDelta: (env: ViewerEnv, position: Point) => void;
};

type ViewerDynamicsModule = {
  buildParameterControls: (env: ViewerEnv) => void;
  evaluateExpr: (expr: any, x: number, parameters: Map<string, number>) => number | null;
  formatExpr: (expr: any, formatAxisNumber: (value: number) => string) => string;
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
