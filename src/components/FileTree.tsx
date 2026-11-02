import {
  ChevronRight,
  FileImage,
  Folder,
  FolderOpen,
} from "lucide-react";
import type { DragEvent } from "react";
import type {
  AssetView,
  DragPayload,
  FolderView,
} from "../bindings/types";
import { useI18n } from "../i18n";

interface FileTreeProps {
  folders: FolderView[];
  assets: AssetView[];
  rootFolderId: string;
  selectedFolderId: string | null;
  selectedAssetId: string | null;
  expanded: Set<string>;
  onToggle: (folderId: string) => void;
  onSelectFolder: (folderId: string) => void;
  onSelectAsset: (asset: AssetView) => void;
  onDrop: (destinationFolderId: string, payload: DragPayload) => void;
}

export function FileTree({
  folders,
  assets,
  rootFolderId,
  selectedFolderId,
  selectedAssetId,
  expanded,
  onToggle,
  onSelectFolder,
  onSelectAsset,
  onDrop,
}: FileTreeProps) {
  const { t } = useI18n();
  const childFolders = new Map<string, FolderView[]>();
  for (const folder of folders) {
    if (!folder.parentId) continue;
    const children = childFolders.get(folder.parentId) ?? [];
    children.push(folder);
    childFolders.set(folder.parentId, children);
  }
  for (const children of childFolders.values()) {
    children.sort((left, right) => left.name.localeCompare(right.name));
  }

  const childAssets = new Map<string, AssetView[]>();
  for (const asset of assets) {
    const children = childAssets.get(asset.folderId) ?? [];
    children.push(asset);
    childAssets.set(asset.folderId, children);
  }
  for (const children of childAssets.values()) {
    children.sort((left, right) =>
      left.filename.localeCompare(right.filename),
    );
  }

  const renderFolder = (folder: FolderView, depth: number) => {
    const isExpanded = expanded.has(folder.id);
    const children = childFolders.get(folder.id) ?? [];
    const files = childAssets.get(folder.id) ?? [];
    return (
      <div key={folder.id}>
        <TreeRow
          kind="folder"
          depth={depth}
          label={folder.name}
          selected={selectedFolderId === folder.id}
          expanded={isExpanded}
          hasChildren={children.length > 0 || files.length > 0}
          onClick={() => onSelectFolder(folder.id)}
          onToggle={() => onToggle(folder.id)}
          payload={{ type: "folder", id: folder.id }}
          onDrop={(payload) => onDrop(folder.id, payload)}
        />
        {isExpanded && (
          <>
            {children.map((child) => renderFolder(child, depth + 1))}
            {files.map((asset) => (
              <button
                key={asset.id}
                type="button"
                draggable
                className={`tree-row tree-file ${
                  selectedAssetId === asset.id ? "selected" : ""
                }`}
                style={{ paddingLeft: 18 + (depth + 0.32) * 16 }}
                onClick={() => onSelectAsset(asset)}
                onDragStart={(event) => setDragPayload(event, {
                  type: "asset",
                  id: asset.id,
                })}
                title={asset.relativePath}
              >
                <span className="tree-caret-spacer" />
                <FileImage size={15} />
                <span className="tree-label">{asset.filename}</span>
              </button>
            ))}
          </>
        )}
      </div>
    );
  };

  const root = folders.find((folder) => folder.id === rootFolderId);
  if (!root) return null;
  return (
    <div className="file-tree" role="tree" aria-label={t("fileTree")}>
      {renderFolder(root, 0)}
    </div>
  );
}

interface TreeRowProps {
  kind: "folder";
  depth: number;
  label: string;
  selected: boolean;
  expanded: boolean;
  hasChildren: boolean;
  payload: DragPayload;
  onClick: () => void;
  onToggle: () => void;
  onDrop: (payload: DragPayload) => void;
}

function TreeRow({
  depth,
  label,
  selected,
  expanded,
  hasChildren,
  payload,
  onClick,
  onToggle,
  onDrop,
}: TreeRowProps) {
  return (
    <button
      type="button"
      draggable
      role="treeitem"
      aria-expanded={hasChildren ? expanded : undefined}
      className={`tree-row tree-folder ${selected ? "selected" : ""}`}
      style={{ paddingLeft: 8 + depth * 16 }}
      onClick={onClick}
      onDragStart={(event) => setDragPayload(event, payload)}
      onDragOver={(event) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
      }}
      onDrop={(event) => {
        event.preventDefault();
        const value = event.dataTransfer.getData("application/x-grapes");
        if (!value) return;
        try {
          onDrop(JSON.parse(value) as DragPayload);
        } catch {
          // Ignore drags that did not originate inside Grapes Album.
        }
      }}
      title={label}
    >
      <span
        className="tree-caret"
        role="button"
        tabIndex={-1}
        onClick={(event) => {
          event.stopPropagation();
          if (hasChildren) onToggle();
        }}
      >
        <ChevronRight
          size={14}
          className={expanded ? "caret-open" : ""}
        />
      </span>
      {expanded ? <FolderOpen size={16} /> : <Folder size={16} />}
      <span className="tree-label">{label}</span>
    </button>
  );
}

function setDragPayload(event: DragEvent, payload: DragPayload) {
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData(
    "application/x-grapes",
    JSON.stringify(payload),
  );
}
