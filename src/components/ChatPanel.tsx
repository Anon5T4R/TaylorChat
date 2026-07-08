import { Fragment, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openAttachment, type KeywordStatus } from "../lib/api";
import type { Contact, FileMeta, Message } from "../lib/types";
import { dayLabel, shortId } from "../lib/ui";
import { t } from "../lib/i18n";
import { StickerPicker } from "./StickerPicker";
import { Avatar } from "./Avatar";

interface Props {
  contact: Contact | null;
  threadName: string;
  messages: Message[];
  draft: string;
  kw: KeywordStatus | null;
  onDraftChange: (v: string) => void;
  onSend: (body: string) => void;
  onAttach: () => void;
  onToggleAi: () => void;
  aiOpen: boolean;
  onRename: (nickname: string) => void;
  onRemove: () => void;
  onSetKeyword: () => void;
  onClear: () => void;
  onNewChat: () => void;
  onSendSticker: (path: string) => void;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function stateGlyph(m: Message): string {
  if (m.direction === "in") return "";
  switch (m.state) {
    case "queued":
      return "🕒";
    case "failed":
      return "⚠";
    case "sent":
      return "✓";
    case "delivered":
    case "read":
      return "✓✓";
    default:
      return "";
  }
}

function FileBubble({ body }: { body: string }) {
  const [imgFail, setImgFail] = useState(false);
  let meta: FileMeta | null = null;
  try {
    meta = JSON.parse(body) as FileMeta;
  } catch {
    /* corpo ilegível (chave errada) */
  }
  if (!meta) return <span className="bubble-body">{t("chat.fileUnreadable")}</span>;
  const path = meta.localPath;

  // Sticker → imagem grande sem moldura de bolha.
  if (path && meta.sticker && !imgFail) {
    return (
      <img
        className="sticker-att"
        src={convertFileSrc(path)}
        alt="sticker"
        onError={() => setImgFail(true)}
        onClick={() => openAttachment(path)}
      />
    );
  }

  // Imagem com cópia local → preview inline (clica pra abrir no visualizador do SO).
  if (path && meta.mime.startsWith("image/") && !imgFail) {
    return (
      <img
        className="img-att"
        src={convertFileSrc(path)}
        alt={meta.filename}
        title={meta.filename}
        onError={() => setImgFail(true)}
        onClick={() => openAttachment(path)}
      />
    );
  }

  return (
    <button
      className="file-att"
      disabled={!path}
      title={path ? t("chat.openTip") : t("chat.unavailable")}
      onClick={() => path && openAttachment(path)}
    >
      <span className="file-ico">📎</span>
      <span className="file-info">
        <span className="file-name">{meta.filename}</span>
        <span className="file-size">{formatSize(meta.size)}</span>
      </span>
    </button>
  );
}

export function ChatPanel({
  contact,
  threadName,
  messages,
  draft,
  kw,
  onDraftChange,
  onSend,
  onAttach,
  onToggleAi,
  aiOpen,
  onRename,
  onRemove,
  onSetKeyword,
  onClear,
  onNewChat,
  onSendSticker,
}: Props) {
  const endRef = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [stickerOpen, setStickerOpen] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!searchOpen) endRef.current?.scrollIntoView({ block: "end" });
  }, [messages, searchOpen]);

  // Ctrl+F abre/fecha a busca na conversa.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setSearchOpen((v) => !v);
      } else if (e.key === "Escape" && searchOpen) {
        setSearchOpen(false);
        setSearch("");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [searchOpen]);

  useEffect(() => {
    if (searchOpen) searchRef.current?.focus();
    else setSearch("");
  }, [searchOpen]);

  if (!contact) {
    return (
      <main className="chat chat-empty">
        <div className="chat-empty-inner">
          <div className="brand-dot big" />
          <h2>{t("chat.emptyTitle")}</h2>
          <p>{t("chat.empty")}</p>
        </div>
      </main>
    );
  }

  const submit = () => {
    const body = draft.trim();
    if (!body) return;
    onSend(body);
    onDraftChange("");
  };

  const rows = Math.min(6, Math.max(1, draft.split("\n").length));
  const kwClass = kw?.matches === true ? "kw-ok" : kw?.matches === false ? "kw-bad" : "";
  const q = search.trim().toLowerCase();
  const visible = q
    ? messages.filter((m) => {
        if (m.kind === "file") {
          try {
            return String(JSON.parse(m.body).filename ?? "").toLowerCase().includes(q);
          } catch {
            return false;
          }
        }
        return m.body.toLowerCase().includes(q);
      })
    : messages;
  let lastDay = "";

  return (
    <main className="chat">
      <header className="chat-head">
        <Avatar
          nodeId={contact.nodeId}
          name={contact.nickname || contact.profileName || contact.nodeId}
          avatar={contact.avatar}
        />
        <div className="chat-head-body">
          <strong>
            {contact.nickname || contact.profileName || shortId(contact.nodeId)}
            {threadName && <span className="thread-tag">· {threadName}</span>}
          </strong>
          <code>{shortId(contact.nodeId)}</code>
        </div>
        <div className="chat-head-actions">
          <button
            className={`btn-hdr ${searchOpen ? "is-active" : ""}`}
            title={t("chat.searchTip")}
            onClick={() => setSearchOpen((v) => !v)}
          >
            🔍
          </button>
          <button
            className={`btn-hdr ${kwClass}`}
            title={t("chat.kwTip")}
            onClick={onSetKeyword}
          >
            🔑
          </button>
          <button
            className="btn-hdr"
            title={t("chat.clearTip")}
            onClick={() => {
              if (window.confirm(t("chat.clearConfirm"))) onClear();
            }}
          >
            🧹
          </button>
          <button className="btn-hdr" title={t("chat.newChatTip")} onClick={onNewChat}>
            ➕
          </button>
          <button
            className="btn-hdr"
            title={t("chat.renameTip")}
            onClick={() => {
              const n = window.prompt(t("chat.renamePrompt"), contact.nickname);
              if (n !== null) onRename(n.trim());
            }}
          >
            ✏️
          </button>
          <button
            className="btn-hdr"
            title={t("chat.removeTip")}
            onClick={() => {
              if (window.confirm(t("chat.removeConfirm"))) onRemove();
            }}
          >
            🗑
          </button>
          <button
            className={`btn-ai ${aiOpen ? "is-active" : ""}`}
            title={t("chat.aiTip")}
            onClick={onToggleAi}
          >
            ✦ IA
          </button>
        </div>
      </header>

      {kw?.matches === false && <div className="kw-banner">{t("kw.mismatch")}</div>}

      {searchOpen && (
        <div className="search-bar">
          <input
            ref={searchRef}
            value={search}
            placeholder={t("chat.searchPh")}
            onChange={(e) => setSearch(e.target.value)}
          />
          <button className="btn-icon" onClick={() => setSearchOpen(false)}>
            ✕
          </button>
        </div>
      )}

      <div className="messages">
        {visible.map((m) => {
          const day = dayLabel(m.ts);
          const sep = day !== lastDay;
          lastDay = day;
          return (
            <Fragment key={m.id}>
              {sep && (
                <div className="day-sep">
                  <span>{day}</span>
                </div>
              )}
              <div className={`bubble ${m.direction === "out" ? "out" : "in"}`}>
                {m.kind === "file" ? (
                  <FileBubble body={m.body} />
                ) : (
                  <span className="bubble-body">{m.body}</span>
                )}
                <span className="bubble-meta">
                  {new Date(m.ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}{" "}
                  <span
                    className={`tick ${m.state === "read" ? "read" : ""}`}
                    title={
                      m.state === "queued"
                        ? t("tick.queued")
                        : m.state === "failed"
                          ? t("tick.failed")
                          : m.state
                    }
                  >
                    {stateGlyph(m)}
                  </span>
                </span>
              </div>
            </Fragment>
          );
        })}
        <div ref={endRef} />
      </div>

      {stickerOpen && (
        <StickerPicker
          onPick={(p) => {
            onSendSticker(p);
            setStickerOpen(false);
          }}
          onClose={() => setStickerOpen(false)}
        />
      )}

      <footer className="composer">
        <button
          className={`btn-attach ${stickerOpen ? "is-active" : ""}`}
          title={t("chat.stickerTip")}
          onClick={() => setStickerOpen((v) => !v)}
        >
          😀
        </button>
        <button className="btn-attach" title={t("chat.attachTip")} onClick={onAttach}>
          📎
        </button>
        <textarea
          value={draft}
          placeholder={t("chat.placeholder")}
          rows={rows}
          onChange={(e) => onDraftChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <button className="btn-send" onClick={submit} disabled={!draft.trim()}>
          {t("chat.send")}
        </button>
      </footer>
    </main>
  );
}
