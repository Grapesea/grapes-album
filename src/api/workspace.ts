import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  Dimension,
  GraphInfo,
  GraphView,
  SmartGraphFilter,
  WorkspaceSnapshot,
} from "../bindings/types";

export const desktopRuntime = isTauri();

export async function chooseWorkspace(title: string): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof selected === "string" ? selected : null;
}

export function openWorkspace(path: string): Promise<WorkspaceSnapshot> {
  return invoke("open_workspace", { path });
}

export function getWorkspaceSnapshot(): Promise<WorkspaceSnapshot> {
  return invoke("workspace_snapshot");
}

export function reconcileWorkspace(): Promise<WorkspaceSnapshot> {
  return invoke("refresh_workspace");
}

export function createFolder(
  parentFolderId: string,
  name: string,
): Promise<WorkspaceSnapshot> {
  return invoke("create_folder", { parentFolderId, name });
}

export function moveFolder(
  folderId: string,
  destinationFolderId: string,
): Promise<WorkspaceSnapshot> {
  return invoke("move_folder", { folderId, destinationFolderId });
}

export function renameFolder(
  folderId: string,
  newName: string,
): Promise<WorkspaceSnapshot> {
  return invoke("rename_folder", { folderId, newName });
}

export function moveAsset(
  assetId: string,
  destinationFolderId: string,
): Promise<WorkspaceSnapshot> {
  return invoke("move_asset", { assetId, destinationFolderId });
}

export function renameAsset(
  assetId: string,
  newName: string,
): Promise<WorkspaceSnapshot> {
  return invoke("rename_asset", { assetId, newName });
}

export function setAssetTags(
  assetId: string,
  tags: string[],
): Promise<WorkspaceSnapshot> {
  return invoke("set_asset_tags", { assetId, tags });
}

export function getGraphCatalog(): Promise<GraphInfo[]> {
  return invoke("graph_catalog");
}

export function getGraphView(graphId?: string): Promise<GraphView> {
  return invoke("graph_view", { graphId: graphId ?? null });
}

export function saveGraphPosition(
  graphId: string,
  nodeKey: string,
  x: number,
  y: number,
  pinned: boolean,
): Promise<void> {
  return invoke("save_graph_position", {
    graphId,
    nodeKey,
    x,
    y,
    pinned,
  });
}

export function getClassificationSchema(): Promise<Dimension[]> {
  return invoke("classification_schema");
}

export function setAssetClassification(
  assetId: string,
  valueId: string,
  selected: boolean,
): Promise<WorkspaceSnapshot> {
  return invoke("set_asset_classification", { assetId, valueId, selected });
}

export function createCustomDimension(name: string): Promise<Dimension[]> {
  return invoke("create_custom_dimension", { name });
}

export function createDimensionValue(
  dimensionId: string,
  name: string,
): Promise<Dimension[]> {
  return invoke("create_dimension_value", { dimensionId, name });
}

export function createSmartGraph(
  name: string,
  filters: SmartGraphFilter[],
): Promise<GraphInfo> {
  return invoke("create_smart_graph", { name, filters });
}

export function deleteSmartGraph(graphId: string): Promise<GraphInfo[]> {
  return invoke("delete_smart_graph", { graphId });
}
