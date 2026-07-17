import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import * as api from "../lib/api";
import { t, type Lang } from "../lib/i18n";
import type { Theme } from "../lib/types";

interface Props {
  theme: Theme;
  onTheme: (t: Theme) => void;
  lang: Lang;
  onLang: (l: Lang) => void;
  readReceipts: boolean;
  onReadReceipts: (b: boolean) => void;
  auditPeer: string | null;
  onProfileSaved: () => void;
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
  onProfileSaved,
  onClose,
}: Props) {
  const [audit, setAudit] = useState<api.AuditResult | null>(null);
  const [auditing, setAuditing] = useState(false);
  const [net, setNet] = useState<api.NetStatus | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [pName, setPName] = useState("");
  const [pAvatar, setPAvatar] = useState<string | null>(null);
  const [notifyPreview, setNotifyPreview] = useState(true);

  const refreshProfile = () =>
    api.getProfile().then((p) => {
      setPName(p.name);
      setPAvatar(p.avatar);
    }).catch(() => {});

  const saveProfile = async (photo?: string) => {
    try {
      const p = await api.setProfile(pName, photo);
      setPName(p.name);
      setPAvatar(p.avatar);
      onProfileSaved();
    } catch {
      /* ignore */
    }
  };
  const pickPhoto = async () => {
    const path = await api.pickProfilePhoto();
    if (path) saveProfile(path);
  };

  const refreshDiag = () => {
    api.netStatus().then(setNet).catch(() => {});
    api.netLog().then(setLog).catch(() => {});
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    refreshDiag();
    refreshProfile();
    api.getNotifyPreview().then(setNotifyPreview).catch(() => {});
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const changeNotifyPreview = (b: boolean) => {
    setNotifyPreview(b);
    api.setNotifyPreview(b).catch(() => {});
  };

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
          <div className="set-profile">
            <div className="set-profile-pic" onClick={pickPhoto} title={t("settings.profilePhoto")}>
              {pAvatar ? <img src={convertFileSrc(pAvatar)} alt="" /> : <span>＋</span>}
            </div>
            <div className="set-profile-fields">
              <input
                value={pName}
                placeholder={t("settings.profileName")}
                onChange={(e) => setPName(e.target.value)}
                onBlur={() => saveProfile()}
              />
              <button className="btn" onClick={pickPhoto}>
                {t("settings.profilePhoto")}
              </button>
            </div>
          </div>

          <label className="set-row">
            <span>{t("settings.theme")}</span>
            <select value={theme} onChange={(e) => onTheme(e.target.value as Theme)}>
              <option value="system">{t("settings.themeSystem")}</option>
              <option value="light">{t("settings.themeLight")}</option>
              <option value="dark">{t("settings.themeDark")}</option>
              <option value="nature">{t("settings.themeNature")}</option>
              <option value="darkblue">{t("settings.themeDarkblue")}</option>
              <option value="calmgreen">{t("settings.themeCalmgreen")}</option>
              <option value="pastelpink">{t("settings.themePastelpink")}</option>
              <option value="punkprincess">{t("settings.themePunkprincess")}</option>
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

          <label className="set-row">
            <span>
              {t("settings.notifyPreview")}
              <small>{t("settings.notifyPreviewHint")}</small>
            </span>
            <input
              type="checkbox"
              checked={notifyPreview}
              onChange={(e) => changeNotifyPreview(e.target.checked)}
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

          <div className="set-audit">
            <strong>{t("settings.diag")}</strong>
            <div className="ai-row">
              <span className={`ai-dot ${net?.up ? "on" : ""}`} />
              <span className="hint">{net?.up ? t("settings.diagUp") : t("settings.diagDown")}</span>
              <button className="btn" style={{ marginLeft: "auto" }} onClick={refreshDiag}>
                ↻
              </button>
            </div>
            {net && <code className="ai-dir">id: {net.nodeId}</code>}
            <pre className="net-log">{log.join("\n") || "—"}</pre>
            <button
              className="btn"
              onClick={() => navigator.clipboard.writeText(log.join("\n")).catch(() => {})}
            >
              {t("settings.diagCopy")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
