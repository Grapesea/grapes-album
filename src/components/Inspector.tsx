import type { ReactNode } from "react";
import {
  Aperture,
  CalendarDays,
  Camera,
  FileType2,
  Folder,
  HardDrive,
  Image as ImageIcon,
  MapPin,
  Plus,
  Ruler,
  Tag,
  Timer,
  X,
} from "lucide-react";
import type { AssetView, FolderView } from "../bindings/types";
import { useI18n } from "../i18n";
import { absoluteWorkspacePath } from "../utils/paths";
import { Histogram } from "./Histogram";

interface InspectorProps {
  asset: AssetView | null;
  folder: FolderView | null;
  assetCount: number;
  folderCount: number;
  workspaceRootPath: string;
  tagBusy: boolean;
  onSetTags: (tags: string[]) => Promise<boolean>;
}

export function Inspector({
  asset,
  folder,
  assetCount,
  folderCount,
  workspaceRootPath,
  tagBusy,
  onSetTags,
}: InspectorProps) {
  const { locale, t } = useI18n();
  if (asset) {
    const metadata = asset.metadata;
    const cameraName = [metadata.cameraMake, metadata.cameraModel]
      .filter(Boolean)
      .join(" ");
    const hasCaptureDetails = Boolean(
      metadata.capturedAt ||
        cameraName ||
        metadata.lensModel ||
        metadata.exposureTime ||
        metadata.fNumber ||
        metadata.iso ||
        metadata.focalLength ||
        metadata.gpsLat != null ||
        (metadata.width && metadata.height),
    );
    const addTags = async () => {
      const input = window.prompt(t("tagPrompt"));
      if (!input) return;
      const additions = input
        .split(/[,，]/)
        .map((tag) => tag.trim())
        .filter(Boolean);
      const existing = new Set(
        asset.tags.map((tag) => tag.toLocaleLowerCase()),
      );
      const next = [
        ...asset.tags,
        ...additions.filter(
          (tag) => !existing.has(tag.toLocaleLowerCase()),
        ),
      ];
      await onSetTags(next);
    };

    return (
      <aside className="inspector">
        <div className="inspector-header">
          <div>
            <span className="eyebrow">{t("inspector")}</span>
            <h2>{t("photoProperties")}</h2>
          </div>
        </div>

        <div className="inspector-hero">
          <div className="file-preview">
            <ImageIcon size={28} />
            <span>{extensionOf(asset.filename)}</span>
          </div>
          <div>
            <strong>{asset.filename}</strong>
            <p>{asset.relativePath}</p>
          </div>
        </div>

        <InspectorSection title={t("histogram")}>
          <Histogram asset={asset} />
        </InspectorSection>

        <InspectorSection title={t("tags")}>
          <div className="tag-manager">
            {asset.tags.length === 0 ? (
              <span className="tag-empty">{t("noTagsYet")}</span>
            ) : (
              asset.tags.map((tag) => (
                <span className="tag-pill editable" key={tag}>
                  <Tag size={11} />
                  {tag}
                  <button
                    type="button"
                    title={t("removeTag", { tag })}
                    disabled={tagBusy}
                    onClick={() =>
                      onSetTags(asset.tags.filter((item) => item !== tag))
                    }
                  >
                    <X size={11} />
                  </button>
                </span>
              ))
            )}
            <button
              type="button"
              className="add-tag-button"
              onClick={addTags}
              disabled={tagBusy}
            >
              <Plus size={12} />
              {t("addTag")}
            </button>
          </div>
        </InspectorSection>

        <InspectorSection title={t("fileInfo")}>
          <InfoRow
            icon={<MapPin size={14} />}
            label={t("location")}
            value={asset.relativePath}
          />
          <InfoRow
            icon={<FileType2 size={14} />}
            label={t("format")}
            value={extensionOf(asset.filename)}
          />
          <InfoRow
            icon={<HardDrive size={14} />}
            label={t("fileSize")}
            value={formatBytes(asset.fileSize)}
          />
          <InfoRow
            icon={<CalendarDays size={14} />}
            label={t("modifiedAt")}
            value={formatModifiedAt(asset.mtimeNs, locale, t("unknown"))}
          />
        </InspectorSection>

        {hasCaptureDetails && (
          <InspectorSection title={t("cameraMetadata")}>
            {cameraName && (
              <InfoRow
                icon={<Camera size={14} />}
                label={t("camera")}
                value={cameraName}
              />
            )}
            {metadata.lensModel && (
              <InfoRow
                icon={<Camera size={14} />}
                label={t("lens")}
                value={metadata.lensModel}
              />
            )}
            {metadata.capturedAt && (
              <InfoRow
                icon={<CalendarDays size={14} />}
                label={t("capturedAt")}
                value={metadata.capturedAt.replace("T", " ")}
              />
            )}
            {metadata.width && metadata.height ? (
              <InfoRow
                icon={<Ruler size={14} />}
                label={t("dimensions")}
                value={`${metadata.width} × ${metadata.height}`}
              />
            ) : null}
            {metadata.exposureTime && (
              <InfoRow
                icon={<Timer size={14} />}
                label={t("exposure")}
                value={metadata.exposureTime}
              />
            )}
            {metadata.fNumber != null && (
              <InfoRow
                icon={<Aperture size={14} />}
                label={t("aperture")}
                value={`f/${metadata.fNumber}`}
              />
            )}
            {metadata.iso != null && (
              <InfoRow
                icon={<Aperture size={14} />}
                label={t("iso")}
                value={`ISO ${metadata.iso}`}
              />
            )}
            {metadata.focalLength != null && (
              <InfoRow
                icon={<Ruler size={14} />}
                label={t("focalLength")}
                value={`${metadata.focalLength} mm`}
              />
            )}
            {metadata.gpsLat != null && metadata.gpsLon != null && (
              <InfoRow
                icon={<MapPin size={14} />}
                label={t("gps")}
                value={`${metadata.gpsLat.toFixed(5)}, ${metadata.gpsLon.toFixed(5)}`}
              />
            )}
          </InspectorSection>
        )}

      </aside>
    );
  }

  if (folder) {
    return (
      <aside className="inspector">
        <div className="inspector-header">
          <div>
            <span className="eyebrow">{t("inspector")}</span>
            <h2>{t("folder")}</h2>
          </div>
        </div>

        <div className="inspector-hero">
          <div className="file-preview folder-preview">
            <Folder size={28} />
          </div>
          <div>
            <strong>{folder.name}</strong>
            <p>
              {absoluteWorkspacePath(
                workspaceRootPath,
                folder.relativePath,
              )}
            </p>
          </div>
        </div>

        <InspectorSection title={t("contents")}>
          <InfoRow
            icon={<ImageIcon size={14} />}
            label={t("photoTotal")}
            value={t("photosCount", { count: assetCount })}
          />
          <InfoRow
            icon={<Folder size={14} />}
            label={t("subfolders")}
            value={t("foldersCount", { count: folderCount })}
          />
        </InspectorSection>

      </aside>
    );
  }

  return null;
}

function InspectorSection({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="inspector-section">
      <h3>{title}</h3>
      <div className="inspector-section-body">{children}</div>
    </section>
  );
}

function InfoRow({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="info-row">
      <span className="info-label">
        {icon}
        {label}
      </span>
      <span className="info-value" title={value}>
        {value}
      </span>
    </div>
  );
}

function extensionOf(filename: string) {
  const extension = filename.split(".").pop();
  return extension ? extension.toUpperCase().slice(0, 5) : "IMG";
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
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
