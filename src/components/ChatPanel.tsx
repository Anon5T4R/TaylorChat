import { useEffect, useRef } from "react";
import { openAttachment } from "../lib/api";
import type { Contact, FileMeta, Message } from "../lib/types";

interface Props {
  contact: Contact | null;
  messages: Message[];
  draft: string;
  onDraftChange: (v: string) => void;
  onSend: (body: string) => void;
  onAttach: () => void;
  onToggleAi: () => void;
  aiOpen: boolean;
  onRename: (nickname: string) => void;
  onRemove: () => void;
}

function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
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
  let meta: FileMeta | null = null;
  try {
    meta = JSON.parse(body) as FileMeta;
  } catch {
    /* corpo ilegível (chave errada) */
  }
  if (!meta) return <span className="bubble-body">⟨anexo ilegível⟩</span>;
  const path = meta.localPath;
  return (
    <button
      className="file-att"
      disabled={!path}
      title={path ? "Abrir anexo" : "Anexo indisponível"}
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
  messages,
  draft,
  onDraftChange,
  onSend,
  onAttach,
  onToggleAi,
  aiOpen,
  onRename,
  onRemove,
}: Props) {
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [messages]);

  if (!contact) {
    return (
      <main className="chat chat-empty">
        <div className="chat-empty-inner">
          <div className="brand-dot big" />
          <h2>TaylorChat</h2>
          <p>Selecione um contato ou pareie com alguém pra começar a conversar.</p>
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

  return (
    <main className="chat">
      <header className="chat-head">
        <span className="avatar">{(contact.nickname || contact.nodeId).slice(0, 1).toUpperCase()}</span>
        <div className="chat-head-body">
          <strong>{contact.nickname || shortId(contact.nodeId)}</strong>
          <code>{shortId(contact.nodeId)}</code>
        </div>
        <div className="chat-head-actions">
          <button
            className="btn-hdr"
            title="Renomear contato"
            onClick={() => {
              const n = window.prompt("Apelido do contato", contact.nickname);
              if (n !== null) onRename(n.trim());
            }}
          >
            ✏️
          </button>
          <button
            className="btn-hdr"
            title="Remover contato"
            onClick={() => {
              if (window.confirm("Remover este contato e sair da conversa?")) onRemove();
            }}
          >
            🗑
          </button>
          <button
            className={`btn-ai ${aiOpen ? "is-active" : ""}`}
            title="Assistente de IA local"
            onClick={onToggleAi}
          >
            ✦ IA
          </button>
        </div>
      </header>

      <div className="messages">
        {messages.map((m) => (
          <div key={m.id} className={`bubble ${m.direction === "out" ? "out" : "in"}`}>
            {m.kind === "file" ? <FileBubble body={m.body} /> : <span className="bubble-body">{m.body}</span>}
            <span className="bubble-meta">
              {new Date(m.ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}{" "}
              <span className={`tick ${m.state === "read" ? "read" : ""}`}>{stateGlyph(m)}</span>
            </span>
          </div>
        ))}
        <div ref={endRef} />
      </div>

      <footer className="composer">
        <button className="btn-attach" title="Anexar arquivo" onClick={onAttach}>
          📎
        </button>
        <textarea
          value={draft}
          placeholder="Escreva uma mensagem…"
          rows={1}
          onChange={(e) => onDraftChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <button className="btn-send" onClick={submit} disabled={!draft.trim()}>
          Enviar
        </button>
      </footer>
    </main>
  );
}
