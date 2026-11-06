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
  classifications: AssetClassification[];
}

export interface DimensionValue {
  id: string;
  dimensionId: string;
  parentId: string | null;
  name: string;
  description: string | null;
  color: string | null;
  sortOrder: number;
}

export interface Dimension {
  id: string;
  name: string;
  kind: string;
  sortOrder: number;
  systemDefined: boolean;
  cardinality: "single" | "multi";
  origin: "derived" | "manual" | "hybrid";
  values: DimensionValue[];
}

export interface AssetClassification {
  dimensionId: string;
  dimensionName: string;
  dimensionKind: string;
  valueId: string;
  valueName: string;
  color: string | null;
  source: string;
  confidence: number | null;
  status: string;
  method: string | null;
  modelId: string | null;
}

export interface SmartGraphFilter {
  dimensionId: string;
  valueIds: string[];
}

export interface PhotoMetadata {
  width: number | null;
  height: number | null;
  capturedAt: string | null;
  capturedOffsetMinutes: number | null;
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
  kind: "root" | "dimension" | "value" | "smart";
  dimension: string | null;
  dimensionValue: string | null;
  photoCount: number;
  childCount: number;
}

export interface GraphNode {
  id: string;
  kind: "portal" | "asset";
  dummy: boolean;
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
  weight: number;
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

export interface SemanticProviderConfig {
  kind: "qa" | "embedding" | "rerank";
  providerType: string;
  baseUrl: string;
  model: string;
  enabled: boolean;
  apiKey: string | null;
  requestTimeoutMs: number;
  maxConcurrency: number;
  maxRetries: number;
}

export interface SemanticSettings {
  qa: SemanticProviderConfig;
  embedding: SemanticProviderConfig;
  rerank: SemanticProviderConfig;
  indexStatus: SemanticIndexStatus;
}

export interface SemanticIndexStatus {
  totalAssets: number;
  indexedAssets: number;
  pendingAssets: number;
  modelId: string | null;
  dimension: number | null;
}

export interface SemanticIndexReport {
  indexed: number;
  failed: number;
  warnings: string[];
}

export interface SemanticIndexResult {
  report: SemanticIndexReport;
  status: SemanticIndexStatus;
}

export interface SemanticMatchReason {
  kind: "embedding" | "classification" | "metadata" | "rerank";
  label: string;
  score: number;
}

export interface SemanticSearchResult {
  assetId: string;
  filename: string;
  relativePath: string;
  score: number;
  matchReasons: SemanticMatchReason[];
}

export interface SemanticSearchResponse {
  query: string;
  answer: string | null;
  searchPlan: SearchPlan | null;
  planSource: "qa" | "fallback";
  embeddingUsed: boolean;
  candidateCount: number;
  reranked: boolean;
  results: SemanticSearchResult[];
  warnings: string[];
}

export interface AskGrapesProgress {
  requestId: string;
  stage:
    | "query"
    | "planning"
    | "embedding"
    | "retrieval"
    | "rerank"
    | "answer"
    | "complete"
    | "cancelled"
    | "error";
  state: "started" | "waiting" | "success" | "warning" | "error";
  message: string;
  detail: string | null;
  elapsedMs: number;
}

export interface SearchPlanFilter {
  dimensionId: string;
  dimensionName: string;
  valueIds: string[];
  valueNames: string[];
}

export interface SearchPlan {
  hardFilters: SearchPlanFilter[];
  softFilters: SearchPlanFilter[];
  semanticQueries: string[];
  exclusions: string[];
  rerankInstruction: string | null;
}

export type DragPayload =
  | { type: "folder"; id: string }
  | { type: "asset"; id: string };
