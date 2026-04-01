declare const van: any;

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

type ViewState = {
  centerX: number;
  centerY: number;
  zoom: number;
};

type SceneData = {
  width: number;
  height: number;
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
  points: any[];
  lines: any[];
  polygons: any[];
  circles: any[];
  labels: any[];
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
    parameters: Array<{ name: string; value: number; labelIndex?: number | null }>;
    functions: any[];
  };
  resolveScenePoint: (index: number) => Point;
  resolvePoint: (handle: PointHandle) => Point;
  resolveAnchorBase: (handle: PointHandle) => Point;
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
  resolveConstrainedPoint: (env: ViewerEnv | null, constraint: any, resolveFn: (index: number) => Point) => Point | null;
  resolveScenePoint: (env: ViewerEnv, index: number) => Point;
  resolvePoint: (env: ViewerEnv, handle: PointHandle) => Point;
  resolveAnchorBase: (env: ViewerEnv, handle: PointHandle) => Point;
  toScreen: (env: ViewerEnv, point: Point) => Point & { scale: number };
  toWorld: (env: ViewerEnv, x: number, y: number) => Point & { scale: number };
  getViewBounds: (env: ViewerEnv) => ViewerEnv["getViewBounds"] extends () => infer T ? T : never;
  getCanvasCoords: (env: ViewerEnv, event: MouseEvent | PointerEvent | WheelEvent) => Point;
  chooseGridStep: (span: number, targetLines: number) => number;
  drawGrid: (env: ViewerEnv) => void;
};

type ViewerRenderModule = {
  draw: (env: ViewerEnv) => void;
  findHitPoint: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
  findHitLabel: (env: ViewerEnv, screenX: number, screenY: number) => number | null;
};

type ViewerDragModule = {
  beginDrag: (env: ViewerEnv, pointerId: number, position: Point, pointIndex: number | null, labelIndex: number | null) => void;
  updateDraggedPoint: (env: ViewerEnv, world: Point) => void;
  updateDraggedLabel: (env: ViewerEnv, world: Point) => void;
  panFromPointerDelta: (env: ViewerEnv, position: Point) => void;
};

type ViewerDynamicsModule = {
  buildParameterControls: (env: ViewerEnv) => void;
  evaluateExpr: (expr: any, x: number, parameters: Map<string, number>) => number | null;
  formatExpr: (expr: any, formatAxisNumber: (value: number) => string) => string;
  refreshDerivedPoints: (env: ViewerEnv, scene: any) => void;
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
