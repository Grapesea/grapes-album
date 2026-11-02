export interface WorkspaceView {
  id: string;
  name: string;
  rootPath: string;
  schemaVersion: number;
  createdAt: string;
}

export interface FolderView {
  id: string;
  parentId: string | null;
  name: string;
  relativePath: string;
  fsFileId: string | null;
}

export interface AssetView {
  id: string;
  folderId: string;
  filename: string;
  relativePath: string;
  fileSize: number;
  mtimeNs: number;
  fsFileId: string | null;
  contentHash: string | null;
  tags: string[];
  metadata: PhotoMetadata;
  hasThumbnail: boolean;
}

export interface PhotoMetadata {
  width: number | null;
  height: number | null;
  capturedAt: string | null;
  cameraMake: string | null;
  cameraModel: string | null;
  lensModel: string | null;
  focalLength: number | null;
  gpsLat: number | null;
  gpsLon: number | null;
  exposureTime: string | null;
  fNumber: number | null;
  iso: number | null;
  orientation: number | null;
  scanned: boolean;
}

export interface ReconcileReport {
  scannedFolders: number;
  scannedAssets: number;
  foldersInserted: number;
  foldersMoved: number;
  foldersRemoved: number;
  assetsInserted: number;
  assetsMoved: number;
  assetsRemoved: number;
  contentChanged: number;
  thumbnailsCreated: number;
  finishedAt: string;
  warnings: string[];
}

export interface GraphInfo {
  id: string;
  parentId: string | null;
  name: string;
  kind: "root" | "dimension" | "value";
  dimension: string | null;
  dimensionValue: string | null;
  photoCount: number;
  childCount: number;
}

export interface GraphNode {
  id: string;
  kind: "portal" | "asset";
  label: string;
  subtitle: string | null;
  targetGraphId: string | null;
  assetId: string | null;
  photoCount: number;
  available: boolean;
  x: number | null;
  y: number | null;
  pinned: boolean;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
}

export interface GraphView {
  graph: GraphInfo;
  breadcrumbs: GraphInfo[];
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface WorkspaceSnapshot {
  workspace: WorkspaceView;
  folders: FolderView[];
  assets: AssetView[];
  rootFolderId: string;
  lastReconcile: ReconcileReport | null;
}

export type DragPayload =
  | { type: "folder"; id: string }
  | { type: "asset"; id: string };
