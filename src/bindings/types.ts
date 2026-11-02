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
}

export interface PhotoMetadata {
  width: number | null;
  height: number | null;
  capturedAt: string | null;
  cameraMake: string | null;
  cameraModel: string | null;
  lensModel: string | null;
  gpsLat: number | null;
  gpsLon: number | null;
  exposureTime: string | null;
  fNumber: number | null;
  iso: number | null;
  focalLength: number | null;
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
  finishedAt: string;
  warnings: string[];
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
