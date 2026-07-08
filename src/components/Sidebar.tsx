import type { Contact, ConvoSummary, MyIdentity, Thread } from "../lib/types";
import { avatarColor, shortId, shortTime, splitConvo } from "../lib/ui";
import { t } from "../lib/i18n";

interface Props {
  me: MyIdentity | null;
  threads: Thread[];
  contacts: Contact[];
  selected: string | null;
  unread: Record<string, number>;
  summaries: Record<string, ConvoSummary>;
  onSelect: (convo: string) => void;
  onOpenPairing: () => void;
  onOpenSettings: () => void;
}

export function Sidebar({
  me,
  threads,
  contacts,
  selected,
  unread,
  summaries,
  onSelect,
  onOpenPairing,
  onOpenSettings,
}: Props) {
  const byNode = new Map(contacts.map((c) => [c.nodeId, c]));
  const rows = [...threads].sort(
    (a, b) => (summaries[b.convo]?.ts ?? 0) - (summaries[a.convo]?.ts ?? 0),
  );

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
        {rows.length === 0 && (
          <div className="empty">
            <p>{t("sidebar.empty")}</p>
            <button className="btn" onClick={onOpenPairing}>
              {t("sidebar.pair")}
            </button>
          </div>
        )}
        {rows.map((th) => {
          const { node } = splitConvo(th.convo);
          const c = byNode.get(node);
          const base = c?.nickname || shortId(node);
          const name = th.name ? `${base} · ${th.name}` : base;
          const s = summaries[th.convo];
          const n = unread[th.convo] ?? 0;
          return (
            <button
              key={th.convo}
              className={`contact ${selected === th.convo ? "is-active" : ""}`}
              onClick={() => onSelect(th.convo)}
            >
              <span className="avatar" style={{ background: avatarColor(node) }}>
                {base.slice(0, 1).toUpperCase()}
              </span>
              <span className="contact-body">
                <span className="contact-top">
                  <span className="contact-name">{name}</span>
                  {s && <span className="contact-time">{shortTime(s.ts)}</span>}
                </span>
                <span className={`contact-preview ${n > 0 ? "is-unread" : ""}`}>
                  {s ? (s.direction === "out" ? `${t("me")}: ${s.body}` : s.body) : shortId(node)}
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
