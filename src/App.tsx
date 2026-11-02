import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
  lazy,
  Suspense,
} from "react";
import type { ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowRight,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Database,
  FileImage,
  Folder as FolderIcon,
  FolderPlus,
  GitBranch,
  HardDrive,
  LoaderCircle,
  Pencil,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import {
  chooseWorkspace,
  createFolder,
  desktopRuntime,
  getWorkspaceSnapshot,
  moveAsset,
  moveFolder,
  openWorkspace,
  reconcileWorkspace,
  renameAsset,
  renameFolder,
  setAssetTags,
} from "./api/workspace";
import type {
  AssetView,
  DragPayload,
  FolderView,
  ReconcileReport,
  WorkspaceSnapshot,
} from "./bindings/types";
import { FileTree } from "./components/FileTree";
import { Inspector } from "./components/Inspector";
import { AssetTags } from "./components/AssetTags";
import { AssetThumbnail } from "./components/AssetThumbnail";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { useI18n } from "./i18n";
import { VirtualGallery } from "./components/VirtualGallery";
import { PhotoWorkspace } from "./components/PhotoWorkspace";
import { absoluteWorkspacePath } from "./utils/paths";
import { SidebarResizer } from "./components/SidebarResizer";
const GraphWorkspace = lazy(() =>
  import("./components/GraphWorkspace").then((module) => ({
    default: module.GraphWorkspace,
  })),
);

const LEFT_SIDEBAR_MIN = 190;
const RIGHT_SIDEBAR_MIN = 230;
const CENTER_PANEL_MIN = 360;
const SIDEBAR_HANDLES_WIDTH = 10;

export default function App() {
  const { locale, t } = useI18n();
  const mainWorkspaceRef = useRef<HTMLElement>(null);
  const workspaceRef = useRef<HTMLDivElement>(null);
  const [leftSidebarWidth, setLeftSidebarWidth] = useState(() =>
    storedSidebarWidth("grapes.sidebar.left", 264, LEFT_SIDEBAR_MIN),
  );
  const [rightSidebarWidth, setRightSidebarWidth] = useState(() =>
    storedSidebarWidth("grapes.sidebar.right", 316, RIGHT_SIDEBAR_MIN),
  );
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null);
  const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);
  const [selectedAssetId, setSelectedAssetId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [folderView, setFolderView] = useState<
    "medium" | "small" | "details"
  >("medium");
  const [photoOpen, setPhotoOpen] = useState(false);
  const [graphOpen, setGraphOpen] = useState(false);
  const [watcherReport, setWatcherReport] =
    useState<ReconcileReport | null>(null);

  const workspaceWidth = () =>
    workspaceRef.current?.clientWidth ?? window.innerWidth;
  const leftSidebarMax = Math.max(
    LEFT_SIDEBAR_MIN,
    workspaceWidth() -
      rightSidebarWidth -
      CENTER_PANEL_MIN -
      SIDEBAR_HANDLES_WIDTH,
  );
  const rightSidebarMax = Math.max(
    RIGHT_SIDEBAR_MIN,
    workspaceWidth() -
      leftSidebarWidth -
      CENTER_PANEL_MIN -
      SIDEBAR_HANDLES_WIDTH,
  );

  useEffect(() => {
    try {
      localStorage.setItem("grapes.sidebar.left", String(leftSidebarWidth));
    } catch {
      // Resizing still works when local storage is unavailable.
    }
  }, [leftSidebarWidth]);

  useEffect(() => {
    try {
      localStorage.setItem("grapes.sidebar.right", String(rightSidebarWidth));
    } catch {
      // Resizing still works when local storage is unavailable.
    }
  }, [rightSidebarWidth]);

  useEffect(() => {
    const clampToWindow = () => {
      const width = workspaceWidth();
      setLeftSidebarWidth((current) =>
        Math.min(
          current,
          Math.max(
            LEFT_SIDEBAR_MIN,
            width -
              rightSidebarWidth -
              CENTER_PANEL_MIN -
              SIDEBAR_HANDLES_WIDTH,
          ),
        ),
      );
      setRightSidebarWidth((current) =>
        Math.min(
          current,
          Math.max(
            RIGHT_SIDEBAR_MIN,
            width -
              leftSidebarWidth -
              CENTER_PANEL_MIN -
              SIDEBAR_HANDLES_WIDTH,
          ),
        ),
      );
    };
    clampToWindow();
    window.addEventListener("resize", clampToWindow);
    return () => window.removeEventListener("resize", clampToWindow);
  }, [leftSidebarWidth, rightSidebarWidth]);

  const applySnapshot = useCallback((next: WorkspaceSnapshot) => {
    setSnapshot(next);
    setExpanded((current) => {
      const merged = new Set(current);
      for (const folder of next.folders) merged.add(folder.id);
      return merged;
    });
    setSelectedFolderId((current) => {
      if (current && next.folders.some((folder) => folder.id === current)) {
        return current;
      }
      return next.rootFolderId;
    });
    setSelectedAssetId((current) => {
      if (current && next.assets.some((asset) => asset.id === current)) {
        return current;
      }
      return null;
    });
  }, []);

  useEffect(() => {
    if (!desktopRuntime) return;

    let disposed = false;
    const cleanup: Array<() => void> = [];

    getWorkspaceSnapshot()
      .then((current) => {
        if (!disposed) {
          applySnapshot(current);
        }
      })
      .catch(() => {
        // A fresh launch has no active Workspace until the user opens one.
      });

    Promise.all([
      listen<ReconcileReport>("workspace-changed", async (event) => {
        setWatcherReport(event.payload);
        try {
          applySnapshot(await getWorkspaceSnapshot());
        } catch (caught) {
          setError(normalizeError(caught, t("operationFailed")));
        }
      }),
      listen<string>("workspace-error", (event) => {
        setError(t("watcherError", { error: event.payload }));
      }),
    ]).then((unlisteners) => {
      if (disposed) {
        unlisteners.forEach((unlisten) => unlisten());
      } else {
        cleanup.push(...unlisteners);
      }
    });

    return () => {
      disposed = true;
      cleanup.forEach((unlisten) => unlisten());
    };
  }, [applySnapshot, t]);

  const run = async (
    label: string,
    action: () => Promise<WorkspaceSnapshot>,
  ) => {
    if (busy) return false;
    setBusy(label);
    setError(null);
    try {
      applySnapshot(await action());
      return true;
    } catch (caught) {
      setError(normalizeError(caught, t("operationFailed")));
      return false;
    } finally {
      setBusy(null);
    }
  };

  const openWorkspacePicker = async () => {
    setError(null);
    try {
      const path = await chooseWorkspace(t("openWorkspaceDialog"));
      if (!path) return;
      const opened = await run(t("openingWorkspace"), () =>
        openWorkspace(path),
      );
      if (!opened) return;
      setWatcherReport(null);
      setNotice(t("workspaceOpened"));
      window.setTimeout(() => setNotice(null), 3000);
    } catch (caught) {
      setError(normalizeError(caught, t("operationFailed")));
    }
  };

  const addFolder = async () => {
    if (!snapshot || !selectedFolderId) return;
    const parentFolderId =
      snapshot.assets.find((asset) => asset.id === selectedAssetId)?.folderId ??
      selectedFolderId;
    const name = window.prompt(t("newFolderPrompt"));
    if (!name) return;
    await run(t("creatingFolder"), () =>
      createFolder(parentFolderId, name),
    );
  };

  const renameSelected = async () => {
    if (!snapshot) return;
    if (selectedAssetId) {
      const asset = snapshot.assets.find((item) => item.id === selectedAssetId);
      if (!asset) return;
      const name = window.prompt(t("renamePhotoPrompt"), asset.filename);
      if (!name || name === asset.filename) return;
      await run(t("renamingPhoto"), () => renameAsset(asset.id, name));
      return;
    }
    if (!selectedFolderId || selectedFolderId === snapshot.rootFolderId) return;
    const folder = snapshot.folders.find(
      (item) => item.id === selectedFolderId,
    );
    if (!folder) return;
    const name = window.prompt(t("renameFolderPrompt"), folder.name);
    if (!name || name === folder.name) return;
    await run(t("renamingFolder"), () => renameFolder(folder.id, name));
  };

  const handleDrop = async (
    destinationFolderId: string,
    payload: DragPayload,
  ) => {
    if (payload.type === "folder") {
      const folder = snapshot?.folders.find((item) => item.id === payload.id);
      if (
        !folder ||
        folder.parentId === destinationFolderId ||
        folder.id === destinationFolderId
      ) {
        return;
      }
      await run(t("movingFolder"), () =>
        moveFolder(folder.id, destinationFolderId),
      );
    } else {
      const asset = snapshot?.assets.find((item) => item.id === payload.id);
      if (!asset || asset.folderId === destinationFolderId) return;
      await run(t("movingPhoto"), () =>
        moveAsset(asset.id, destinationFolderId),
      );
    }
  };

  const selectedFolder =
    snapshot?.folders.find((folder) => folder.id === selectedFolderId) ?? null;
  const selectedAsset =
    snapshot?.assets.find((asset) => asset.id === selectedAssetId) ?? null;
  const currentFolderId =
    selectedAsset?.folderId ?? selectedFolderId ?? snapshot?.rootFolderId;
  const currentFolder =
    snapshot?.folders.find((folder) => folder.id === currentFolderId) ?? null;

  const childFolders = useMemo(
    () =>
      snapshot?.folders.filter(
        (folder) => folder.parentId === currentFolderId,
      ) ?? [],
    [snapshot, currentFolderId],
  );

  const deferredFilter = useDeferredValue(filter);
  const searchActive = deferredFilter.trim().length > 0;
  const currentAssets = useMemo(() => {
    const normalizedFilter = deferredFilter.trim().toLocaleLowerCase();
    const source = normalizedFilter ? snapshot?.assets ?? [] : snapshot?.assets.filter(
      (asset) => asset.folderId === currentFolderId,
    ) ?? [];
    return (
      source.filter(
        (asset) =>
          (!normalizedFilter ||
            [
              asset.filename,
              asset.relativePath,
              asset.metadata.cameraMake,
              asset.metadata.cameraModel,
              asset.metadata.lensModel,
              asset.metadata.capturedAt,
              ...asset.tags,
            ]
              .filter(Boolean)
              .join(" ")
              .toLocaleLowerCase()
              .includes(normalizedFilter)),
      ) ?? []
    );
  }, [snapshot, currentFolderId, deferredFilter]);

  const breadcrumbs = useMemo(() => {
    if (!snapshot || !currentFolder) return [];
    const byId = new Map(
      snapshot.folders.map((folder) => [folder.id, folder]),
    );
    const result: FolderView[] = [];
    let folder: FolderView | undefined = currentFolder;
    while (folder) {
      result.unshift(folder);
      folder = folder.parentId ? byId.get(folder.parentId) : undefined;
    }
    return result;
  }, [snapshot, currentFolder]);

  const selectFolder = (folderId: string) => {
    setSelectedFolderId(folderId);
    setSelectedAssetId(null);
    setPhotoOpen(false);
    setGraphOpen(false);
    setFilter("");
  };

  const selectAsset = (asset: AssetView) => {
    setSelectedFolderId(asset.folderId);
    setSelectedAssetId(asset.id);
    setGraphOpen(false);
  };

  const selectGraphAsset = (asset: AssetView) => {
    setSelectedFolderId(asset.folderId);
    setSelectedAssetId(asset.id);
  };

  const openGraphAsset = (asset: AssetView) => {
    selectGraphAsset(asset);
    setPhotoOpen(true);
  };

  const openPhoto = (asset: AssetView) => {
    selectAsset(asset);
    setPhotoOpen(true);
  };

  if (!snapshot) {
    return (
      <div className="welcome-shell">
        <div className="welcome-orb orb-one" />
        <div className="welcome-orb orb-two" />
        <main className="welcome-card">
          <LanguageSwitcher className="welcome-language-switch" />
          <div className="brand-lockup large welcome-brand">
            <BrandMark />
            <div>
              <strong>{t("brandName")}</strong>
            </div>
          </div>
          <button
            type="button"
            className="primary-button open-button"
            onClick={openWorkspacePicker}
            disabled={busy !== null || !desktopRuntime}
          >
            {busy ? (
              <LoaderCircle className="spin" size={18} />
            ) : (
              <FolderIcon size={18} />
            )}
            {busy ?? t("openWorkspace")}
            {!busy && <ArrowRight size={17} />}
          </button>
          {!desktopRuntime && (
            <div className="inline-alert">
              <CircleAlert size={17} />
              {t("desktopOnlyPrefix")}{" "}
              <code>npm run tauri dev</code> {t("desktopOnlySuffix")}
            </div>
          )}
          {error && (
            <div className="inline-alert">
              <CircleAlert size={17} />
              {error}
            </div>
          )}
        </main>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand-lockup">
          <BrandMark />
          <div>
            <strong>{t("brandName")}</strong>
          </div>
        </div>
        <label className="search-box">
          <Search size={16} />
          <input
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            placeholder={t("filterPlaceholder")}
          />
        </label>
        <button
          type="button"
          className="ghost-button graph-button"
          onClick={() => {
            setGraphOpen(true);
            setPhotoOpen(false);
          }}
        >
          <GitBranch size={15} />
          GraphSpaces
          <span>P2</span>
        </button>
        <LanguageSwitcher />
        <button
          type="button"
          className="icon-button"
          title={t("rescanTitle")}
          disabled={busy !== null}
          onClick={() => run(t("rescanBusy"), () => reconcileWorkspace())}
        >
          <RefreshCw
            size={17}
            className={busy === t("rescanBusy") ? "spin" : ""}
          />
        </button>
        <button
          type="button"
          className="icon-button"
          title={t("settings")}
          disabled
        >
          <Settings size={17} />
        </button>
      </header>

      <div
        className="workspace-grid"
        ref={workspaceRef}
        style={{
          gridTemplateColumns: `${leftSidebarWidth}px 5px minmax(${CENTER_PANEL_MIN}px, 1fr) 5px ${rightSidebarWidth}px`,
        }}
      >
        <aside className="files-sidebar">
          <div className="sidebar-heading">
            <div>
              <span className="sidebar-count">
                {t("folderSummary", {
                  folders: snapshot.folders.length - 1,
                  photos: snapshot.assets.length,
                })}
              </span>
            </div>
            <button
              type="button"
              className="icon-button subtle"
              title={t("openOtherWorkspace")}
              onClick={openWorkspacePicker}
            >
              <FolderPlus size={16} />
            </button>
          </div>
          <button
            type="button"
            className={`graphspaces-entry ${graphOpen ? "active" : ""}`}
            onClick={() => {
              setGraphOpen(true);
              setPhotoOpen(false);
            }}
          >
            <span className="entry-orb">
              <Sparkles size={15} />
            </span>
            GraphSpaces
            <span className="phase-chip">P2</span>
          </button>
          <div className="tree-scroll">
            <FileTree
              folders={snapshot.folders}
              assets={snapshot.assets}
              rootFolderId={snapshot.rootFolderId}
              selectedFolderId={selectedFolderId}
              selectedAssetId={selectedAssetId}
              expanded={expanded}
              onToggle={(folderId) =>
                setExpanded((current) => {
                  const next = new Set(current);
                  if (next.has(folderId)) next.delete(folderId);
                  else next.add(folderId);
                  return next;
                })
              }
              onSelectFolder={selectFolder}
              onSelectAsset={selectAsset}
              onDrop={handleDrop}
            />
          </div>
          <div className="sidebar-bottom">
            <div className="ask-grapes">
              <div className="ask-heading">
                <span className="ask-icon">
                  <Sparkles size={15} />
                </span>
                <strong>Ask Grapes</strong>
                <span className="status-dot" />
                <span className="provider-model">Qwen3.5-0.8B</span>
              </div>
              <div className="ask-input">
                {t("askPlaceholder")}
              </div>
            </div>
          </div>
        </aside>

        <SidebarResizer
          side="left"
          width={leftSidebarWidth}
          min={LEFT_SIDEBAR_MIN}
          max={leftSidebarMax}
          defaultWidth={264}
          onChange={setLeftSidebarWidth}
        />

        <main
          ref={mainWorkspaceRef}
          className={`main-workspace ${
            photoOpen && selectedAsset
              ? "photo-mode"
              : graphOpen
                ? "graph-mode"
                : ""
          }`}
        >
          {photoOpen && selectedAsset ? (
            <PhotoWorkspace
              asset={selectedAsset}
              assets={currentAssets}
              onSelect={selectAsset}
              onClose={() => setPhotoOpen(false)}
            />
          ) : graphOpen ? (
            <Suspense
              fallback={
                <div className="graph-loading">
                  <LoaderCircle size={20} className="spin" />
                  {t("graphLoading")}
                </div>
              }
            >
              <GraphWorkspace
                assets={snapshot.assets}
                refreshToken={
                  watcherReport?.finishedAt ??
                  snapshot.lastReconcile?.finishedAt ??
                  snapshot.workspace.createdAt
                }
                onClose={() => setGraphOpen(false)}
                onSelectAsset={selectGraphAsset}
                onOpenAsset={openGraphAsset}
                onError={setError}
              />
            </Suspense>
          ) : (
            <>
          <div className="content-toolbar">
            <nav className="breadcrumbs" aria-label={t("currentLocation")}>
              {breadcrumbs.map((folder, index) => (
                <span key={folder.id}>
                  {index > 0 && <ChevronRight size={13} />}
                  <button
                    type="button"
                    onClick={() => selectFolder(folder.id)}
                    className={
                      folder.id === currentFolderId ? "current" : ""
                    }
                  >
                    {index === 0 ? (
                      <HardDrive size={14} />
                    ) : (
                      <FolderIcon size={14} />
                    )}
                    {folder.name}
                  </button>
                </span>
              ))}
            </nav>
            <div className="content-actions">
              <label
                className="folder-view-control"
                title={t("folderView")}
              >
                <select
                  aria-label={t("folderView")}
                  value={folderView}
                  onChange={(event) =>
                    setFolderView(
                      event.target.value as "medium" | "small" | "details",
                    )
                  }
                >
                  <option value="medium">{t("mediumIcons")}</option>
                  <option value="small">{t("smallIcons")}</option>
                  <option value="details">{t("details")}</option>
                </select>
              </label>
              <button
                type="button"
                className="secondary-button"
                title={t("newFolder")}
                onClick={addFolder}
                disabled={busy !== null}
              >
                <FolderPlus size={15} />
              </button>
              <button
                type="button"
                className="secondary-button"
                title={t("rename")}
                onClick={renameSelected}
                disabled={
                  busy !== null ||
                  (!selectedAssetId &&
                    (!selectedFolderId ||
                      selectedFolderId === snapshot.rootFolderId))
                }
              >
                <Pencil size={15} />
              </button>
            </div>
          </div>

          <section className="library-header">
            <div>
              <h3>
                {searchActive
                  ? t("searchSummary", { count: currentAssets.length })
                  : absoluteWorkspacePath(
                      snapshot.workspace.rootPath,
                      currentFolder?.relativePath ?? "",
                    )}
              </h3>
            </div>
            <div className="library-stats">
              <Stat
                icon={<FolderIcon size={16} />}
                value={childFolders.length}
                label=""
              />
              <Stat
                icon={<FileImage size={16} />}
                value={currentAssets.length}
                label=""
              />
            </div>
          </section>

          {currentAssets.length === 0 ? (
            <div className="empty-state">
              <div className="empty-icon">
                <FileImage size={25} />
              </div>
              <h3>{filter ? t("noMatches") : t("emptyFolder")}</h3>
              <p>{filter ? t("clearFilter") : t("emptyFolderHint")}</p>
            </div>
          ) : folderView !== "details" ? (
            <VirtualGallery
              assets={currentAssets}
              selectedAssetId={selectedAssetId}
              onSelect={selectAsset}
              onOpen={openPhoto}
              compact={folderView === "small"}
              scrollElementRef={mainWorkspaceRef}
            />
          ) : (
            <div className="asset-table" role="table">
              <div className="asset-table-header" role="row">
                <span>{t("name")}</span>
                <span>{t("tags")}</span>
                <span>{t("modified")}</span>
                <span>{t("size")}</span>
              </div>
              {currentAssets.map((asset) => (
                <button
                  type="button"
                  role="row"
                  className={`asset-row ${
                    selectedAssetId === asset.id ? "selected" : ""
                  }`}
                  key={asset.id}
                  draggable
                  onClick={() => selectAsset(asset)}
                  onDoubleClick={() => openPhoto(asset)}
                  onDragStart={(event) => {
                    event.dataTransfer.effectAllowed = "move";
                    event.dataTransfer.setData(
                      "application/x-grapes",
                      JSON.stringify({ type: "asset", id: asset.id }),
                    );
                  }}
                >
                  <span className="asset-name">
                    <AssetThumbnail asset={asset} />
                    <span>
                      <strong>{asset.filename}</strong>
                      <small>{asset.relativePath}</small>
                    </span>
                  </span>
                  <AssetTags tags={asset.tags} />
                  <span className="asset-modified">
                    {formatModifiedAt(asset.mtimeNs, locale, t("unknown"))}
                  </span>
                  <span>{formatBytes(asset.fileSize)}</span>
                </button>
              ))}
            </div>
          )}
            </>
          )}
        </main>

        <SidebarResizer
          side="right"
          width={rightSidebarWidth}
          min={RIGHT_SIDEBAR_MIN}
          max={rightSidebarMax}
          defaultWidth={316}
          onChange={setRightSidebarWidth}
        />

        <Inspector
          asset={selectedAsset}
          folder={currentFolder}
          assetCount={
            snapshot.assets.filter(
              (asset) => asset.folderId === currentFolderId,
            ).length
          }
          folderCount={childFolders.length}
          workspaceRootPath={snapshot.workspace.rootPath}
          tagBusy={busy !== null}
          onSetTags={async (tags) => {
            if (!selectedAsset) return false;
            return run(t("updatingTags"), () =>
              setAssetTags(selectedAsset.id, tags),
            );
          }}
        />
      </div>

      <footer className="statusbar">
        <span>
          <CheckCircle2 size={13} />
          {busy ?? t("indexSynced")}
        </span>
        <span className="status-spacer" />
        {(watcherReport ?? snapshot.lastReconcile) && (
          <span className="last-reconcile">
            {t("lastUpdated")}
            {new Date(
              (watcherReport ?? snapshot.lastReconcile)!.finishedAt,
            ).toLocaleTimeString(locale === "zh" ? "zh-CN" : "en-US", {
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
            })}
          </span>
        )}
        <span className="status-security">
          <ShieldCheck size={13} />
          {t("localFirst")}
        </span>
      </footer>

      {(error || notice) && (
        <div className={`toast ${error ? "toast-error" : "toast-success"}`}>
          {error ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}
          <span>{error ?? notice}</span>
          <button
            type="button"
            onClick={() => {
              setError(null);
              setNotice(null);
            }}
          >
            ×
          </button>
        </div>
      )}
    </div>
  );
}

function storedSidebarWidth(key: string, fallback: number, min: number) {
  try {
    const value = Number(localStorage.getItem(key));
    return Number.isFinite(value) && value > 0 ? Math.max(min, value) : fallback;
  } catch {
    return fallback;
  }
}

function BrandMark() {
  return (
    <span className="brand-mark" aria-hidden="true">
      <span className="grape-leaf" />
      <span className="grape-body">
        <i />
        <i />
        <i />
        <i />
        <i />
      </span>
    </span>
  );
}

function WelcomeFeature({
  icon,
  title,
  text,
}: {
  icon: ReactNode;
  title: string;
  text: string;
}) {
  return (
    <div className="welcome-feature">
      <span>{icon}</span>
      <strong>{title}</strong>
      <p>{text}</p>
    </div>
  );
}

function Stat({
  icon,
  value,
  label,
}: {
  icon: ReactNode;
  value: number | string;
  label: string;
}) {
  return (
    <div className="stat">
      {icon}
      <span>
        <strong>{value}</strong>
        <small>{label}</small>
      </span>
    </div>
  );
}

function formatModifiedAt(
  mtimeNs: number,
  locale: "zh" | "en",
  unknown: string,
) {
  const date = new Date(Math.floor(mtimeNs / 1_000_000));
  if (Number.isNaN(date.getTime())) return unknown;
  return date.toLocaleString(locale === "zh" ? "zh-CN" : "en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function normalizeError(error: unknown, fallback: string) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return fallback;
}
