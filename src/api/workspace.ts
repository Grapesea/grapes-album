import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { WorkspaceSnapshot } from "../bindings/types";

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
