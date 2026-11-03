import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Grape, Minus, RefreshCw, Settings, Square, X } from "lucide-react";
import { desktopRuntime } from "../api/workspace";
import { useI18n } from "../i18n";
import { LanguageSwitcher } from "./LanguageSwitcher";

interface TitleBarProps {
  workspaceOpen: boolean;
  busy: boolean;
  refreshing: boolean;
  onSettings: () => void;
  onRefresh: () => void;
  onError: (error: unknown) => void;
}

export function TitleBar({
  workspaceOpen, busy, refreshing, onSettings, onRefresh, onError,
}: TitleBarProps) {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!desktopRuntime) return;
    const appWindow = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const updateMaximized = () => {
      appWindow.isMaximized().then((value) => {
        if (!disposed) setMaximized(value);
      }).catch(onError);
    };
    updateMaximized();
    appWindow.onResized(updateMaximized).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch(onError);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onError]);

  return (
    <header className="titlebar">
      <span className="titlebar-brand" aria-hidden="true"><Grape size={19} /></span>
      <nav className="titlebar-actions" aria-label={t("appMenu")}>
        <button type="button" className="titlebar-action" title={t("semanticProviderSettings")}
          disabled={!workspaceOpen} onClick={onSettings}>
          <Settings size={14} aria-hidden="true" />
        </button>
        <button type="button" className="titlebar-action" title={t("rescanTitle")}
          disabled={!workspaceOpen || busy} onClick={onRefresh}>
          <RefreshCw size={14} aria-hidden="true" className={refreshing ? "spin" : ""} />
        </button>
        <LanguageSwitcher className="titlebar-language" />
      </nav>
      <div className="titlebar-drag-region" data-tauri-drag-region>
        
      </div>
      {desktopRuntime && (
        <div className="window-controls">
          <button type="button" title={t("minimizeWindow")} aria-label={t("minimizeWindow")}
            onClick={() => getCurrentWindow().minimize().catch(onError)}>
            <Minus size={14} aria-hidden="true" />
          </button>
          <button type="button" title={t(maximized ? "restoreWindow" : "maximizeWindow")}
            aria-label={t(maximized ? "restoreWindow" : "maximizeWindow")}
            onClick={() => getCurrentWindow().toggleMaximize().catch(onError)}>
            {maximized ? <Copy size={12} aria-hidden="true" /> : <Square size={12} aria-hidden="true" />}
          </button>
          <button type="button" className="window-close" title={t("closeWindow")} aria-label={t("closeWindow")}
            onClick={() => getCurrentWindow().close().catch(onError)}>
            <X size={16} aria-hidden="true" />
          </button>
        </div>
      )}
    </header>
  );
}
