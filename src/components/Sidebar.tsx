import type { Contact, MyIdentity } from "../lib/types";

interface Props {
  me: MyIdentity | null;
  contacts: Contact[];
  selected: string | null;
  unread: Record<string, number>;
  onSelect: (nodeId: string) => void;
  onOpenPairing: () => void;
}

function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
}

export function Sidebar({ me, contacts, selected, unread, onSelect, onOpenPairing }: Props) {
  return (
    <aside className="sidebar">
      <header className="sidebar-head">
        <div className="brand">
          <span className="brand-dot" />
          TaylorChat
        </div>
        <button className="btn-icon" title="Parear / adicionar contato" onClick={onOpenPairing}>
          ＋
        </button>
      </header>

      {me && (
        <div className="me" title={me.nodeId}>
          <span className="me-label">eu</span>
          <code className="me-id">{shortId(me.nodeId)}</code>
        </div>
      )}

      <div className="contacts">
        {contacts.length === 0 && (
          <div className="empty">
            <p>Nenhum contato ainda.</p>
            <button className="btn" onClick={onOpenPairing}>
              Parear com alguém
            </button>
          </div>
        )}
        {contacts.map((c) => (
          <button
            key={c.nodeId}
            className={`contact ${selected === c.nodeId ? "is-active" : ""}`}
            onClick={() => onSelect(c.nodeId)}
          >
            <span className="avatar">{(c.nickname || c.nodeId).slice(0, 1).toUpperCase()}</span>
            <span className="contact-body">
              <span className="contact-name">{c.nickname || shortId(c.nodeId)}</span>
              <code className="contact-id">{shortId(c.nodeId)}</code>
            </span>
            {(unread[c.nodeId] ?? 0) > 0 && <span className="badge">{unread[c.nodeId]}</span>}
          </button>
        ))}
      </div>
    </aside>
  );
}
