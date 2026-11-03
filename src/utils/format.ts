import type { Locale } from "../i18n";

export function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function formatModifiedAt(
  mtimeNs: number,
  locale: Locale,
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

export function normalizeError(error: unknown, fallback: string) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return fallback;
}

export function fileExtension(filename: string, maxLength = 4) {
  const extension = filename.split(".").pop();
  return extension ? extension.toUpperCase().slice(0, maxLength) : "IMG";
}
