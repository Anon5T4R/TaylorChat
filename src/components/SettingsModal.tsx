import { useEffect, useState } from "react";
import * as api from "../lib/api";
import { t, type Lang } from "../lib/i18n";

type Theme = "system" | "light" | "dark";

interface Props {
  theme: Theme;
  onTheme: (t: Theme) => void;
  lang: Lang;
  onLang: (l: Lang) => void;
  readReceipts: boolean;
  onReadReceipts: (b: boolean) => void;
  auditPeer: string | null;
  onClose: () => void;
}

function grouped(hex: string): string {
  return (hex.match(/.{1,4}/g) ?? []).join(" ");
}

export function SettingsModal({
  theme,
  onTheme,
  lang,
  onLang,
  readReceipts,
  onReadReceipts,
  auditPeer,
  onClose,
}: Props) {
  const [audit, setAudit] = useState<api.AuditResult | null>(null);
  const [auditing, setAuditing] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const runAudit = async () => {
    if (!auditPeer) return;
    setAuditing(true);
    try {
      setAudit(await api.auditConversation(auditPeer));
    } catch {
      setAudit(null);
    } finally {
      setAuditing(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal settings" onClick={(e) => e.stopPropagation()}>
        <header className="modal-head">
          <h3>{t("settings.title")}</h3>
          <button className="btn-icon" onClick={onClose}>
            ✕
          </button>
        </header>

        <div className="settings-body">
          <label className="set-row">
            <span>{t("settings.theme")}</span>
            <select value={theme} onChange={(e) => onTheme(e.target.value as Theme)}>
              <option value="system">{t("settings.themeSystem")}</option>
              <option value="light">{t("settings.themeLight")}</option>
              <option value="dark">{t("settings.themeDark")}</option>
            </select>
          </label>

          <label className="set-row">
            <span>{t("settings.lang")}</span>
            <select value={lang} onChange={(e) => onLang(e.target.value as Lang)}>
              <option value="pt">Português</option>
              <option value="es">Español</option>
              <option value="en">English</option>
            </select>
          </label>

          <label className="set-row">
            <span>
              {t("settings.readReceipts")}
              <small>{t("settings.readReceiptsHint")}</small>
            </span>
            <input
              type="checkbox"
              checked={readReceipts}
              onChange={(e) => onReadReceipts(e.target.checked)}
            />
          </label>

          <div className="set-audit">
            <strong>{t("settings.audit")}</strong>
            <p className="hint">{t("settings.auditHint")}</p>
            {auditPeer ? (
              <button className="btn" disabled={auditing} onClick={runAudit}>
                {t("settings.auditRun")}
              </button>
            ) : (
              <p className="hint">{t("settings.auditPick")}</p>
            )}
            {audit && (
              <div className="audit-out">
                <div className="audit-count">
                  {audit.count} {t("settings.auditCount")}
                </div>
                <code className="audit-digest">{grouped(audit.digest)}</code>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
