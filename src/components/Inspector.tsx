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
import type { AssetView, Dimension, FolderView } from "../bindings/types";
import { useI18n } from "../i18n";
import { absoluteWorkspacePath } from "../utils/paths";
import { Histogram } from "./Histogram";
import { ClassificationEditor } from "./ClassificationEditor";

interface InspectorProps {
  asset: AssetView | null;
  folder: FolderView | null;
  assetCount: number;
  folderCount: number;
  workspaceRootPath: string;
  classificationSchema: Dimension[];
  classificationBusy: boolean;
  onSetClassification: (
    assetId: string,
    valueId: string,
    selected: boolean,
  ) => Promise<boolean>;
  onCreateClassificationDimension: (name: string) => Promise<boolean>;
  onCreateClassificationValue: (
    dimensionId: string,
    name: string,
  ) => Promise<boolean>;
  tagBusy: boolean;
  onSetTags: (tags: string[]) => Promise<boolean>;
}

export function Inspector({
  asset,
  folder,
  assetCount,
  folderCount,
  workspaceRootPath,
  classificationSchema,
  classificationBusy,
  onSetClassification,
  onCreateClassificationDimension,
  onCreateClassificationValue,
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
        metadata.focalLength ||
        metadata.exposureTime ||
        metadata.fNumber ||
        metadata.iso ||
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
            <h3>{t("photoProperties")}: {(asset.filename)}</h3>
          </div>
        </div>

        <InspectorSection title={t("histogram")}>
          <Histogram asset={asset} />
        </InspectorSection>

        <InspectorSection title={t("tags")} className="flush-inspector-section">
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

        <InspectorSection
          title={t("fileInfo")}
          className="flush-inspector-section"
        >
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
          <InspectorSection
            title={t("cameraMetadata")}
            className="flush-inspector-section"
          >
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
            {metadata.focalLength != null && (
              <InfoRow
                icon={<Ruler size={14} />}
                label={t("focalLength")}
                value={`${metadata.focalLength} mm`}
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
            {metadata.gpsLat != null && metadata.gpsLon != null && (
              <InfoRow
                icon={<MapPin size={14} />}
                label={t("gps")}
                value={`${metadata.gpsLat.toFixed(5)}, ${metadata.gpsLon.toFixed(5)}`}
              />
            )}
          </InspectorSection>
        )}

        <InspectorSection
          title={t("classifications")}
          className="classification-section"
          action={
            <button
              type="button"
              className="classification-title-action"
              disabled={classificationBusy}
              onClick={async () => {
                const name = window.prompt(t("newDimensionPrompt"));
                if (name) {
                  await onCreateClassificationDimension(name);
                }
              }}
            >
              <Plus size={11} />
              {t("addDimension")}
            </button>
          }
        >
          <ClassificationEditor
            classifications={asset.classifications}
            dimensions={classificationSchema}
            busy={classificationBusy}
            onToggle={(valueId, selected) =>
              onSetClassification(asset.id, valueId, selected)
            }
            onCreateValue={onCreateClassificationValue}
          />
        </InspectorSection>

      </aside>
    );
  }

  if (folder) {
    return (
      <aside className="inspector">
        <div className="inspector-header">
          <div>
            <h2>{t("folder")}: {folder.name}</h2>
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
  className = "",
  action,
  children,
}: {
  title: string;
  className?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className={`inspector-section ${className}`.trim()}>
      <div className="inspector-section-heading">
        <h3>{title}</h3>
        {action}
      </div>
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
