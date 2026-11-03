import { Languages } from "lucide-react";
import { useI18n } from "../i18n";

export function LanguageSwitcher({ className = "" }: { className?: string }) {
  const { locale, setLocale, t } = useI18n();
  return (
    <div
      className={`language-switch ${className}`.trim()}
      role="group"
      aria-label={t("language")}
    >
      <button
        type="button"
        className={locale === "zh" ? "active" : ""}
        aria-pressed={locale === "zh"}
        onClick={() => setLocale("zh")}
      >
        {t("chinese")}
      </button>
      <button
        type="button"
        className={locale === "en" ? "active" : ""}
        aria-pressed={locale === "en"}
        onClick={() => setLocale("en")}
      >
        {t("english")}
      </button>
    </div>
  );
}
