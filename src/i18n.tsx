import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import type { ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type Locale = "zh" | "en";

const zh = {
  brandName: "葡萄相簿",
  language: "语言",
  chinese: "中文",
  english: "EN",
  openWorkspace: "打开照片 Workspace",
  openingWorkspace: "正在打开并建立索引",
  desktopOnlyPrefix: "请通过",
  desktopOnlySuffix: "启动桌面应用。",
  openWorkspaceDialog: "打开 Grapes Workspace",
  workspaceOpened: "Workspace 已打开，文件变化将自动同步。",
  watcherError: "文件监听器：{error}",
  filterPlaceholder: "按名称或标签筛选...",
  rescanTitle: "重新扫描并对账",
  rescanBusy: "正在对账文件系统",
  settings: "设置（P1）",
  folderSummary: "{folders} 个目录 · {photos} 张照片",
  openOtherWorkspace: "打开其他 Workspace",
  currentLocation: "当前位置",
  askPlaceholder: "开始跟葡萄架对话…",
  newFolder: "新建文件夹",
  newFolderPrompt: "新建文件夹名称",
  creatingFolder: "正在新建文件夹",
  rename: "重命名",
  renamePhotoPrompt: "重命名照片",
  renamingPhoto: "正在重命名照片",
  renameFolderPrompt: "重命名文件夹",
  renamingFolder: "正在重命名文件夹",
  movingFolder: "正在移动文件夹",
  movingPhoto: "正在移动照片",
  photoCount: "{count} 张照片",
  fileList: "文件列表",
  gallery: "图库",
  list: "列表",
  showGallery: "切换到虚拟图库",
  showList: "切换到文件列表",
  folderView: "文件夹显示方式",
  mediumIcons: "中图标",
  smallIcons: "小图标",
  details: "详细信息",
  folderType: "类型",
  items: "项目数",
  searchResults: "搜索结果",
  searchSummary: "找到 {count} 张匹配照片",
  fit: "适合窗口",
  actualSize: "原始尺寸",
  closePhoto: "关闭照片",
  previewUnavailable: "当前格式无法在应用内预览，可使用缩略图继续浏览。",
  watcherLive: "实时同步",
  noMatches: "没有匹配的照片",
  emptyFolder: "这个目录还没有照片",
  clearFilter: "尝试清空当前筛选条件。",
  emptyFolderHint: "把支持的图片放入当前文件夹，变化会自动纳入索引。",
  name: "名称",
  tags: "标签",
  modified: "修改时间",
  size: "大小",
  updatingTags: "正在更新标签",
  indexSynced: "文件索引已同步",
  lastUpdated: "上次更新：",
  localFirst: "本地优先",
  unknown: "未知",
  operationFailed: "操作失败，请检查文件系统权限后重试。",
  inspector: "详细信息",
  photoProperties: "照片属性",
  histogram: "直方图",
  tagPrompt: "添加标签，多个标签请用逗号分隔",
  removeTag: "移除标签 {tag}",
  addTag: "添加标签",
  noTagsYet: "还没有标签",
  noTags: "暂无标签",
  fileInfo: "文件信息",
  location: "所在位置",
  format: "格式",
  fileSize: "文件大小",
  modifiedAt: "修改时间",
  cameraMetadata: "拍摄信息",
  camera: "相机",
  lens: "镜头",
  focalLength: "焦距",
  capturedAt: "拍摄时间",
  dimensions: "尺寸",
  exposure: "快门",
  aperture: "光圈",
  iso: "ISO",
  gps: "GPS",
  folder: "文件夹",
  workspaceRoot: "Workspace 根目录",
  contents: "内容",
  photoTotal: "照片总数",
  subfolders: "子文件夹",
  photosCount: "{count} 张",
  foldersCount: "{count} 个",
  fileTree: "Workspace 文件树",
  windowTitle: "葡萄相簿",
} as const;

type MessageKey = keyof typeof zh;

const en: Record<MessageKey, string> = {
  brandName: "Grapes Album",
  language: "Language",
  chinese: "中文",
  english: "EN",
  openWorkspace: "Open Photo Workspace",
  openingWorkspace: "Opening and indexing",
  desktopOnlyPrefix: "Launch the desktop app with",
  desktopOnlySuffix: ".",
  openWorkspaceDialog: "Open Grapes Workspace",
  workspaceOpened: "Workspace opened. File changes will sync automatically.",
  watcherError: "File watcher: {error}",
  filterPlaceholder: "Filter by name or tag...",
  rescanTitle: "Rescan workspace",
  rescanBusy: "Reconciling files",
  settings: "Settings (P1)",
  folderSummary: "{folders} folders · {photos} photos",
  openOtherWorkspace: "Open another workspace",
  currentLocation: "Current location",
  askPlaceholder: "Start a conversation with Grapes…",
  newFolder: "New folder",
  newFolderPrompt: "New folder name",
  creatingFolder: "Creating folder",
  rename: "Rename",
  renamePhotoPrompt: "Rename photo",
  renamingPhoto: "Renaming photo",
  renameFolderPrompt: "Rename folder",
  renamingFolder: "Renaming folder",
  movingFolder: "Moving folder",
  movingPhoto: "Moving photo",
  photoCount: "{count} photos",
  fileList: "Files",
  gallery: "Gallery",
  list: "List",
  showGallery: "Switch to gallery",
  showList: "Switch to list",
  folderView: "Folder view",
  mediumIcons: "Medium icons",
  smallIcons: "Small icons",
  details: "Details",
  folderType: "Type",
  items: "Items",
  searchResults: "Search results",
  searchSummary: "{count} matching photos",
  fit: "Fit",
  actualSize: "Actual size",
  closePhoto: "Close photo",
  previewUnavailable: "This format cannot be previewed here. The thumbnail remains available.",
  watcherLive: "LIVE SYNC",
  noMatches: "No matching photos",
  emptyFolder: "No photos in this folder",
  clearFilter: "Try clearing the current filter.",
  emptyFolderHint: "Add supported images here and they will be indexed automatically.",
  name: "Name",
  tags: "Tags",
  modified: "Modified",
  size: "Size",
  updatingTags: "Updating tags",
  indexSynced: "File index is up to date",
  lastUpdated: "Last updated: ",
  localFirst: "Local-first",
  unknown: "Unknown",
  operationFailed: "The operation failed. Check filesystem permissions and retry.",
  inspector: "INSPECTOR",
  photoProperties: "Photo details",
  histogram: "Histogram",
  tagPrompt: "Add tags separated by commas",
  removeTag: "Remove tag {tag}",
  addTag: "Add tag",
  noTagsYet: "No tags yet",
  noTags: "No tags",
  fileInfo: "File",
  location: "Location",
  format: "Format",
  fileSize: "File size",
  modifiedAt: "Modified",
  cameraMetadata: "Capture details",
  camera: "Camera",
  lens: "Lens",
  focalLength: "Focal length",
  capturedAt: "Captured",
  dimensions: "Dimensions",
  exposure: "Exposure",
  aperture: "Aperture",
  iso: "ISO",
  gps: "GPS",
  folder: "Folder",
  workspaceRoot: "Workspace root",
  contents: "Contents",
  photoTotal: "Photos here",
  subfolders: "Subfolders",
  photosCount: "{count} photos",
  foldersCount: "{count} folders",
  fileTree: "Workspace file tree",
  windowTitle: "Grapes Album",
};

const messages: Record<Locale, Record<MessageKey, string>> = { zh, en };

interface I18nContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: MessageKey, values?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);
const STORAGE_KEY = "grapes.locale";

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => {
    try {
      return localStorage.getItem(STORAGE_KEY) === "en" ? "en" : "zh";
    } catch {
      return "zh";
    }
  });

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // The in-memory selection still works when storage is unavailable.
    }
  }, []);

  const t = useCallback(
    (key: MessageKey, values?: Record<string, string | number>) => {
      let message: string = messages[locale][key];
      for (const [name, value] of Object.entries(values ?? {})) {
        message = message.replaceAll(`{${name}}`, String(value));
      }
      return message;
    },
    [locale],
  );

  useEffect(() => {
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
    const title = t("windowTitle");
    document.title = title;
    if (isTauri()) {
      void getCurrentWindow().setTitle(title).catch(() => {
        // The document title still follows the selected language.
      });
    }
  }, [locale, t]);

  const value = useMemo(
    () => ({ locale, setLocale, t }),
    [locale, setLocale, t],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const value = useContext(I18nContext);
  if (!value) {
    throw new Error("useI18n must be used inside I18nProvider");
  }
  return value;
}
