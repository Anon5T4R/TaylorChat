import { useEffect, useState } from "react";
import type { Contact } from "../lib/types";
import { shortId } from "../lib/ui";
import { t } from "../lib/i18n";
import { Avatar } from "./Avatar";

interface Props {
  contact: Contact;
  onSave: (
    nodeId: string,
    fields: { nickname: string; phone: string; email: string; birthday: string; notes: string },
  ) => void;
  onRemove: (nodeId: string) => void;
  onClose: () => void;
}

/// Ficha do contato: nome/foto que ELE definiu (cacheados, só leitura) + campos locais
/// meus (apelido, telefone, email, aniversário, notas). Aqui também mora o apagar contato.
export function ContactProfile({ contact, onSave, onRemove, onClose }: Props) {
  const [nickname, setNickname] = useState(contact.nickname ?? "");
  const [phone, setPhone] = useState(contact.phone ?? "");
  const [email, setEmail] = useState(contact.email ?? "");
  const [birthday, setBirthday] = useState(contact.birthday ?? "");
  const [notes, setNotes] = useState(contact.notes ?? "");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const save = () => {
    onSave(contact.nodeId, { nickname, phone, email, birthday, notes });
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  const displayName = contact.nickname || contact.profileName || shortId(contact.nodeId);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal profile" onClick={(e) => e.stopPropagation()}>
        <header className="modal-head">
          <h3>{t("profile.title")}</h3>
          <button className="btn-icon" onClick={onClose}>
            ✕
          </button>
        </header>

        <div className="profile-body">
          <div className="profile-hero">
            <Avatar nodeId={contact.nodeId} name={displayName} avatar={contact.avatar} size={72} />
            <div className="profile-hero-txt">
              <strong>{contact.profileName || displayName}</strong>
              <code>{shortId(contact.nodeId)}</code>
              {contact.muted && <span className="profile-muted">🔕 {t("profile.muted")}</span>}
            </div>
          </div>

          <label className="profile-field">
            <span>{t("profile.nickname")}</span>
            <input value={nickname} placeholder={contact.profileName ?? ""} onChange={(e) => setNickname(e.target.value)} />
          </label>
          <label className="profile-field">
            <span>{t("profile.phone")}</span>
            <input value={phone} inputMode="tel" onChange={(e) => setPhone(e.target.value)} />
          </label>
          <label className="profile-field">
            <span>{t("profile.email")}</span>
            <input value={email} inputMode="email" onChange={(e) => setEmail(e.target.value)} />
          </label>
          <label className="profile-field">
            <span>{t("profile.birthday")}</span>
            <input type="date" value={birthday} onChange={(e) => setBirthday(e.target.value)} />
          </label>
          <label className="profile-field">
            <span>{t("profile.notes")}</span>
            <textarea rows={3} value={notes} onChange={(e) => setNotes(e.target.value)} />
          </label>

          <button className="btn btn-primary profile-save" onClick={save}>
            {saved ? t("profile.saved") : t("profile.save")}
          </button>

          <div className="profile-danger">
            <span className="profile-danger-label">{t("profile.dangerZone")}</span>
            <button
              className="btn-danger"
              onClick={() => {
                if (window.confirm(`${displayName} — ${t("profile.removeConfirm")}`)) onRemove(contact.nodeId);
              }}
            >
              🗑 {t("profile.remove")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
