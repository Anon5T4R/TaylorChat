import { useEffect, useState } from "react";
import type { Contact, ConvoSummary, MyIdentity, Profile, Thread } from "../lib/types";
import * as api from "../lib/api";
import { shortId, shortTime, splitConvo } from "../lib/ui";
import { t } from "../lib/i18n";
import { Avatar } from "./Avatar";

interface Props {
  me: MyIdentity | null;
  myProfile: Profile | null;
  threads: Thread[];
  contacts: Contact[];
  selected: string | null;
  unread: Record<string, number>;
  summaries: Record<string, ConvoSummary>;
  onSelect: (convo: string) => void;
  onOpenPairing: () => void;
  onOpenSettings: () => void;
  onOpenProfile: (node: string) => void;
}

export function Sidebar({
  me,
  myProfile,
  threads,
  contacts,
  selected,
  unread,
  summaries,
  onSelect,
  onOpenPairing,
  onOpenSettings,
  onOpenProfile,
}: Props) {
  const [tab, setTab] = useState<"chats" | "contacts">("chats");
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<api.SearchHit[]>([]);
  const sq = q.trim();
  const sqLower = sq.toLowerCase();

  // Busca de mensagens (backend, decifra e casa) com um pequeno debounce.
  useEffect(() => {
    if (!sq) {
      setHits([]);
      return;
    }
    let alive = true;
    const id = setTimeout(() => {
      api.searchMessages(sq, 30).then((h) => alive && setHits(h)).catch(() => {});
    }, 180);
    return () => {
      alive = false;
      clearTimeout(id);
    };
  }, [sq]);

  const byNode = new Map(contacts.map((c) => [c.nodeId, c]));
  const contactHits = sq
    ? contacts.filter(
        (c) =>
          (c.nickname || "").toLowerCase().includes(sqLower) ||
          (c.profileName || "").toLowerCase().includes(sqLower) ||
          c.nodeId.includes(sqLower),
      )
    : [];
  const rows = [...threads].sort(
    (a, b) => (summaries[b.convo]?.ts ?? 0) - (summaries[a.convo]?.ts ?? 0),
  );
  const contactRows = [...contacts].sort((a, b) =>
    (a.nickname || a.profileName || a.nodeId).localeCompare(b.nickname || b.profileName || b.nodeId),
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
        <button className="me" title={me.nodeId} onClick={onOpenSettings}>
          <Avatar nodeId={me.nodeId} name={myProfile?.name || t("me")} avatar={myProfile?.avatar} size={30} />
          <span className="me-body">
            <span className="me-name">{myProfile?.name || t("me")}</span>
            <code className="me-id">{shortId(me.nodeId)}</code>
          </span>
        </button>
      )}

      <button className="pair-cta" onClick={onOpenPairing}>
        ＋ {t("sidebar.pair")}
      </button>

      <div className="sidebar-search">
        <span className="search-ico">🔍</span>
        <input
          value={q}
          placeholder={t("sidebar.search")}
          onChange={(e) => setQ(e.target.value)}
        />
        {q && (
          <button className="search-clear" onClick={() => setQ("")} title="✕">
            ✕
          </button>
        )}
      </div>

      {sq ? (
        <div className="contacts">
          {contactHits.length === 0 && hits.length === 0 && (
            <div className="empty">
              <p>{t("sidebar.searchNone")}</p>
            </div>
          )}
          {contactHits.length > 0 && <div className="search-group">{t("sidebar.searchContacts")}</div>}
          {contactHits.map((c) => {
            const name = c.nickname || c.profileName || shortId(c.nodeId);
            return (
              <button
                key={`c-${c.nodeId}`}
                className={`contact ${selected && splitConvo(selected).node === c.nodeId ? "is-active" : ""}`}
                onClick={() => onSelect(c.nodeId)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  onOpenProfile(c.nodeId);
                }}
              >
                <Avatar nodeId={c.nodeId} name={name} avatar={c.avatar} />
                <span className="contact-body">
                  <span className="contact-top">
                    <span className="contact-name">{name}</span>
                  </span>
                  <span className="contact-preview">
                    <code>{shortId(c.nodeId)}</code>
                  </span>
                </span>
              </button>
            );
          })}
          {hits.length > 0 && <div className="search-group">{t("sidebar.searchMessages")}</div>}
          {hits.map((h, i) => {
            const node = splitConvo(h.convo).node;
            const c = byNode.get(node);
            const base = c?.nickname || c?.profileName || shortId(node);
            return (
              <button
                key={`h-${i}`}
                className={`contact ${selected === h.convo ? "is-active" : ""}`}
                onClick={() => onSelect(h.convo)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  onOpenProfile(node);
                }}
              >
                <Avatar nodeId={node} name={base} avatar={c?.avatar} />
                <span className="contact-body">
                  <span className="contact-top">
                    <span className="contact-name">{base}</span>
                    <span className="contact-time">{shortTime(h.ts)}</span>
                  </span>
                  <span className="contact-preview">{h.snippet}</span>
                </span>
              </button>
            );
          })}
        </div>
      ) : (
      <>
      <div className="sidebar-tabs">
        <button
          className={`sidebar-tab ${tab === "chats" ? "is-active" : ""}`}
          onClick={() => setTab("chats")}
        >
          {t("sidebar.tabChats")}
        </button>
        <button
          className={`sidebar-tab ${tab === "contacts" ? "is-active" : ""}`}
          onClick={() => setTab("contacts")}
        >
          {t("sidebar.tabContacts")} {contacts.length > 0 && <span className="tab-count">{contacts.length}</span>}
        </button>
      </div>

      {tab === "contacts" && (
        <div className="contacts">
          {contactRows.length === 0 && (
            <div className="empty">
              <p>{t("sidebar.contactsEmpty")}</p>
              <button className="btn" onClick={onOpenPairing}>
                {t("sidebar.pair")}
              </button>
            </div>
          )}
          {contactRows.map((c) => {
            const name = c.nickname || c.profileName || shortId(c.nodeId);
            return (
              <button
                key={c.nodeId}
                className={`contact ${selected && splitConvo(selected).node === c.nodeId ? "is-active" : ""}`}
                onClick={() => onSelect(c.nodeId)}
              >
                <Avatar nodeId={c.nodeId} name={name} avatar={c.avatar} />
                <span className="contact-body">
                  <span className="contact-top">
                    <span className="contact-name">{name}</span>
                  </span>
                  <span className="contact-preview">
                    <code>{shortId(c.nodeId)}</code>
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      )}

      {tab === "chats" && (
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
          const base = c?.nickname || c?.profileName || shortId(node);
          const name = th.name ? `${base} · ${th.name}` : base;
          const s = summaries[th.convo];
          const n = unread[th.convo] ?? 0;
          return (
            <button
              key={th.convo}
              className={`contact ${selected === th.convo ? "is-active" : ""}`}
              onClick={() => onSelect(th.convo)}
              onContextMenu={(e) => {
                e.preventDefault();
                onOpenProfile(node);
              }}
            >
              <Avatar nodeId={node} name={base} avatar={c?.avatar} />
              <span className="contact-body">
                <span className="contact-top">
                  <span className="contact-name">{name}</span>
                  {s && <span className="contact-time">{shortTime(s.ts)}</span>}
                </span>
                <span className={`contact-preview ${n > 0 ? "is-unread" : ""}`}>
                  {s
                    ? s.deleted
                      ? `🚫 ${t("msg.deleted")}`
                      : s.direction === "out"
                        ? `${t("me")}: ${s.body}`
                        : s.body
                    : shortId(node)}
                </span>
              </span>
              {n > 0 && <span className="badge">{n}</span>}
            </button>
          );
        })}
      </div>
      )}
      </>
      )}
    </aside>
  );
}
