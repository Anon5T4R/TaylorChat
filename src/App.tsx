import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import * as api from "./lib/api";
import type { Contact, ConvoSummary, Message, MyIdentity } from "./lib/types";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { PairingModal } from "./components/PairingModal";
import { AiPanel } from "./components/AiPanel";

export default function App() {
  const [me, setMe] = useState<MyIdentity | null>(null);
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [pairingOpen, setPairingOpen] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [unread, setUnread] = useState<Record<string, number>>({});
  const [summaries, setSummaries] = useState<Record<string, ConvoSummary>>({});
  const [dropping, setDropping] = useState(false);

  // ref pra listeners de rede não lerem `selected` desatualizado
  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;

  const reloadContacts = useCallback(async () => {
    try {
      setContacts(await api.contactsList());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const reloadSummaries = useCallback(async () => {
    try {
      const list = await api.conversationsSummary();
      setSummaries(Object.fromEntries(list.map((s) => [s.peer, s])));
    } catch {
      /* prévia é cosmética — não vira erro na tela */
    }
  }, []);

  const loadMessages = useCallback(async (peer: string) => {
    try {
      setMessages(await api.messagesList(peer));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /// Tenta despachar a fila do par; se algo saiu e a conversa está aberta, recarrega.
  const flushQueue = useCallback(
    (peer: string) => {
      api
        .resendQueued(peer)
        .then((n) => {
          if (n > 0 && peer === selectedRef.current) loadMessages(peer);
        })
        .catch(() => {});
    },
    [loadMessages],
  );

  // boot: identidade + contatos + prévias + listeners (mensagens, recibos, drag&drop)
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    (async () => {
      try {
        setMe(await api.myIdentity());
      } catch (e) {
        setError(String(e));
      }
      await reloadContacts();
      await reloadSummaries();
      unlisteners.push(
        await api.onMessageIn((m) => {
          reloadSummaries();
          if (m.peer === selectedRef.current) {
            setMessages((prev) => [...prev, m]);
            api.markRead(m.peer).catch(() => {});
          } else {
            setUnread((prev) => ({ ...prev, [m.peer]: (prev[m.peer] ?? 0) + 1 }));
          }
          // o par acabou de falar comigo → está online: despacha o que ficou na fila
          flushQueue(m.peer);
        }),
      );
      unlisteners.push(
        await api.onReceipts((peer) => {
          if (peer === selectedRef.current) loadMessages(peer);
        }),
      );
      // Drag & drop de arquivos na conversa (paths vêm do webview do Tauri).
      try {
        unlisteners.push(
          await getCurrentWebview().onDragDropEvent((event) => {
            const p = event.payload as { type: string; paths?: string[] };
            if (p.type === "enter" || p.type === "over") setDropping(true);
            if (p.type === "leave") setDropping(false);
            if (p.type === "drop") {
              setDropping(false);
              const peer = selectedRef.current;
              if (!peer || !p.paths?.length) return;
              (async () => {
                for (const path of p.paths!.slice(0, 10)) {
                  try {
                    const msg = await api.attachPath(peer, path);
                    if (peer === selectedRef.current) setMessages((prev) => [...prev, msg]);
                  } catch (e) {
                    setError(String(e));
                  }
                }
                reloadSummaries();
              })();
            }
          }),
        );
      } catch {
        /* fora do Tauri (preview) não há drag&drop nativo */
      }
    })();
    return () => unlisteners.forEach((u) => u());
  }, [reloadContacts, reloadSummaries, loadMessages, flushQueue]);

  const handleSelect = useCallback(
    (nodeId: string) => {
      setSelected(nodeId);
      loadMessages(nodeId);
      setUnread((prev) => (prev[nodeId] ? { ...prev, [nodeId]: 0 } : prev));
      api.markRead(nodeId).catch(() => {});
      flushQueue(nodeId);
    },
    [loadMessages, flushQueue],
  );

  const handleSend = useCallback(
    async (body: string) => {
      if (!selected) return;
      try {
        const msg = await api.sendMessage(selected, body);
        setMessages((prev) => [...prev, msg]);
        reloadSummaries();
      } catch (e) {
        setError(String(e));
      }
    },
    [selected, reloadSummaries],
  );

  const handleAttach = useCallback(async () => {
    if (!selected) return;
    try {
      const msg = await api.pickAndAttach(selected);
      if (msg) {
        setMessages((prev) => [...prev, msg]);
        reloadSummaries();
      }
    } catch (e) {
      setError(String(e));
    }
  }, [selected, reloadSummaries]);

  const handleRemoveContact = useCallback(async () => {
    if (!selected) return;
    try {
      await api.contactRemove(selected);
      setSelected(null);
      setMessages([]);
      await reloadContacts();
    } catch (e) {
      setError(String(e));
    }
  }, [selected, reloadContacts]);

  const handleRenameContact = useCallback(
    async (nickname: string) => {
      if (!selected) return;
      try {
        await api.contactAdd(selected, nickname);
        await reloadContacts();
      } catch (e) {
        setError(String(e));
      }
    },
    [selected, reloadContacts],
  );

  const handleAddContact = useCallback(
    async (nodeId: string, nickname: string) => {
      await api.contactAdd(nodeId, nickname);
      await reloadContacts();
      setPairingOpen(false);
      handleSelect(nodeId);
    },
    [reloadContacts, handleSelect],
  );

  const selectedContact = contacts.find((c) => c.nodeId === selected) ?? null;
  const showAi = aiOpen && !!selectedContact;
  const sortedContacts = [...contacts].sort(
    (a, b) => (summaries[b.nodeId]?.ts ?? 0) - (summaries[a.nodeId]?.ts ?? 0),
  );

  return (
    <div className={`app ${showAi ? "with-ai" : ""}`}>
      <Sidebar
        me={me}
        contacts={sortedContacts}
        selected={selected}
        unread={unread}
        summaries={summaries}
        onSelect={handleSelect}
        onOpenPairing={() => setPairingOpen(true)}
      />
      <ChatPanel
        contact={selectedContact}
        messages={messages}
        draft={draft}
        onDraftChange={setDraft}
        onSend={handleSend}
        onAttach={handleAttach}
        onToggleAi={() => setAiOpen((v) => !v)}
        aiOpen={aiOpen}
        onRename={handleRenameContact}
        onRemove={handleRemoveContact}
      />
      {showAi && (
        <AiPanel
          messages={messages}
          draft={draft}
          onUseText={setDraft}
          onClose={() => setAiOpen(false)}
        />
      )}
      {pairingOpen && me && (
        <PairingModal me={me} onClose={() => setPairingOpen(false)} onAdd={handleAddContact} />
      )}
      {dropping && selected && (
        <div className="drop-overlay">
          <div className="drop-inner">📎 Solte para enviar</div>
        </div>
      )}
      {error && (
        <div className="toast" onClick={() => setError(null)} role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
