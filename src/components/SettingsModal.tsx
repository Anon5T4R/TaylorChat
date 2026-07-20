import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import * as api from "../lib/api";
import { fmtBytes } from "../lib/bytes";
import { LANG_LABELS, t, type Lang, type MessageKey } from "../lib/i18n";
import type { Theme } from "../lib/types";

/** As quatro limpezas do painel; `confirm` é a pergunta que precede cada uma. */
type CleanKind = "orphan" | "partial" | "avatars" | "backups";
const CONFIRM: Record<CleanKind, MessageKey> = {
  orphan: "storage.confirmOrphan",
  partial: "storage.confirmPartial",
  avatars: "storage.confirmAvatars",
  backups: "storage.confirmBackups",
};

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
  const [notifyEnabled, setNotifyEnabled] = useState(true);
  const [store, setStore] = useState<api.StorageInfo | null>(null);
  const [confirm, setConfirm] = useState<CleanKind | null>(null);
  const [busy, setBusy] = useState(false);
  const [storeMsg, setStoreMsg] = useState("");

  const refreshStorage = useCallback(async () => {
    try {
      setStore(await api.storageInfo());
    } catch (e) {
      // A varredura ABORTA se alguma linha de anexo não decifrar. Mostrar o erro
      // é o certo: o painel some, e some dizendo por quê.
      setStore(null);
      setStoreMsg(t("storage.loadFailed", { e: String(e) }));
    }
  }, []);

  const runClean = async (kind: CleanKind) => {
    setConfirm(null);
    setBusy(true);
    try {
      const freed =
        kind === "orphan"
          ? await api.storageClearOrphanAttachments()
          : kind === "partial"
            ? await api.storageClearPartials()
            : kind === "avatars"
              ? await api.storageClearOrphanAvatars()
              : await api.storageClearOldBackups();
      setStoreMsg(
        freed.files === 0
          ? t("storage.nothing")
          : t("storage.freed", { size: fmtBytes(freed.bytes), n: freed.files }),
      );
      await refreshStorage();
    } catch (e) {
      setStoreMsg(t("storage.failed", { e: String(e) }));
    } finally {
      setBusy(false);
    }
  };

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
    api.getNotifyEnabled().then(setNotifyEnabled).catch(() => {});
    void refreshStorage();
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, refreshStorage]);

  const changeNotifyPreview = (b: boolean) => {
    setNotifyPreview(b);
    api.setNotifyPreview(b).catch(() => {});
  };
  const changeNotifyEnabled = (b: boolean) => {
    setNotifyEnabled(b);
    api.setNotifyEnabled(b).catch(() => {});
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
              {(Object.keys(LANG_LABELS) as Lang[]).map((l) => (
                <option key={l} value={l}>
                  {LANG_LABELS[l]}
                </option>
              ))}
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
              {t("settings.notify")}
              <small>{t("settings.notifyHint")}</small>
            </span>
            <input
              type="checkbox"
              checked={notifyEnabled}
              onChange={(e) => changeNotifyEnabled(e.target.checked)}
            />
          </label>

          {/* Prévia só faz sentido com o aviso ligado — desabilita em vez de sumir, pra
              o usuário ver que a opção existe e por que está fora de alcance. */}
          <label className="set-row">
            <span>
              {t("settings.notifyPreview")}
              <small>{t("settings.notifyPreviewHint")}</small>
            </span>
            <input
              type="checkbox"
              disabled={!notifyEnabled}
              checked={notifyPreview}
              onChange={(e) => changeNotifyPreview(e.target.checked)}
            />
          </label>

          {/* Honestidade: o app não tem como saber se o toast apareceu. O plugin responde
              "permitido" fixo no desktop e engole o erro do disparo, então prometer que
              está funcionando seria chute. Dizemos onde olhar se não aparecer. */}
          {notifyEnabled && <p className="hint">{t("settings.notifyOsHint")}</p>}

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

          <div className="set-audit storage">
            <strong>{t("storage.section")}</strong>

            {store && (
              <>
                <div className="storage-row">
                  <div className="storage-label">
                    <span>{t("storage.path")}</span>
                    <code className="storage-dir" title={store.dir}>
                      {store.dir}
                    </code>
                  </div>
                  <button className="btn" onClick={() => void api.openAttachment(store.dir)}>
                    {t("storage.open")}
                  </button>
                </div>

                <div className="storage-row">
                  <div className="storage-label">
                    <span>
                      {t("storage.db")} — <strong>{fmtBytes(store.dbBytes)}</strong>
                    </span>
                    <small>
                      {t("storage.dbCounts", {
                        n: store.messages,
                        f: store.fileMessages,
                        c: store.contacts,
                        v: store.conversations,
                      })}
                    </small>
                    <small>{t("storage.dbHint")}</small>
                  </div>
                </div>

                <div className="storage-row">
                  <div className="storage-label">
                    <span>
                      {t("storage.attachments")} — <strong>{fmtBytes(store.attachmentsBytes)}</strong>
                    </span>
                    <small>
                      {t("storage.attachmentsCounts", {
                        n: store.attachmentsFiles,
                        orphans: store.orphanAttachmentsFiles,
                        orphanSize: fmtBytes(store.orphanAttachmentsBytes),
                      })}
                    </small>
                    <small>{t("storage.clearOrphanHint")}</small>
                  </div>
                  <button
                    className="btn"
                    disabled={busy || store.orphanAttachmentsFiles === 0}
                    onClick={() => setConfirm("orphan")}
                  >
                    {t("storage.clear")}
                  </button>
                </div>

                <div className="storage-row">
                  <div className="storage-label">
                    <span>
                      {t("storage.partial")} — <strong>{fmtBytes(store.partialBytes)}</strong>
                    </span>
                    <small>{t("storage.partialCounts", { n: store.partialFiles })}</small>
                    <small>{t("storage.clearPartialHint")}</small>
                  </div>
                  <button
                    className="btn"
                    disabled={busy || store.partialFiles === 0}
                    onClick={() => setConfirm("partial")}
                  >
                    {t("storage.clear")}
                  </button>
                </div>

                <div className="storage-row">
                  <div className="storage-label">
                    <span>
                      {t("storage.avatars")} — <strong>{fmtBytes(store.avatarsBytes)}</strong>
                    </span>
                    <small>
                      {t("storage.avatarsCounts", {
                        n: store.avatarsFiles,
                        orphans: store.orphanAvatarsFiles,
                        orphanSize: fmtBytes(store.orphanAvatarsBytes),
                      })}
                    </small>
                    <small>{t("storage.clearAvatarsHint")}</small>
                  </div>
                  <button
                    className="btn"
                    disabled={busy || store.orphanAvatarsFiles === 0}
                    onClick={() => setConfirm("avatars")}
                  >
                    {t("storage.clear")}
                  </button>
                </div>

                <div className="storage-row">
                  <div className="storage-label">
                    <span>
                      {t("storage.backups")} — <strong>{fmtBytes(store.backupsBytes)}</strong>
                    </span>
                    <small>
                      {t("storage.backupsCounts", {
                        n: store.backupsFiles,
                        old: store.oldBackupsFiles,
                        oldSize: fmtBytes(store.oldBackupsBytes),
                      })}
                    </small>
                    <small>{t("storage.clearBackupsHint")}</small>
                  </div>
                  <button
                    className="btn"
                    disabled={busy || store.oldBackupsFiles === 0}
                    onClick={() => setConfirm("backups")}
                  >
                    {t("storage.clear")}
                  </button>
                </div>

                {/* Medido e nunca apagado — dizer isso vale mais que um botão. */}
                <div className="storage-row">
                  <div className="storage-label">
                    <span>
                      {t("storage.stickers")} — <strong>{fmtBytes(store.stickersBytes)}</strong>
                    </span>
                    <small>{t("storage.stickersCounts", { n: store.stickersFiles })}</small>
                    <small>{t("storage.stickersHint")}</small>
                  </div>
                </div>
              </>
            )}

            {confirm && (
              <div className="storage-confirm">
                <strong>{t("storage.confirmTitle")}</strong>
                <p>{t(CONFIRM[confirm])}</p>
                <div className="storage-confirm-actions">
                  <button className="btn" onClick={() => setConfirm(null)}>
                    {t("storage.cancel")}
                  </button>
                  <button className="btn danger" onClick={() => void runClean(confirm)}>
                    {t("storage.confirmYes")}
                  </button>
                </div>
              </div>
            )}

            {storeMsg && <p className="hint">{storeMsg}</p>}
          </div>
        </div>
      </div>
    </div>
  );
}
