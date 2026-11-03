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
  FileImage,
  Folder as FolderIcon,
  FolderPlus,
  HardDrive,
  LoaderCircle,
  Pencil,
  Search,
  ShieldCheck,
  Sparkles,
  Square,
} from "lucide-react";
import {
  cancelAskGrapes,
  chooseWorkspace,
  createFolder,
  createCustomDimension,
  createDimensionValue,
  desktopRuntime,
  getClassificationSchema,
  getSemanticSettings,
  getWorkspaceSnapshot,
  moveAsset,
  moveFolder,
  openWorkspace,
  reconcileWorkspace,
  renameAsset,
  renameFolder,
  semanticSearch,
  setAssetClassification,
  setAssetTags,
} from "./api/workspace";
import type {
  AskGrapesProgress,
  AssetView,
  DragPayload,
  Dimension,
  FolderView,
  ReconcileReport,
  SemanticSearchResponse,
  WorkspaceSnapshot,
} from "./bindings/types";
import { FileTree } from "./components/FileTree";
import { Inspector } from "./components/Inspector";
import { AssetTags } from "./components/AssetTags";
import { AssetThumbnail } from "./components/AssetThumbnail";
import { TitleBar } from "./components/TitleBar";
import { useI18n } from "./i18n";
import {
  classificationValueDisplayName,
  dimensionDisplayName,
} from "./classificationLabels";
import { VirtualGallery } from "./components/VirtualGallery";
import { PhotoWorkspace } from "./components/PhotoWorkspace";
import { SemanticSearchPanel } from "./components/SemanticSearchPanel";
import { absoluteWorkspacePath } from "./utils/paths";
import {
  formatBytes,
  formatModifiedAt,
  normalizeError,
} from "./utils/format";
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
  const [graphId, setGraphId] = useState("graphspaces");
  const [graphPhotoIds, setGraphPhotoIds] = useState<string[]>([]);
  const [watcherReport, setWatcherReport] =
    useState<ReconcileReport | null>(null);
  const [classificationSchema, setClassificationSchema] = useState<Dimension[]>(
    [],
  );
  const [graphRevision, setGraphRevision] = useState(0);
  const [askGrapesModel, setAskGrapesModel] = useState("Qwen/Qwen3.5-4B");
  const [semanticPanel, setSemanticPanel] = useState<
    "results" | "settings" | null
  >(null);
  const [askQuery, setAskQuery] = useState("");
  const [semanticResponse, setSemanticResponse] =
    useState<SemanticSearchResponse | null>(null);
  const [semanticSearching, setSemanticSearching] = useState(false);
  const [askProgress, setAskProgress] = useState<AskGrapesProgress[]>([]);
  const [askProgressStartedAt, setAskProgressStartedAt] = useState<
    number | null
  >(null);
  const [askProgressClock, setAskProgressClock] = useState(0);
  const [askDebugOpen, setAskDebugOpen] = useState(true);
  const activeAskRequest = useRef<string | null>(null);
  const askCancelRequested = useRef(false);

  useEffect(() => {
    setGraphId("graphspaces");
    setGraphPhotoIds([]);
    setSemanticPanel(null);
    setSemanticResponse(null);
    setAskQuery("");
    setAskProgress([]);
    setAskProgressStartedAt(null);
    setAskDebugOpen(true);
    activeAskRequest.current = null;
    askCancelRequested.current = false;
  }, [snapshot?.workspace.id]);

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

  useEffect(() => {
    if (!snapshot) return;
    let disposed = false;
    getClassificationSchema()
      .then((schema) => {
        if (!disposed) setClassificationSchema(schema);
      })
      .catch((caught) => {
        if (!disposed) setError(normalizeError(caught, t("operationFailed")));
      });
    return () => {
      disposed = true;
    };
  }, [snapshot?.workspace.id, t]);

  useEffect(() => {
    if (!snapshot) return;
    let disposed = false;
    getSemanticSettings()
      .then((settings) => {
        if (!disposed) setAskGrapesModel(settings.qa.model);
      })
      .catch(() => {
        // The sidebar retains the documented default before settings load.
      });
    return () => {
      disposed = true;
    };
  }, [snapshot?.workspace.id]);

  useEffect(() => {
    if (!semanticSearching || askProgressStartedAt == null) return;
    const update = () =>
      setAskProgressClock(Date.now() - askProgressStartedAt);
    update();
    const timer = window.setInterval(update, 250);
    return () => window.clearInterval(timer);
  }, [askProgressStartedAt, semanticSearching]);

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
      listen<AskGrapesProgress>("ask-grapes-progress", (event) => {
        if (event.payload.requestId !== activeAskRequest.current) return;
        setAskProgress((current) => [...current, event.payload].slice(-10));
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

  const openGraphAsset = (asset: AssetView, memberIds: string[]) => {
    setGraphPhotoIds(memberIds);
    selectGraphAsset(asset);
    setPhotoOpen(true);
  };

  const openPhoto = (asset: AssetView) => {
    selectAsset(asset);
    setPhotoOpen(true);
  };

  const submitAskSearch = async () => {
    const query = askQuery.trim();
    if (!query || semanticSearching) return;
    const requestId =
      globalThis.crypto?.randomUUID?.() ??
      `ask-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const startedAt = Date.now();
    activeAskRequest.current = requestId;
    askCancelRequested.current = false;
    setAskProgress([]);
    setAskProgressStartedAt(startedAt);
    setAskProgressClock(0);
    setAskDebugOpen(true);
    setSemanticSearching(true);
    setError(null);
    try {
      setSemanticResponse(await semanticSearch(query, 20, requestId));
      setSemanticPanel("results");
    } catch (caught) {
      const message = normalizeError(caught, t("operationFailed"));
      if (askCancelRequested.current) {
        setAskProgress((current) => [
          ...current,
          {
            requestId,
            stage: "cancelled",
            state: "warning",
            message: t("askCancelled"),
            detail: null,
            elapsedMs: Date.now() - startedAt,
          },
        ]);
        return;
      }
      setAskProgress((current) => [
        ...current,
        {
          requestId,
          stage: "error",
          state: "error",
          message: t("askRequestFailed"),
          detail: message,
          elapsedMs: Date.now() - startedAt,
        },
      ]);
      setError(message);
    } finally {
      setSemanticSearching(false);
    }
  };

  const stopAskSearch = async () => {
    const requestId = activeAskRequest.current;
    if (!requestId || !semanticSearching) return;
    askCancelRequested.current = true;
    setAskProgress((current) => [
      ...current,
      {
        requestId,
        stage: "cancelled",
        state: "warning",
        message: t("askStopping"),
        detail: null,
        elapsedMs: askProgressClock,
      },
    ]);
    try {
      const cancelled = await cancelAskGrapes(requestId);
      if (!cancelled) {
        setError(t("askStopFailed"));
      }
    } catch (caught) {
      setError(normalizeError(caught, t("askStopFailed")));
    }
  };

  const createClassificationDimension = async (name: string) => {
    if (busy) return false;
    setBusy(t("creatingDimension"));
    setError(null);
    try {
      setClassificationSchema(await createCustomDimension(name));
      setGraphRevision((value) => value + 1);
      return true;
    } catch (caught) {
      setError(normalizeError(caught, t("operationFailed")));
      return false;
    } finally {
      setBusy(null);
    }
  };

  const addClassificationValue = async (
    dimensionId: string,
    name: string,
  ) => {
    if (busy) return false;
    setBusy(t("creatingClassificationValue"));
    setError(null);
    try {
      setClassificationSchema(
        await createDimensionValue(dimensionId, name),
      );
      setGraphRevision((value) => value + 1);
      return true;
    } catch (caught) {
      setError(normalizeError(caught, t("operationFailed")));
      return false;
    } finally {
      setBusy(null);
    }
  };

  const updateClassification = async (
    assetId: string,
    valueId: string,
    selected: boolean,
  ) => {
    const updated = await run(t("updatingClassification"), () =>
      setAssetClassification(assetId, valueId, selected),
    );
    if (updated) setGraphRevision((value) => value + 1);
    return updated;
  };

  const handleWindowError = useCallback((error: unknown) => {
    setError(normalizeError(error, t("operationFailed")));
  }, [t]);

  const titlebar = (
    <TitleBar
      workspaceOpen={snapshot !== null}
      busy={busy !== null}
      refreshing={busy === t("rescanBusy")}
      onSettings={() => setSemanticPanel("settings")}
      onRefresh={() => { void run(t("rescanBusy"), () => reconcileWorkspace()); }}
      onError={handleWindowError}
    />
  );

  if (!snapshot) {
    return (
      <div className="app-frame">
        {titlebar}
        <div className="welcome-shell">
        <div className="welcome-orb orb-one" />
        <div className="welcome-orb orb-two" />
        <main className="welcome-card">
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
      </div>
    );
  }

  return (
    <div className="app-frame">
      {titlebar}
      <div className="app-shell">

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
          <label className="search-box sidebar-search-box">
            <Search size={14} />
            <input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={t("filterPlaceholder")}
            />
          </label>
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
            <form
              className="ask-grapes"
              onSubmit={(event) => {
                event.preventDefault();
                void submitAskSearch();
              }}
            >
              <span className="ask-heading">
                <span className="ask-icon">
                  <Sparkles size={15} />
                </span>
                <strong>Ask Grapes</strong>
                <span className="status-dot" />
                <span className="provider-model">{askGrapesModel}</span>
              </span>
              {(semanticSearching || askProgress.length > 0) && (
                <span
                  className={`ask-debug-panel ${
                    askDebugOpen ? "open" : "collapsed"
                  }`}
                  aria-live="polite"
                >
                  <span className="ask-debug-heading">
                    <button
                      type="button"
                      className="ask-debug-toggle"
                      title={
                        askDebugOpen
                          ? t("askDebugCollapse")
                          : t("askDebugExpand")
                      }
                      aria-label={
                        askDebugOpen
                          ? t("askDebugCollapse")
                          : t("askDebugExpand")
                      }
                      aria-expanded={askDebugOpen}
                      onClick={() => setAskDebugOpen((open) => !open)}
                    >
                      <ChevronRight size={11} />
                    </button>
                    <strong>{t("askDebugTitle")}</strong>
                    <time>
                      {(
                        (semanticSearching
                          ? askProgressClock
                          : askProgress.at(-1)?.elapsedMs ?? 0) / 1000
                      ).toFixed(1)}
                      s
                    </time>
                  </span>
                  {askDebugOpen && (
                    <span className="ask-debug-list">
                      {askProgress.slice(-6).map((progress, index) => (
                        <span
                          className={`ask-debug-item ${progress.state}`}
                          key={`${progress.requestId}:${progress.stage}:${index}`}
                        >
                          <b>{progress.stage.toUpperCase()}</b>
                          <span>
                            <span>{progress.message}</span>
                            {progress.detail && (
                              <small>{progress.detail}</small>
                            )}
                          </span>
                        </span>
                      ))}
                    </span>
                  )}
                </span>
              )}
              <span className="ask-input">
                <input
                  value={askQuery}
                  onChange={(event) => setAskQuery(event.target.value)}
                  placeholder={t("askPlaceholder")}
                  aria-label={t("semanticSearch")}
                  disabled={semanticSearching}
                />
                <button
                  type="submit"
                  title={t("semanticFind")}
                  aria-label={t("semanticFind")}
                  disabled={semanticSearching || !askQuery.trim()}
                >
                  {semanticSearching ? (
                    <LoaderCircle className="spin" size={13} />
                  ) : (
                    <Search size={13} />
                  )}
                </button>
                {semanticSearching && (
                  <button
                    type="button"
                    className="ask-stop-button"
                    title={t("askStop")}
                    aria-label={t("askStop")}
                    onClick={() => void stopAskSearch()}
                  >
                    <Square size={10} fill="currentColor" />
                  </button>
                )}
              </span>
            </form>
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
              assets={graphOpen ? graphPhotoIds.flatMap((id) => {
                const asset = snapshot.assets.find((item) => item.id === id);
                return asset ? [asset] : [];
              }) : currentAssets}
              onSelect={graphOpen ? selectGraphAsset : selectAsset}
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
                graphId={graphId}
                onNavigate={setGraphId}
                workspaceId={snapshot.workspace.id}
                assets={snapshot.assets}
                refreshToken={
                `${watcherReport?.finishedAt ?? snapshot.lastReconcile?.finishedAt ?? snapshot.workspace.createdAt}:${graphRevision}`
                }
                onClose={() => setGraphOpen(false)}
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
                <span>{t("classifications")}</span>
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
                    </span>
                  </span>
                  <AssetTags tags={asset.tags} />
                  <AssetClassifications
                    asset={asset}
                    locale={locale}
                    unclassified={t("unclassified")}
                  />
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
          classificationSchema={classificationSchema}
          classificationBusy={busy !== null}
          onSetClassification={updateClassification}
          onCreateClassificationDimension={createClassificationDimension}
          onCreateClassificationValue={addClassificationValue}
          tagBusy={busy !== null}
          onSetTags={async (tags) => {
            if (!selectedAsset) return false;
            const updated = await run(t("updatingTags"), () =>
              setAssetTags(selectedAsset.id, tags),
            );
            if (updated) setGraphRevision((value) => value + 1);
            return updated;
          }}
        />
      </div>

      {semanticPanel && (
        <SemanticSearchPanel
          assets={snapshot.assets}
          initialTab={semanticPanel}
          initialResponse={semanticResponse}
          onClose={() => setSemanticPanel(null)}
          onOpenAsset={(asset) => {
            selectAsset(asset);
            setPhotoOpen(true);
            setSemanticPanel(null);
          }}
          onError={setError}
          onAskModelChange={setAskGrapesModel}
        />
      )}

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

function AssetClassifications({
  asset,
  locale,
  unclassified,
}: {
  asset: AssetView;
  locale: "zh" | "en";
  unclassified: string;
}) {
  if (asset.classifications.length === 0) {
    return (
      <span className="asset-classifications empty">{unclassified}</span>
    );
  }

  const labels = asset.classifications.map((classification) => {
    const dimension = dimensionDisplayName(
      classification.dimensionId,
      classification.dimensionName,
      locale,
    );
    const value = classificationValueDisplayName(
      classification.valueId,
      classification.valueName,
      locale,
    );
    return {
      ...classification,
      label: `${dimension}: ${value}`,
      dimension,
      value,
    };
  });
  const visible = labels.slice(0, 2);
  const hiddenCount = labels.length - visible.length;

  return (
    <span
      className="asset-classifications"
      title={labels.map(({ label }) => label).join(" · ")}
    >
      {visible.map(({ dimensionId, valueId, dimension, value }) => (
        <span
          className="asset-classification"
          key={`${dimensionId}:${valueId}`}
        >
          <small>{dimension}</small>
          <strong>{value}</strong>
        </span>
      ))}
      {hiddenCount > 0 && (
        <span className="asset-classification-more">+{hiddenCount}</span>
      )}
    </span>
  );
}
