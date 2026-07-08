import type { Contact, ConvoSummary, MyIdentity } from "../lib/types";
import { avatarColor, shortId, shortTime } from "../lib/ui";
import { t } from "../lib/i18n";

interface Props {
  me: MyIdentity | null;
  contacts: Contact[];
  selected: string | null;
  unread: Record<string, number>;
  summaries: Record<string, ConvoSummary>;
  onSelect: (nodeId: string) => void;
  onOpenPairing: () => void;
  onOpenSettings: () => void;
}

export function Sidebar({
  me,
  contacts,
  selected,
  unread,
  summaries,
  onSelect,
  onOpenPairing,
  onOpenSettings,
}: Props) {
  return (
    <aside className="sidebar">
      <header className="sidebar-head">
        <div className="brand">
          <span className="brand-dot" />
          TaylorChat
        </div>
        <div className="sidebar-actions">
          <button className="btn-icon" title={t("sidebar.settingsTip")} onClick={onOpenSettings}>
            ⚙
          </button>
          <button className="btn-icon" title={t("sidebar.pairTip")} onClick={onOpenPairing}>
            ＋
          </button>
        </div>
      </header>

      {me && (
        <div className="me" title={me.nodeId}>
          <span className="me-label">{t("me")}</span>
          <code className="me-id">{shortId(me.nodeId)}</code>
        </div>
      )}

      <div className="contacts">
        {contacts.length === 0 && (
          <div className="empty">
            <p>{t("sidebar.empty")}</p>
            <button className="btn" onClick={onOpenPairing}>
              {t("sidebar.pair")}
            </button>
          </div>
        )}
        {contacts.map((c) => {
          const s = summaries[c.nodeId];
          const n = unread[c.nodeId] ?? 0;
          return (
            <button
              key={c.nodeId}
              className={`contact ${selected === c.nodeId ? "is-active" : ""}`}
              onClick={() => onSelect(c.nodeId)}
            >
              <span className="avatar" style={{ background: avatarColor(c.nodeId) }}>
                {(c.nickname || c.nodeId).slice(0, 1).toUpperCase()}
              </span>
              <span className="contact-body">
                <span className="contact-top">
                  <span className="contact-name">{c.nickname || shortId(c.nodeId)}</span>
                  {s && <span className="contact-time">{shortTime(s.ts)}</span>}
                </span>
                <span className={`contact-preview ${n > 0 ? "is-unread" : ""}`}>
                  {s ? (s.direction === "out" ? `Você: ${s.body}` : s.body) : shortId(c.nodeId)}
                </span>
              </span>
              {n > 0 && <span className="badge">{n}</span>}
            </button>
          );
        })}
      </div>
    </aside>
  );
}
