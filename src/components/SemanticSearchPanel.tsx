import { useEffect, useState } from "react";
import {
  Database,
  Eye,
  EyeOff,
  FlaskConical,
  LoaderCircle,
  Search,
  Settings2,
  X,
} from "lucide-react";
import {
  getSemanticSettings,
  rebuildSemanticIndex,
  saveSemanticProvider,
  testSemanticProvider,
} from "../api/workspace";
import type {
  AssetView,
  SemanticProviderConfig,
  SemanticSearchResponse,
  SemanticSettings,
} from "../bindings/types";
import { useI18n } from "../i18n";
import { normalizeError } from "../utils/format";
import { AssetThumbnail } from "./AssetThumbnail";

type SemanticTab = "results" | "settings";
type ProviderKind = "qa" | "embedding" | "rerank";

export function SemanticSearchPanel({
  assets,
  initialTab,
  initialResponse,
  onClose,
  onOpenAsset,
  onError,
  onAskModelChange,
}: {
  assets: AssetView[];
  initialTab: SemanticTab;
  initialResponse: SemanticSearchResponse | null;
  onClose: () => void;
  onOpenAsset: (asset: AssetView) => void;
  onError: (message: string) => void;
  onAskModelChange: (model: string) => void;
}) {
  const { t } = useI18n();
  const [tab, setTab] = useState<SemanticTab>(initialTab);
  const [settings, setSettings] = useState<SemanticSettings | null>(null);
  const [visibleKeys, setVisibleKeys] = useState({
    qa: false,
    embedding: false,
    rerank: false,
  });
  const [response, setResponse] =
    useState<SemanticSearchResponse | null>(initialResponse);
  const [loadingSettings, setLoadingSettings] = useState(true);
  const [savingKind, setSavingKind] = useState<string | null>(null);
  const [testingKind, setTestingKind] = useState<string | null>(null);
  const [indexing, setIndexing] = useState(false);
  const [providerMessage, setProviderMessage] = useState("");
  const [notice, setNotice] = useState("");

  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    setResponse(initialResponse);
  }, [initialResponse]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  useEffect(() => {
    let disposed = false;
    setLoadingSettings(true);
    getSemanticSettings()
      .then((next) => {
        if (!disposed) {
          setSettings(next);
          onAskModelChange(next.qa.model);
        }
      })
      .catch((caught) => {
        if (!disposed) onError(normalizeError(caught, "Semantic search failed."));
      })
      .finally(() => {
        if (!disposed) setLoadingSettings(false);
      });
    return () => {
      disposed = true;
    };
  }, [onAskModelChange, onError]);

  const updateProvider = (
    kind: ProviderKind,
    patch: Partial<SemanticProviderConfig>,
  ) => {
    setSettings((current) =>
      current
        ? {
            ...current,
            [kind]: { ...current[kind], ...patch },
          }
        : current,
    );
  };

  const saveAndTest = async (kind: ProviderKind) => {
    if (!settings) return;
    setSavingKind(kind);
    setTestingKind(kind);
    setProviderMessage("");
    try {
      const saved = await saveSemanticProvider(
        settings[kind],
      );
      setSettings(saved);
      if (kind === "qa") {
        onAskModelChange(saved.qa.model);
        setProviderMessage(t("semanticProviderSaved"));
      } else {
        setProviderMessage(await testSemanticProvider(kind));
      }
    } catch (caught) {
      onError(normalizeError(caught, "Semantic search failed."));
    } finally {
      setSavingKind(null);
      setTestingKind(null);
    }
  };

  const rebuildIndex = async () => {
    if (indexing) return;
    setIndexing(true);
    setNotice("");
    try {
      const result = await rebuildSemanticIndex();
      setSettings((current) =>
        current ? { ...current, indexStatus: result.status } : current,
      );
      setNotice(
        t("semanticIndexResult", {
          indexed: result.report.indexed,
          failed: result.report.failed,
        }),
      );
      if (result.report.warnings.length > 0) {
        onError(result.report.warnings.join(" · "));
      }
    } catch (caught) {
      onError(normalizeError(caught, "Semantic search failed."));
    } finally {
      setIndexing(false);
    }
  };

  return (
    <div className="semantic-overlay" role="presentation" onMouseDown={onClose}>
      <aside
        className="semantic-panel"
        role="dialog"
        aria-modal="true"
        aria-label={t("semanticSearch")}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="semantic-panel-header">
          <div>
            <span className="eyebrow">Ask Grapes</span>
            <strong>{t("semanticPanelTitle")}</strong>
          </div>
          <button
            type="button"
            className="semantic-icon-button"
            onClick={onClose}
            title={t("closePanel")}
            aria-label={t("closePanel")}
          >
            <X size={16} />
          </button>
        </header>

        <nav className="semantic-tabs">
          <button
            type="button"
            className={tab === "results" ? "active" : ""}
            onClick={() => setTab("results")}
          >
            <Search size={14} />
            {t("semanticResults")}
          </button>
          <button
            type="button"
            className={tab === "settings" ? "active" : ""}
            onClick={() => setTab("settings")}
          >
            <Settings2 size={14} />
            {t("semanticProviderSettings")}
          </button>
        </nav>

        {loadingSettings ? (
          <div className="semantic-loading">
            <LoaderCircle className="spin" size={18} />
            {t("semanticLoading")}
          </div>
        ) : tab === "settings" && settings ? (
          <div className="semantic-settings">
            {(["qa", "embedding", "rerank"] as const).map((kind) => {
              const provider = settings[kind];
              const busy = savingKind === kind || testingKind === kind;
              return (
                <section className="semantic-provider-card" key={kind}>
                  <header>
                    <div>
                      <strong>
                        {kind === "embedding"
                          ? t("semanticEmbeddingProvider")
                          : kind === "rerank"
                            ? t("semanticRerankProvider")
                            : t("semanticAskProvider")}
                      </strong>
                    </div>
                    <label className="semantic-enabled">
                      <input
                        type="checkbox"
                        checked={provider.enabled}
                        onChange={(event) =>
                          updateProvider(kind, {
                            enabled: event.target.checked,
                          })
                        }
                      />
                      {t("semanticEnabled")}
                    </label>
                  </header>
                  <label>
                    <span>{t("semanticProviderType")}</span>
                    <select
                      value={provider.providerType}
                      onChange={(event) =>
                        updateProvider(kind, {
                          providerType: event.target.value,
                        })
                      }
                    >
                      <option value="siliconflow">
                        {t("semanticSiliconFlow")}
                      </option>
                      <option value="custom">
                        {t("semanticCustomProvider")}
                      </option>
                    </select>
                  </label>
                  <label>
                    <span>{t("semanticBaseUrl")}</span>
                    <input
                      value={provider.baseUrl}
                      onChange={(event) =>
                        updateProvider(kind, { baseUrl: event.target.value })
                      }
                      placeholder="https://api.siliconflow.cn/v1"
                    />
                  </label>
                  <label>
                    <span>{t("semanticModel")}</span>
                    <input
                      value={provider.model}
                      onChange={(event) =>
                        updateProvider(kind, { model: event.target.value })
                      }
                    />
                  </label>
                  {provider.model.trim() && (
                    <p className="semantic-price-warning">
                      {provider.model.trim() === "Qwen/Qwen3.5-4B"
                        ? t("semanticProviderPriceFree")
                        : t("semanticProviderPriceWarning")}
                    </p>
                  )}
                  <label>
                    <span>{t("semanticApiKey")}</span>
                    <span className="semantic-secret-field">
                      <input
                        type={visibleKeys[kind] ? "text" : "password"}
                        autoComplete="off"
                        value={provider.apiKey ?? ""}
                        onChange={(event) =>
                          updateProvider(kind, {
                            apiKey: event.target.value,
                          })
                        }
                        placeholder={t("semanticApiKeyPlaceholder")}
                      />
                      <button
                        type="button"
                        title={
                          visibleKeys[kind]
                            ? t("semanticHideApiKey")
                            : t("semanticShowApiKey")
                        }
                        aria-label={
                          visibleKeys[kind]
                            ? t("semanticHideApiKey")
                            : t("semanticShowApiKey")
                        }
                        onClick={() =>
                          setVisibleKeys((current) => ({
                            ...current,
                            [kind]: !current[kind],
                          }))
                        }
                      >
                        {visibleKeys[kind] ? (
                          <EyeOff size={13} />
                        ) : (
                          <Eye size={13} />
                        )}
                      </button>
                    </span>
                  </label>
                  <button
                    type="button"
                    className="semantic-primary-button"
                    onClick={() => void saveAndTest(kind)}
                    disabled={busy}
                  >
                    {busy ? (
                      <LoaderCircle className="spin" size={14} />
                    ) : (
                      <FlaskConical size={14} />
                    )}
                    {busy
                      ? kind === "qa"
                        ? t("semanticSaving")
                        : t("semanticTesting")
                      : kind === "qa"
                        ? t("semanticSaveProvider")
                        : t("semanticSaveAndTest")}
                  </button>
                </section>
              );
            })}
            {providerMessage && (
              <div className="semantic-inline-note">{providerMessage}</div>
            )}
          </div>
        ) : settings ? (
          <div className="semantic-search-view">
            {(!settings.embedding.enabled ||
              !settings.embedding.apiKey?.trim()) && (
              <button
                type="button"
                className="semantic-setup-warning"
                onClick={() => setTab("settings")}
              >
                {t("semanticSetupRequired")}
              </button>
            )}
            <div className="semantic-index-summary">
              <div>
                <Database size={15} />
                <span>
                  <strong>
                    {settings.indexStatus.indexedAssets} /{" "}
                    {settings.indexStatus.totalAssets}
                  </strong>
                  <small>{t("semanticIndexedPhotos")}</small>
                </span>
              </div>
              <button
                type="button"
                onClick={() => void rebuildIndex()}
                disabled={indexing || !settings.embedding.enabled}
              >
                {indexing ? (
                  <LoaderCircle className="spin" size={13} />
                ) : (
                  <Database size={13} />
                )}
                {indexing ? t("semanticIndexing") : t("semanticRebuildIndex")}
              </button>
            </div>
            {notice && <div className="semantic-inline-note">{notice}</div>}
            {!response && (
              <p className="semantic-empty">{t("semanticEnterQueryHint")}</p>
            )}
            {response && (
              <div className="semantic-results">
                <header>
                  <strong>{t("semanticResults")}</strong>
                  <span>
                    {t("semanticCandidateCount", {
                      count: response.candidateCount,
                    })}
                    {response.reranked ? ` · ${t("semanticReranked")}` : ""}
                  </span>
                </header>
                {response.warnings.map((warning) => (
                  <p className="semantic-warning" key={warning}>
                    {warning}
                  </p>
                ))}
                {response.results.length === 0 ? (
                  <p className="semantic-empty">{t("semanticNoResults")}</p>
                ) : (
                  response.results.map((result) => {
                    const asset = assets.find(
                      (item) => item.id === result.assetId,
                    );
                    if (!asset) return null;
                    return (
                      <button
                        type="button"
                        className="semantic-result-card"
                        key={result.assetId}
                        onClick={() => onOpenAsset(asset)}
                      >
                        <AssetThumbnail
                          asset={asset}
                          className="semantic-result-thumb"
                        />
                        <span className="semantic-result-content">
                          <span className="semantic-result-heading">
                            <strong>{result.filename}</strong>
                            <b>{Math.round(result.score * 100)}%</b>
                          </span>
                          <small>{result.relativePath}</small>
                          <span className="semantic-result-reasons">
                            {result.matchReasons.slice(0, 4).map((reason) => (
                              <i key={`${reason.kind}:${reason.label}`}>
                                {reason.kind === "embedding"
                                  ? `${t("semanticReasonEmbedding")} ${Math.round(reason.score * 100)}%`
                                  : reason.kind === "rerank"
                                    ? `${t("semanticReasonRerank")} ${Math.round(reason.score * 100)}%`
                                    : reason.kind === "classification"
                                      ? reason.label
                                      : t("semanticReasonMetadata")}
                              </i>
                            ))}
                          </span>
                        </span>
                      </button>
                    );
                  })
                )}
              </div>
            )}
          </div>
        ) : null}
      </aside>
    </div>
  );
}
