import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  onPresence,
  openAttachment,
  openLink,
  peerOnline,
  peerUnwatch,
  type KeywordStatus,
  type Reaction,
} from "../lib/api";
import type { Contact, FileMeta, Message } from "../lib/types";
import { dayLabel, shortId } from "../lib/ui";
import { t } from "../lib/i18n";
import { StickerPicker } from "./StickerPicker";
import { Avatar } from "./Avatar";

// Reações rápidas (como o WhatsApp).
const REACT_EMOJIS = ["👍", "❤️", "😂", "😮", "😢", "🙏"];

// Emojis comuns pro seletor do composer (sem dependência externa).
const EMOJIS =
  "😀 😁 😂 🤣 😊 😍 😘 😎 🤩 🥳 😉 🙂 🙃 😇 🤔 🤨 😐 😴 😢 😭 😤 😠 😱 😳 🥺 🤯 🤗 👍 👎 👌 🙏 👏 💪 🔥 ✨ 🎉 ❤️ 🧡 💛 💚 💙 💜 🖤 💯 ✅ ❌ ⭐ 👀 🎂 🍺 ☕".split(
    " ",
  );

// URL → link clicável, sem HTML cru (evita XSS). Divide o texto e transforma só as URLs.
function renderText(text: string) {
  const parts = text.split(/(https?:\/\/[^\s]+)/g);
  return parts.map((p, i) =>
    /^https?:\/\/\S+$/.test(p) ? (
      <a
        key={i}
        className="msg-link"
        href={p}
        onClick={(e) => {
          e.preventDefault();
          openLink(p);
        }}
      >
        {p}
      </a>
    ) : (
      <span key={i}>{p}</span>
    ),
  );
}

