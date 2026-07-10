import { useEffect, useState } from "react";
import type { Contact, ConvoSummary, Thread } from "../lib/types";
import { shortId, splitConvo } from "../lib/ui";
import { t } from "../lib/i18n";
import { Avatar } from "./Avatar";

interface Props {
  threads: Thread[];
  contacts: Contact[];
  summaries: Record<string, ConvoSummary>;
  exclude: string | null; // não encaminhar pra própria conversa
  onPick: (convo: string) => void;
  onClose: () => void;
}

/// Seletor de conversa pra encaminhar uma mensagem. Lista as conversas por atividade.
export function ForwardModal({ threads, contacts, summaries, exclude, onPick, onClose }: Props) {
  const [q, setQ] = useState("");
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const byNode = new Map(contacts.map((c) => [c.nodeId, c]));
  const ql = q.trim().toLowerCase();
  const rows = [...threads]
    .filter((th) => th.convo !== exclude)
    .map((th) => {
      const { node } = splitConvo(th.convo);
      const c = byNode.get(node);
      const base = c?.nickname || c?.profileName || shortId(node);
      return { convo: th.convo, node, base, name: th.name ? `${base} · ${th.name}` : base, avatar: c?.avatar };
    })
    .filter((r) => !ql || r.name.toLowerCase().includes(ql))
    .sort((a, b) => (summaries[b.convo]?.ts ?? 0) - (summaries[a.convo]?.ts ?? 0));

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal fwd" onClick={(e) => e.stopPropagation()}>
        <header className="modal-head">
          <h3>{t("fwd.title")}</h3>
          <button className="btn-icon" onClick={onClose}>
            ✕
          </button>
        </header>
        <div className="fwd-search">
          <input
            autoFocus
            value={q}
            placeholder={t("fwd.search")}
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        <div className="fwd-list">
          {rows.length === 0 && <div className="empty">{t("sidebar.searchNone")}</div>}
          {rows.map((r) => (
            <button key={r.convo} className="contact" onClick={() => onPick(r.convo)}>
              <Avatar nodeId={r.node} name={r.base} avatar={r.avatar} />
              <span className="contact-body">
                <span className="contact-name">{r.name}</span>
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