interface Props {
  contact: Contact | null;
  threadName: string;
  messages: Message[];
  draft: string;
  kw: KeywordStatus | null;
  peerTyping?: boolean;
  reactions?: Reaction[];
  onReact: (targetTs: number, emoji: string) => void;
  onForward: (m: Message) => void;
  hasMore?: boolean;
  onLoadOlder: () => void;
  onDraftChange: (v: string) => void;
  onSend: (body: string, replyTo?: number | null, replyPreview?: string | null) => void;
  onAttach: () => void;
  onToggleAi: () => void;
  aiOpen: boolean;
  onOpenProfile: () => void;
  onClear: () => void;
  onNewChat: () => void;
  onSendSticker: (path: string) => void;
  onDeleteMine: (id: number) => void;
  onDeleteEveryone: (ts: number) => void;
  onToggleMute: () => void;
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
  peerTyping,
  reactions,
  onReact,
  onForward,
  hasMore,
  onLoadOlder,
  onDraftChange,
  onSend,
  onAttach,
  onToggleAi,
  aiOpen,
  onOpenProfile,
  onClear,
  onNewChat,
  onSendSticker,
  onDeleteMine,
  onDeleteEveryone,
  onToggleMute,
}: Props) {
  const endRef = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [stickerOpen, setStickerOpen] = useState(false);
  const [emojiOpen, setEmojiOpen] = useState(false);
  const [online, setOnline] = useState<boolean | null>(null); // null = verificando
  const [menuFor, setMenuFor] = useState<number | null>(null); // id da msg com menu aberto
  const [replyingTo, setReplyingTo] = useState<Message | null>(null);
  const [showNewPill, setShowNewPill] = useState(false); // "nova mensagem ↓" (rolado pra cima)
  const searchRef = useRef<HTMLInputElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const node = contact?.nodeId ?? null;

  const scrollToBottom = () => {
    endRef.current?.scrollIntoView({ block: "end" });
    setShowNewPill(false);
  };

  // Fecha o menu da bolha ao clicar fora (o clique no ⋯/menu usa stopPropagation).
  useEffect(() => {
    if (menuFor === null) return;
    const close = () => setMenuFor(null);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [menuFor]);

  // Insere um emoji na posição do cursor do composer.
  const insertEmoji = (emo: string) => {
    const ta = taRef.current;
    if (!ta) {
      onDraftChange(draft + emo);
      return;
    }
    const start = ta.selectionStart ?? draft.length;
    const end = ta.selectionEnd ?? draft.length;
    onDraftChange(draft.slice(0, start) + emo + draft.slice(end));
    requestAnimationFrame(() => {
      ta.focus();
      const pos = start + emo.length;
      ta.setSelectionRange(pos, pos);
    });
  };

  // Presença em tempo real: ao abrir a conversa, `peerOnline` liga a conexão quente +
  // heartbeat e devolve o status inicial; daí o evento `presence` atualiza a bolinha na
  // hora que o par entra/sai (o ping/pong detecta). null = ainda verificando.
  useEffect(() => {
    if (!node) return;
    let alive = true;
    setOnline(null);
    peerOnline(node)
      .then((v) => alive && setOnline(v))
      .catch(() => {});
    const unlisten = onPresence((p) => {
      if (alive && p.peer === node) setOnline(p.online);
    });
    return () => {
      alive = false;
      unlisten.then((u) => u());
      peerUnwatch(node).catch(() => {}); // encerra o watcher ao fechar/trocar (L4)
    };
  }, [node]);

  const prevFirstId = useRef<number | null>(null);
  const prevNode = useRef<string | null>(null);
  const prevLen = useRef(0);
  const prevScrollHeight = useRef(0);
  useLayoutEffect(() => {
    const el = messagesRef.current;
    const firstId = messages[0]?.id ?? null;
    const lastMsg = messages[messages.length - 1];
    const nodeChanged = prevNode.current !== node;
    // Prepend (carregar antigas): a 1ª msg ficou mais antiga.
    const prepended =
      !nodeChanged &&
      prevFirstId.current !== null &&
      firstId !== null &&
      firstId < prevFirstId.current;
    // Append (mensagem nova no fim), sem ser troca de conversa nem prepend.
    const appended = !nodeChanged && !prepended && messages.length > prevLen.current;

    if (searchOpen) {
      // não mexe no scroll durante a busca
    } else if (prepended && el) {
      // #10: mantém a posição — o conteúdo cresceu no topo, compensa o scroll.
      el.scrollTop += el.scrollHeight - prevScrollHeight.current;
    } else if (appended && el) {
      const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 120;
      // #6: se estou lendo antigas (longe do fim) e não fui eu que mandei, mostra o pill.
      if (nearBottom || lastMsg?.direction === "out") scrollToBottom();
      else setShowNewPill(true);
    } else if (nodeChanged) {
      scrollToBottom(); // abriu/trocou a conversa → desce
    }
    // senão: atualização in-place (recibo de leitura, apagar, mesmo tamanho) — NÃO mexe
    // no scroll, senão qualquer refresh jogava o usuário pro fim mesmo lendo antigas.

    prevFirstId.current = firstId;
    prevNode.current = node;
    prevLen.current = messages.length;
    if (el) prevScrollHeight.current = el.scrollHeight;
  }, [messages, searchOpen, node]);

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
    const preview = replyingTo
      ? replyingTo.kind === "file"
        ? "📎"
        : replyingTo.body.slice(0, 90)
      : null;
    onSend(body, replyingTo?.ts ?? null, preview);
    onDraftChange("");
    setReplyingTo(null);
  };

  const rows = Math.min(6, Math.max(1, draft.split("\n").length));
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
        <button className="chat-head-id" title={t("chat.profileTip")} onClick={onOpenProfile}>
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
            <span className="chat-presence">
            {peerTyping ? (
              <span className="typing-label">{t("chat.typing")}</span>
            ) : (
              <>
                <span
                  className={`presence-dot ${online === true ? "is-online" : online === false ? "is-offline" : "is-unknown"}`}
                />
                {online === true ? t("chat.online") : online === false ? t("chat.offline") : t("chat.checking")}
              </>
            )}
              <code className="presence-id">{shortId(contact.nodeId)}</code>
            </span>
          </div>
        </button>
        <div className="chat-head-actions">
          <button
            className={`btn-hdr ${searchOpen ? "is-active" : ""}`}
            title={t("chat.searchTip")}
            onClick={() => setSearchOpen((v) => !v)}
          >
            🔍
          </button>
          <button
            className={`btn-hdr ${contact.muted ? "is-active" : ""}`}
            title={contact.muted ? t("chat.unmuteTip") : t("chat.muteTip")}
            onClick={onToggleMute}
          >
            {contact.muted ? "🔕" : "🔔"}
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

      <div
        className="messages"
        ref={messagesRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          if (showNewPill && el.scrollHeight - el.scrollTop - el.clientHeight < 120)
            setShowNewPill(false);
        }}
      >
        {hasMore && !search && (
          <button className="load-older" onClick={onLoadOlder}>
            {t("chat.loadOlder")}
          </button>
        )}
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
              <div className={`bubble ${m.direction === "out" ? "out" : "in"} ${m.deleted ? "is-deleted" : ""}`}>
                {!m.deleted && (
                  <button
                    className="bubble-menu-btn"
                    title="⋯"
                    onClick={(e) => {
                      e.stopPropagation();
                      setMenuFor((cur) => (cur === m.id ? null : m.id));
                    }}
                  >
                    ⋯
                  </button>
                )}
                {menuFor === m.id && (
                  <div className="bubble-menu" onClick={(e) => e.stopPropagation()}>
                    <div className="react-row">
                      {REACT_EMOJIS.map((e) => (
                        <button
                          key={e}
                          className="react-pick"
                          onClick={() => {
                            onReact(m.ts, e);
                            setMenuFor(null);
                          }}
                        >
                          {e}
                        </button>
                      ))}
                    </div>
                    {m.kind !== "file" && (
                      <button
                        onClick={() => {
                          navigator.clipboard.writeText(m.body).catch(() => {});
                          setMenuFor(null);
                        }}
                      >
                        {t("msg.copy")}
                      </button>
                    )}
                    <button
                      onClick={() => {
                        setReplyingTo(m);
                        setMenuFor(null);
                        taRef.current?.focus();
                      }}
                    >
                      {t("msg.reply")}
                    </button>
                    <button
                      onClick={() => {
                        onForward(m);
                        setMenuFor(null);
                      }}
                    >
                      {t("msg.forward")}
                    </button>
                    <button
                      onClick={() => {
                        onDeleteMine(m.id);
                        setMenuFor(null);
                      }}
                    >
                      {t("msg.deleteMine")}
                    </button>
                    {m.direction === "out" && (
                      <button
                        className="danger"
                        onClick={() => {
                          if (window.confirm(t("msg.deleteAllConfirm"))) onDeleteEveryone(m.ts);
                          setMenuFor(null);
                        }}
                      >
                        {t("msg.deleteAll")}
                      </button>
                    )}
                  </div>
                )}
                {m.replyTo != null &&
                  (() => {
                    // Usa o trecho guardado (#4); só cai no lookup pra msgs antigas (pré-migração).
                    let txt = m.replyPreview ?? "";
                    if (!txt) {
                      const q = messages.find((x) => x.ts === m.replyTo);
                      txt = !q
                        ? t("msg.quoteGone")
                        : q.deleted
                          ? t("msg.deleted")
                          : q.kind === "file"
                            ? "📎"
                            : q.body.slice(0, 90);
                    }
                    return <div className="bubble-quote">{txt}</div>;
                  })()}
                {m.deleted ? (
                  <span className="bubble-body deleted">🚫 {t("msg.deleted")}</span>
                ) : m.kind === "file" ? (
                  <FileBubble body={m.body} />
                ) : (
                  <span className="bubble-body">{renderText(m.body)}</span>
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
                {(() => {
                  const mine = reactions?.find((r) => r.targetTs === m.ts && r.mine)?.emoji;
                  const peer = reactions?.find((r) => r.targetTs === m.ts && !r.mine)?.emoji;
                  if (!mine && !peer) return null;
                  return (
                    <span className="reactions">
                      {peer && <span className="react-chip">{peer}</span>}
                      {mine && (
                        <span
                          className="react-chip mine"
                          title={t("msg.reactRemove")}
                          onClick={() => onReact(m.ts, mine)}
                        >
                          {mine}
                        </span>
                      )}
                    </span>
                  );
                })()}
              </div>
            </Fragment>
          );
        })}
        <div ref={endRef} />
      </div>

      {showNewPill && (
        <button className="new-msg-pill" onClick={scrollToBottom}>
          {t("chat.newMessages")} ↓
        </button>
      )}

      {stickerOpen && (
        <StickerPicker
          onPick={(p) => {
            onSendSticker(p);
            setStickerOpen(false);
          }}
          onClose={() => setStickerOpen(false)}
        />
      )}

      {emojiOpen && (
        <div className="emoji-picker">
          {EMOJIS.map((e) => (
            <button key={e} className="emoji-cell" onClick={() => insertEmoji(e)}>
              {e}
            </button>
          ))}
        </div>
      )}

      {replyingTo && (
        <div className="reply-compose">
          <span className="reply-compose-txt">
            ↩ {replyingTo.kind === "file" ? "📎" : replyingTo.body.slice(0, 80)}
          </span>
          <button className="btn-icon" onClick={() => setReplyingTo(null)}>
            ✕
          </button>
        </div>
      )}

      <footer className="composer">
        <button
          className={`btn-attach ${emojiOpen ? "is-active" : ""}`}
          title={t("chat.emojiTip")}
          onClick={() => {
            setEmojiOpen((v) => !v);
            setStickerOpen(false);
          }}
        >
          🙂
        </button>
        <button
          className={`btn-attach ${stickerOpen ? "is-active" : ""}`}
          title={t("chat.stickerTip")}
          onClick={() => {
            setStickerOpen((v) => !v);
            setEmojiOpen(false);
          }}
        >
          😀
        </button>
        <button className="btn-attach" title={t("chat.attachTip")} onClick={onAttach}>
          📎
        </button>
        <textarea
          ref={taRef}
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
